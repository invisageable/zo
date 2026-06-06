//! UIKit application bootstrap + `UiCommand` → native-view renderer.
//!
//! Uses the modern `UIScene` lifecycle: `UIApplicationMain` builds the
//! `ZoAppDelegate`, which only declares the scene configuration; UIKit
//! then connects a `UIWindowScene` and calls `ZoSceneDelegate`, which
//! builds the window via `initWithWindowScene:` (no deprecated
//! `UIScreen::mainScreen` / `UIWindow::initWithFrame:`). The Info.plist
//! `UIApplicationSceneManifest` names `ZoSceneDelegate`.

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{ClassType, MainThreadMarker, MainThreadOnly, define_class, sel};

use objc2_foundation::{NSArray, NSDictionary, NSObjectProtocol, NSString};
use objc2_ui_kit::{
  NSLayoutConstraint, UIApplication, UIApplicationDelegate,
  UIApplicationLaunchOptionsKey, UIButton, UIButtonType, UIColor,
  UIControlEvents, UIControlState, UILabel, UILayoutConstraintAxis, UIScene,
  UISceneConnectionOptions, UISceneDelegate, UISceneSession, UIStackView,
  UIStackViewAlignment, UIViewController, UIWindow, UIWindowScene,
  UIWindowSceneDelegate,
};

use zo_runtime_render::render::{EventPayload, EventRegistry, build_event_map};
use zo_ui_protocol::{Attr, ElementTag, EventKind, UiCommand};

use std::cell::RefCell;
use std::ffi::{c_char, c_int};
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};

thread_local! {
  /// Retains the key window for the process lifetime. UIKit holds
  /// only a weak reference, so without an owner here the window
  /// deallocates the moment `sceneWillConnect` returns and the
  /// screen goes black. A `thread_local` (not a `static`) because
  /// `Retained` is not `Sync`, and everything here runs on the
  /// UIKit main thread.
  static WINDOW: RefCell<Option<Retained<UIWindow>>> =
    const { RefCell::new(None) };

  /// The live reactive wiring: the registry that dispatches a tap
  /// into the compiled zo handler, the shared command buffer those
  /// handlers refresh in place, and the labels to re-read afterwards.
  /// Set by `install` before the run loop starts; read on every tap.
  /// A `thread_local` for the same reason as `WINDOW` — UIKit is
  /// single-threaded and `Retained`/`EventRegistry` aren't `Sync`.
  static RUNTIME: RefCell<Option<Runtime>> = const { RefCell::new(None) };
}

/// Per-process reactive wiring, built once at startup from the
/// `ZoRuntimeContext` and consumed by the scene delegate.
struct Runtime {
  /// Dispatches a handler name into its compiled zo closure, then
  /// refreshes reactive bindings into `shared`.
  registry: EventRegistry,
  /// The command buffer handlers mutate; reread after each tap to
  /// repaint bound labels.
  shared: Arc<Mutex<Vec<UiCommand>>>,
  /// `(command index, label)` for every reactive `Text` rendered as
  /// a `UILabel`. After a tap, `shared[index]`'s text is copied into
  /// the label.
  labels: Vec<(usize, Retained<UILabel>)>,
}

/// Stash the reactive wiring for the scene delegate to consume.
/// Called on the main thread before `UIApplicationMain` spins up the
/// run loop.
pub(crate) fn install(
  registry: EventRegistry,
  shared: Arc<Mutex<Vec<UiCommand>>>,
) {
  RUNTIME.with(|r| {
    *r.borrow_mut() = Some(Runtime {
      registry,
      shared,
      labels: Vec::new(),
    });
  });
}

define_class!(
  // SAFETY: the superclass `NSObject` has no subclassing
  // requirements, and `AppDelegate` holds no ivars / no `Drop`.
  #[unsafe(super(NSObject))]
  #[thread_kind = MainThreadOnly]
  #[name = "ZoAppDelegate"]
  struct AppDelegate;

  unsafe impl NSObjectProtocol for AppDelegate {}

  unsafe impl UIApplicationDelegate for AppDelegate {
    // The window is built by `ZoSceneDelegate` on scene connection;
    // the app delegate only has to launch cleanly. UIKit reads the
    // scene configuration (and the delegate class name) from the
    // Info.plist `UIApplicationSceneManifest`.
    #[unsafe(method(application:didFinishLaunchingWithOptions:))]
    fn did_finish_launching(
      &self,
      _application: &UIApplication,
      _launch_options: Option<
        &NSDictionary<UIApplicationLaunchOptionsKey, AnyObject>,
      >,
    ) -> bool {
      true
    }
  }
);

define_class!(
  // SAFETY: the superclass `NSObject` has no subclassing
  // requirements, and `SceneDelegate` holds no ivars / no `Drop`.
  #[unsafe(super(NSObject))]
  #[thread_kind = MainThreadOnly]
  #[name = "ZoSceneDelegate"]
  struct SceneDelegate;

  impl SceneDelegate {
    /// Target-action for every `UIButton`. The sender's `tag` is the
    /// widget id assigned at lowering; map it back to its click
    /// handler, dispatch into compiled zo (which mutates state and
    /// refreshes `shared`), then repaint the bound labels.
    #[unsafe(method(buttonTapped:))]
    fn button_tapped(&self, sender: &UIButton) {
      let widget_id = sender.tag().to_string();

      RUNTIME.with(|r| {
        let runtime = r.borrow();
        let Some(runtime) = runtime.as_ref() else {
          return;
        };

        // Resolve the handler under a short lock, then drop it
        // before dispatching: the registry callback re-locks
        // `shared` to refresh bindings, so holding it here would
        // deadlock. Rebuild the event map per tap (cheap, n tiny)
        // exactly as the desktop runtime does per frame.
        let handler = {
          let cmds = runtime.shared.lock().unwrap();

          build_event_map(&cmds)
            .get(&(widget_id, EventKind::Click))
            .cloned()
        };

        if let Some(handler) = handler {
          runtime.registry.dispatch(&handler, &EventPayload::default());
        }

        // Repaint: the handler refreshed `shared` in place, so each
        // bound label re-reads its command's current text.
        let cmds = runtime.shared.lock().unwrap();

        for (idx, label) in &runtime.labels {
          if let Some(UiCommand::Text(text)) = cmds.get(*idx) {
            label.setText(Some(&NSString::from_str(text)));
          }
        }
      });
    }
  }

  unsafe impl NSObjectProtocol for SceneDelegate {}

  unsafe impl UISceneDelegate for SceneDelegate {
    #[unsafe(method(scene:willConnectToSession:options:))]
    fn scene_will_connect(
      &self,
      scene: &UIScene,
      _session: &UISceneSession,
      _connection_options: &UISceneConnectionOptions,
    ) {
      let mtm = MainThreadMarker::from(self);

      // The application-role scene is always a `UIWindowScene`; the
      // window binds to it instead of the deprecated main screen.
      let Some(window_scene) = scene.downcast_ref::<UIWindowScene>() else {
        return;
      };

      let window =
        UIWindow::initWithWindowScene(UIWindow::alloc(mtm), window_scene);
      let controller = UIViewController::new(mtm);

      let cmds = RUNTIME.with(|r| {
        r.borrow()
          .as_ref()
          .map(|rt| rt.shared.lock().unwrap().clone())
          .unwrap_or_default()
      });

      let (stack, labels) = build_view(&cmds, self, mtm);

      RUNTIME.with(|r| {
        if let Some(rt) = r.borrow_mut().as_mut() {
          rt.labels = labels;
        }
      });

      if let Some(root) = controller.view() {
        root.setBackgroundColor(Some(&UIColor::whiteColor()));
        stack.setTranslatesAutoresizingMaskIntoConstraints(false);
        root.addSubview(&stack);

        // Pin the stack's centre to the view's centre. Without
        // explicit constraints the stack fills the screen and the
        // arranged items spread to the edges; centring clusters
        // `− 0 +` together.
        let constraints = NSArray::from_retained_slice(&[
          stack
            .centerXAnchor()
            .constraintEqualToAnchor(&root.centerXAnchor()),
          stack
            .centerYAnchor()
            .constraintEqualToAnchor(&root.centerYAnchor()),
        ]);

        NSLayoutConstraint::activateConstraints(&constraints, mtm);
      }

      window.setRootViewController(Some(&controller));
      window.makeKeyAndVisible();
      WINDOW.with(|w| *w.borrow_mut() = Some(window));
    }
  }

  // A window scene needs a `UIWindowSceneDelegate`; the `window`
  // accessors stay optional (the window is owned via `WINDOW`).
  unsafe impl UIWindowSceneDelegate for SceneDelegate {}
);

/// Build the native view tree from the command stream. A vertical
/// `UIStackView`; `Element{Button}` → `UIButton` whose title is the
/// `Text` that follows before `EndElement` and whose `tag` is the
/// lowering-assigned widget id (wired to `buttonTapped:` on
/// `target`); a free-standing `Text` → `UILabel`, returned with its
/// command index so taps can repaint it. Styles are ignored for now.
fn build_view(
  cmds: &[UiCommand],
  target: &SceneDelegate,
  mtm: MainThreadMarker,
) -> (Retained<UIStackView>, Vec<(usize, Retained<UILabel>)>) {
  let stack = UIStackView::new(mtm);

  stack.setAxis(UILayoutConstraintAxis::Vertical);
  stack.setAlignment(UIStackViewAlignment::Center);
  stack.setSpacing(24.0);

  let target_object: &AnyObject = target.as_ref();
  let mut labels: Vec<(usize, Retained<UILabel>)> = Vec::new();
  let mut pending_button: Option<Retained<UIButton>> = None;

  for (idx, cmd) in cmds.iter().enumerate() {
    match cmd {
      UiCommand::Element { tag, attrs, .. } if *tag == ElementTag::Button => {
        let button = UIButton::buttonWithType(UIButtonType::System, mtm);

        // The `data-id` carries the widget id the `Event` command
        // references; stash it in `tag` so `buttonTapped:` can route
        // back to the handler.
        if let Some(id) = widget_id(attrs) {
          button.setTag(id);
        }

        // SAFETY: `target_object` is the live scene delegate, and
        // `buttonTapped:` is a registered selector taking the sender.
        unsafe {
          button.addTarget_action_forControlEvents(
            Some(target_object),
            sel!(buttonTapped:),
            UIControlEvents::TouchUpInside,
          );
        }

        stack.addArrangedSubview(&button);
        pending_button = Some(button);
      }
      UiCommand::Text(content) => {
        let text = NSString::from_str(content);

        match &pending_button {
          Some(button) => {
            button.setTitle_forState(Some(&text), UIControlState::Normal);
          }
          None => {
            let label = UILabel::new(mtm);

            label.setText(Some(&text));
            stack.addArrangedSubview(&label);
            labels.push((idx, label));
          }
        }
      }
      UiCommand::EndElement => pending_button = None,
      _ => {}
    }
  }

  (stack, labels)
}

/// Read an element's `data-id` widget id. The executor stores it as
/// a numeric prop (`PropValue::Num`), so read `as_num` first and fall
/// back to parsing a string form. Without the numeric read every
/// button keeps the default tag `0` and all taps route to one widget.
fn widget_id(attrs: &[Attr]) -> Option<isize> {
  let attr = attrs.iter().find(|a| a.name() == "data-id")?;

  attr
    .as_num()
    .map(|n| n as isize)
    .or_else(|| attr.as_str().and_then(|s| s.parse().ok()))
}

/// Launch the UIKit run loop. Blocks until the app exits. The
/// delegates are constructed by UIKit from their registered class
/// names, so the classes must be registered first.
pub(crate) fn run() {
  let _mtm = MainThreadMarker::new()
    .expect("zo_run_native must be called on the main thread");

  // Force class registration so the ObjC runtime can resolve
  // "ZoAppDelegate" / "ZoSceneDelegate" by name when
  // `UIApplicationMain` and the scene manifest instantiate them.
  let _ = AppDelegate::class();
  let _ = SceneDelegate::class();

  let name = NSString::from_str("ZoAppDelegate");
  let mut argv0: *mut c_char = std::ptr::null_mut();
  let argv = NonNull::from(&mut argv0);

  // The system binding for `UIApplicationMain` is `#[deprecated]` in
  // favour of a crate-private `__main`; re-declare the underlying C
  // entry directly so `-D warnings` stays clean.
  unsafe {
    UIApplicationMain(0, argv, None, Some(&name));
  }
}

unsafe extern "C-unwind" {
  fn UIApplicationMain(
    argc: c_int,
    argv: NonNull<*mut c_char>,
    principal_class_name: Option<&NSString>,
    delegate_class_name: Option<&NSString>,
  ) -> c_int;
}
