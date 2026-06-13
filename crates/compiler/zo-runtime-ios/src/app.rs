//! UIKit application bootstrap + `UiCommand` → native-view renderer.
//!
//! Uses the modern `UIScene` lifecycle: `UIApplicationMain` builds the
//! `ZoAppDelegate`, which only declares the scene configuration; UIKit
//! then connects a `UIWindowScene` and calls `ZoSceneDelegate`, which
//! builds the window via `initWithWindowScene:` (no deprecated
//! `UIScreen::mainScreen` / `UIWindow::initWithFrame:`). The Info.plist
//! `UIApplicationSceneManifest` names `ZoSceneDelegate`.

use zo_runtime_render::aot::UpdateReport;
use zo_runtime_render::asset::load_image_bytes;
use zo_runtime_render::layout::{LayoutTree, Rect, collapse_text};
use zo_runtime_render::render::{EventPayload, EventRegistry, build_event_map};
use zo_ui_protocol::style::{
  ComputedStyle, GlassStyle, Material, Rgba, StylePatch,
};
use zo_ui_protocol::{Attr, ElementTag, EventKind, UiCommand};

#[cfg(target_os = "watchos")]
use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{ClassType, MainThreadMarker, MainThreadOnly, define_class, sel};

use objc2_core_foundation::{CGFloat, CGPoint, CGRect, CGSize};
use objc2_foundation::{
  NSArray, NSBundle, NSData, NSDictionary, NSObjectProtocol,
  NSOperatingSystemVersion, NSProcessInfo, NSString,
};
#[cfg(target_os = "watchos")]
use objc2_ui_kit::UIScreen;
use objc2_ui_kit::{
  UIApplication, UIApplicationLaunchOptionsKey, UIButton,
  UIButtonConfiguration, UIButtonType, UIColor, UIControlEvents,
  UIControlState, UIFont, UIGlassEffect, UIGlassEffectStyle, UIImage,
  UIImageView, UILabel, UISegmentedControl, UISwitch, UITextBorderStyle,
  UITextField, UIView, UIViewContentMode, UIViewController, UIVisualEffectView,
  UIWindow,
};
#[cfg(target_os = "ios")]
use objc2_ui_kit::{
  UIApplicationDelegate, UIScene, UISceneConnectionOptions, UISceneDelegate,
  UISceneSession, UIWindowScene, UIWindowSceneDelegate,
};

use std::cell::RefCell;
#[cfg(target_os = "ios")]
use std::ffi::c_float;
use std::ffi::{c_char, c_int};
use std::path::Path;
use std::ptr::NonNull;
use std::sync::{Arc, Mutex, OnceLock};

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

#[cfg(target_os = "watchos")]
thread_local! {
  /// Retains the widget event target for the process lifetime —
  /// `addTarget` holds its target weakly, and unlike iOS (where UIKit
  /// retains the scene delegate) nothing else on watchOS would keep
  /// it alive.
  static TARGET: RefCell<Option<Retained<SceneDelegate>>> =
    const { RefCell::new(None) };
}

/// Per-process reactive wiring, built once at startup from the
/// `ZoRuntimeContext` and consumed by the scene delegate.
struct Runtime {
  /// Dispatches a handler name into its compiled zo closure, then
  /// refreshes reactive bindings into `shared`.
  registry: EventRegistry,
  /// The command buffer handlers mutate; reread after each tap to
  /// reconcile the view tree.
  shared: Arc<Mutex<Vec<UiCommand>>>,
  /// What the registry's refresh did on the last event — which
  /// commands it rewrote, or that it replaced the stream. Read
  /// after dispatch so the view patches O(dirty) instead of
  /// re-diffing the whole stream.
  report: Arc<Mutex<UpdateReport>>,
  /// The live view hierarchy + its layout tree, retained across taps
  /// so a tap reconciles in place instead of rebuilding every widget.
  /// `None` until the scene connects and the first render runs.
  host: Option<ViewHost>,
}

/// The retained native view tree paired with its layout solver. Held
/// across updates: a tap mutates `shared`, then `reconcile` updates
/// only the widgets whose text changed and re-frames the rest — no
/// allocation on the common path, so hundreds of items stay cheap.
struct ViewHost {
  /// The root view every widget is framed inside; reused on rebuild.
  container: Retained<UIView>,
  /// Persistent Taffy tree — `reconcile` dirties only changed leaves.
  tree: LayoutTree,
  /// One native view per placed leaf, in `solve` order.
  views: Vec<PlacedView>,
}

/// A native view bound to a placed leaf. Kept across updates so the
/// reconciler repaints in place; the variant carries the concrete
/// type so text + frame writes hit the right API.
enum PlacedView {
  Button(Retained<UIButton>),
  Label(Retained<UILabel>),
  /// An editable text input (`<input>` / `<textarea>`) wired to
  /// fire `@input` on every edit and `@submit` on the return key.
  Field(Retained<UITextField>),
  /// A `<input type="checkbox">` → `UISwitch`, firing `@change`
  /// with `"true"` / `"false"` on toggle.
  Switch(Retained<UISwitch>),
  /// A `<select>` → `UISegmentedControl` over its `<option>`
  /// children — the idiomatic iOS one-of-N picker; `@change`
  /// carries the chosen option's title.
  Segmented(Retained<UISegmentedControl>),
  /// Reserved geometry for leaves the iOS path does not paint yet
  /// (image).
  Other(Retained<UIView>),
  /// A glass-backed leaf: `panel` is the glass stack (shadow host →
  /// glass) framed and attached to the container; `inner` is its real
  /// content, sized to the panel's local bounds inside the glass
  /// `contentView`.
  Glass {
    panel: GlassPanel,
    inner: Box<PlacedView>,
  },
  /// A glass container surface: the glass stack framed behind its
  /// children. The children mount in the glass `contentView` so they
  /// ride crisp on the glass, so they are reparented there (local-
  /// framed) instead of left as flat siblings. Kept typed (not erased to
  /// `UIView`) so the placer can reach the content view.
  GlassBackdrop(GlassPanel),
  /// A non-glass container surface (colour / image) framed behind its
  /// children; it holds no text of its own. Its children stay flat
  /// siblings layered on top — no compositing requirement.
  Backdrop(Retained<UIView>),
}

impl PlacedView {
  fn set_frame(&self, frame: CGRect) {
    match self {
      Self::Button(button) => button.setFrame(frame),
      Self::Label(label) => label.setFrame(frame),
      Self::Field(field) => field.setFrame(frame),
      Self::Switch(toggle) => toggle.setFrame(frame),
      Self::Segmented(control) => control.setFrame(frame),
      Self::Other(view) => view.setFrame(frame),
      // Frame the glass panel; the inner view fills it at local origin,
      // so the solver's geometry stays the source of truth.
      Self::Glass { panel, inner } => {
        panel.set_frame(frame);
        inner.set_frame(CGRect::new(CGPoint::new(0.0, 0.0), frame.size));
      }
      // The panel takes the solved frame; its reparented children are
      // re-framed by the placement loop in the panel's local space.
      Self::GlassBackdrop(panel) => panel.set_frame(frame),
      Self::Backdrop(view) => view.setFrame(frame),
    }
  }

  fn set_text(&self, text: &str) {
    let ns = NSString::from_str(text);

    match self {
      Self::Button(button) => {
        button.setTitle_forState(Some(&ns), UIControlState::Normal);
      }
      Self::Label(label) => label.setText(Some(&ns)),
      // A text field's content is the user's live edit — only
      // overwrite it on a real programmatic change (e.g. the
      // input clearing after Add), never echo it back mid-type.
      Self::Field(field) => {
        let current = field.text().map(|s| s.to_string()).unwrap_or_default();

        if current != text {
          field.setText(Some(&ns));
        }
      }
      Self::Other(_) => {}
      // Switch / segmented state is driven by their controls, not
      // by reconciled text.
      Self::Switch(_) | Self::Segmented(_) => {}
      // The glass wrapper holds no text of its own; defer to its
      // inner content (a label whose text the reconciler refreshes).
      Self::Glass { inner, .. } => inner.set_text(text),
      // A container surface paints no text.
      Self::GlassBackdrop(_) | Self::Backdrop(_) => {}
    }
  }
}

/// Stash the reactive wiring for the scene delegate to consume.
/// Called on the main thread before `UIApplicationMain` spins up the
/// run loop.
pub(crate) fn install(
  registry: EventRegistry,
  shared: Arc<Mutex<Vec<UiCommand>>>,
  report: Arc<Mutex<UpdateReport>>,
) {
  RUNTIME.with(|r| {
    *r.borrow_mut() = Some(Runtime {
      registry,
      shared,
      report,
      host: None,
    });
  });
}

#[cfg(target_os = "ios")]
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

#[cfg(target_os = "watchos")]
define_class!(
  // SAFETY: the superclass `NSObject` has no subclassing
  // requirements, and `AppDelegate` holds no ivars / no `Drop`.
  #[unsafe(super(NSObject))]
  #[thread_kind = MainThreadOnly]
  #[name = "ZoAppDelegate"]
  struct AppDelegate;

  unsafe impl NSObjectProtocol for AppDelegate {}

  impl AppDelegate {
    // watchOS never connects a `UIScene` to a third-party UIKit app:
    // Carousel's WatchKit scene agent instead reads the `window`
    // property off the app delegate and hosts it. So the window is
    // built here, at launch, sized to the (only) screen.
    #[unsafe(method(application:didFinishLaunchingWithOptions:))]
    fn did_finish_launching(
      &self,
      _application: &UIApplication,
      _launch_options: Option<
        &NSDictionary<UIApplicationLaunchOptionsKey, AnyObject>,
      >,
    ) -> bool {
      let mtm = MainThreadMarker::from(self);

      // The non-deprecated forms (`UIScreen` via a connected scene,
      // `UIWindow` via `init(windowScene:)`) cannot exist yet —
      // Carousel connects no scene before reading `window` — so the
      // screen-frame forms are the only way to build the window on
      // watchOS.
      #[expect(deprecated)]
      let window = {
        let bounds = UIScreen::mainScreen(mtm).bounds();

        UIWindow::initWithFrame(UIWindow::alloc(mtm), bounds)
      };

      // The event target outlives this call: `addTarget` holds it
      // weakly, so it is retained in `TARGET` for the process
      // lifetime (on iOS, UIKit retains the scene delegate instead).
      let target: Retained<SceneDelegate> =
        unsafe { msg_send![SceneDelegate::alloc(mtm), init] };

      present(&window, &target, mtm);
      TARGET.with(|t| *t.borrow_mut() = Some(target));
      WINDOW.with(|w| *w.borrow_mut() = Some(window));

      true
    }

    // Carousel's scene agent pulls the hosted window from here.
    #[unsafe(method(window))]
    fn window(&self) -> *mut UIWindow {
      WINDOW.with(|w| {
        w.borrow().as_ref().map_or(std::ptr::null_mut(), |window| {
          Retained::as_ptr(window).cast_mut()
        })
      })
    }
  }
);

#[cfg(target_os = "ios")]
define_class!(
  // SAFETY: the superclass `NSObject` has no subclassing
  // requirements, and `SceneDelegate` holds no ivars / no `Drop`.
  #[unsafe(super(NSObject))]
  #[thread_kind = MainThreadOnly]
  #[name = "ZoSceneDelegate"]
  struct SceneDelegate;

  impl SceneDelegate {
    /// `UIButton` tap → the widget's `@click` handler. The
    /// sender's `tag` is the lowering-assigned widget id.
    #[unsafe(method(buttonTapped:))]
    fn button_tapped(&self, sender: &UIButton) {
      self.dispatch_widget_event(
        sender.tag().to_string(),
        EventKind::Click,
        EventPayload::default(),
      );
    }

    /// `UISwitch` toggle → `@change`, carrying `"true"` / `"false"`.
    #[unsafe(method(switchToggled:))]
    fn switch_toggled(&self, sender: &UISwitch) {
      self.dispatch_widget_event(
        sender.tag().to_string(),
        EventKind::Change,
        EventPayload::with_value(sender.isOn().to_string()),
      );
    }

    /// Radio `UIButton` tap → `@change`. The handler closure carries
    /// which value it selects (as the desktop path), so the payload
    /// is empty.
    #[unsafe(method(radioTapped:))]
    fn radio_tapped(&self, sender: &UIButton) {
      self.dispatch_widget_event(
        sender.tag().to_string(),
        EventKind::Change,
        EventPayload::default(),
      );
    }

    /// `UISegmentedControl` change → `@change`, carrying the chosen
    /// segment's title.
    #[unsafe(method(segmentChanged:))]
    fn segment_changed(&self, sender: &UISegmentedControl) {
      let index = sender.selectedSegmentIndex();
      // `selectedSegmentIndex` is -1 when nothing is selected;
      // only a non-negative index has a title.
      let title = usize::try_from(index)
        .ok()
        .and_then(|i| sender.titleForSegmentAtIndex(i))
        .map(|s| s.to_string())
        .unwrap_or_default();

      self.dispatch_widget_event(
        sender.tag().to_string(),
        EventKind::Change,
        EventPayload::with_value(title),
      );
    }

    /// `UITextField` edit → `@input`, carrying the field's
    /// current text as the payload.
    #[unsafe(method(textChanged:))]
    fn text_changed(&self, sender: &UITextField) {
      let text = sender.text().map(|s| s.to_string()).unwrap_or_default();

      self.dispatch_widget_event(
        sender.tag().to_string(),
        EventKind::Input,
        EventPayload::with_value(text),
      );
    }

    /// `UITextField` return key → `@submit`, carrying the
    /// field's text as the payload.
    #[unsafe(method(textSubmitted:))]
    fn text_submitted(&self, sender: &UITextField) {
      let text = sender.text().map(|s| s.to_string()).unwrap_or_default();

      self.dispatch_widget_event(
        sender.tag().to_string(),
        EventKind::Submit,
        EventPayload::with_value(text),
      );
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

      present(&window, self, mtm);
      WINDOW.with(|w| *w.borrow_mut() = Some(window));
    }
  }

  // A window scene needs a `UIWindowSceneDelegate`; the `window`
  // accessors stay optional (the window is owned via `WINDOW`).
  unsafe impl UIWindowSceneDelegate for SceneDelegate {}
);

#[cfg(target_os = "watchos")]
define_class!(
  // SAFETY: the superclass `NSObject` has no subclassing
  // requirements, and `SceneDelegate` holds no ivars / no `Drop`.
  //
  // watchOS never connects a `UIScene` to this process (Carousel's
  // agent hosts the app-delegate window instead), so this class is
  // only the widget event target — same selectors, no scene
  // conformances.
  #[unsafe(super(NSObject))]
  #[thread_kind = MainThreadOnly]
  #[name = "ZoSceneDelegate"]
  struct SceneDelegate;

  impl SceneDelegate {
    /// `UIButton` tap → the widget's `@click` handler. The
    /// sender's `tag` is the lowering-assigned widget id.
    #[unsafe(method(buttonTapped:))]
    fn button_tapped(&self, sender: &UIButton) {
      self.dispatch_widget_event(
        sender.tag().to_string(),
        EventKind::Click,
        EventPayload::default(),
      );
    }

    /// `UISwitch` toggle → `@change`, carrying `"true"` / `"false"`.
    #[unsafe(method(switchToggled:))]
    fn switch_toggled(&self, sender: &UISwitch) {
      self.dispatch_widget_event(
        sender.tag().to_string(),
        EventKind::Change,
        EventPayload::with_value(sender.isOn().to_string()),
      );
    }

    /// Radio `UIButton` tap → `@change`. The handler closure carries
    /// which value it selects (as the desktop path), so the payload
    /// is empty.
    #[unsafe(method(radioTapped:))]
    fn radio_tapped(&self, sender: &UIButton) {
      self.dispatch_widget_event(
        sender.tag().to_string(),
        EventKind::Change,
        EventPayload::default(),
      );
    }

    /// `UISegmentedControl` change → `@change`, carrying the chosen
    /// segment's title.
    #[unsafe(method(segmentChanged:))]
    fn segment_changed(&self, sender: &UISegmentedControl) {
      let index = sender.selectedSegmentIndex();
      // `selectedSegmentIndex` is -1 when nothing is selected;
      // only a non-negative index has a title.
      let title = usize::try_from(index)
        .ok()
        .and_then(|i| sender.titleForSegmentAtIndex(i))
        .map(|s| s.to_string())
        .unwrap_or_default();

      self.dispatch_widget_event(
        sender.tag().to_string(),
        EventKind::Change,
        EventPayload::with_value(title),
      );
    }

    /// `UITextField` edit → `@input`, carrying the field's
    /// current text as the payload.
    #[unsafe(method(textChanged:))]
    fn text_changed(&self, sender: &UITextField) {
      let text = sender.text().map(|s| s.to_string()).unwrap_or_default();

      self.dispatch_widget_event(
        sender.tag().to_string(),
        EventKind::Input,
        EventPayload::with_value(text),
      );
    }

    /// `UITextField` return key → `@submit`, carrying the
    /// field's text as the payload.
    #[unsafe(method(textSubmitted:))]
    fn text_submitted(&self, sender: &UITextField) {
      let text = sender.text().map(|s| s.to_string()).unwrap_or_default();

      self.dispatch_widget_event(
        sender.tag().to_string(),
        EventKind::Submit,
        EventPayload::with_value(text),
      );
    }
  }

  unsafe impl NSObjectProtocol for SceneDelegate {}
);

/// Build the root view hierarchy into `window` and bring it on
/// screen: a container sized to the window's bounds (the solve's
/// coordinate space — centring comes from the root's justify/align),
/// the first render against the shared command stream, and the view
/// host stored for reconciliation. One container owns every widget by
/// frame — no `UIStackView`, no Auto-Layout. Shared by the iOS scene
/// connect and the watchOS launch path.
fn present(window: &UIWindow, target: &SceneDelegate, mtm: MainThreadMarker) {
  let controller = UIViewController::new(mtm);

  let cmds = RUNTIME.with(|r| {
    r.borrow()
      .as_ref()
      .map(|rt| rt.shared.lock().unwrap().clone())
      .unwrap_or_default()
  });

  let bounds = window.bounds();
  let container = UIView::initWithFrame(UIView::alloc(mtm), bounds);

  // The backdrop (body colour / image) is painted by `render_into`.
  controller.setView(Some(&container));

  let (tree, views) = render_into(&cmds, &container, target, mtm);

  RUNTIME.with(|r| {
    if let Some(rt) = r.borrow_mut().as_mut() {
      rt.host = Some(ViewHost {
        container,
        tree,
        views,
      });
    }
  });

  window.setRootViewController(Some(&controller));
  window.makeKeyAndVisible();
}

impl SceneDelegate {
  /// Resolve `(widget_id, kind)` to its handler, dispatch into
  /// compiled zo (which mutates state and refreshes `shared`),
  /// then reconcile the view tree against the new command stream.
  /// Shared by the button-tap and text-field event selectors.
  fn dispatch_widget_event(
    &self,
    widget_id: String,
    kind: EventKind,
    payload: EventPayload,
  ) {
    RUNTIME.with(|r| {
      let mut runtime = r.borrow_mut();
      let Some(runtime) = runtime.as_mut() else {
        return;
      };

      // Resolve the handler under a short lock, then drop it
      // before dispatching: the registry callback re-locks
      // `shared` to refresh bindings, so holding it here would
      // deadlock. Rebuild the event map per event (cheap, n tiny)
      // exactly as the desktop runtime does per frame.
      let handler = {
        let cmds = runtime.shared.lock().unwrap();

        build_event_map(&cmds)
          .get(&(widget_id.clone(), kind))
          .cloned()
      };

      if let Some(handler) = handler {
        runtime.registry.dispatch(&handler, &payload);
      }

      // The handler mutated `shared` in place. The registry's
      // report says exactly what happened: a structural rebuild
      // (list slot written — indices shifted) falls back to the
      // full diff; otherwise `apply_dirty` patches O(dirty)
      // placements with no structural scan and no stream clone.
      let (structural, appended) = {
        let report = runtime.report.lock().unwrap();

        (report.structural, report.appended)
      };

      // Radio group exclusivity: a tapped radio takes the filled
      // glyph and clears its same-`name` siblings immediately — a
      // plain `UIButton` does not self-toggle the way UISwitch /
      // UISegmentedControl do. A no-op for non-radio widgets.
      if kind == EventKind::Change {
        let cmds = runtime.shared.lock().unwrap().clone();

        if let Some(host) = runtime.host.as_mut() {
          select_radio_in_group(host, &cmds, &widget_id);
        }
      }

      let Some(host) = runtime.host.as_mut() else {
        return;
      };

      // The push fast path: append only the new items' views —
      // existing placements keep their views and just re-frame.
      let appended_range = appended.and_then(|block| {
        let cmds = runtime.shared.lock().unwrap();

        host.tree.append_list_items(block.at, block.count, &cmds)
      });

      let patched = if let Some(new_placements) = appended_range {
        let cmds = runtime.shared.lock().unwrap().clone();
        let bounds = host.container.bounds();
        let rects = host
          .tree
          .solve((bounds.size.width as f32, bounds.size.height as f32));
        let styles = host.tree.styles().to_vec();
        let authors = host.tree.authors().to_vec();
        let parents = host.tree.parents().to_vec();
        let images = host.tree.images().to_vec();
        let mtm = MainThreadMarker::from(self);
        let builder = ViewBuilder::new(&cmds, self, mtm, &images);

        for i in new_placements {
          let (superview, frame) = attach_target(
            &host.container,
            &host.views,
            parents[i],
            &rects,
            rects[i].1,
          );

          let placement = Placement {
            superview: &superview,
            frame,
            style: &styles[i],
            author: &authors[i],
          };

          host.views.push(builder.place(rects[i].0, &placement));
        }

        // Existing widgets re-frame below via the fast path; no
        // text changed, so the patch list is empty.
        Some(Vec::new())
      } else if structural {
        None
      } else {
        let cmds = runtime.shared.lock().unwrap();
        let report = runtime.report.lock().unwrap();

        Some(host.tree.apply_dirty(&report.touched, &cmds))
      };

      let reconciled = match patched {
        Some(changed) => Some(changed),
        None => {
          let cmds = runtime.shared.lock().unwrap().clone();

          host.tree.reconcile(&cmds)
        }
      };

      match reconciled {
        // Fast path: structure unchanged. Re-solve (Taffy only
        // re-measures the dirtied leaves), re-frame every view,
        // and rewrite text on just the changed ones.
        Some(changed) => {
          let bounds = host.container.bounds();
          let rects = host
            .tree
            .solve((bounds.size.width as f32, bounds.size.height as f32));
          // Re-frame in the same space `render_into` placed each view:
          // a nested glass child stays in its effect view's local
          // coordinates, so offset it by the parent's origin.
          let parents = host.tree.parents();

          for (index, (view, (_, rect))) in
            host.views.iter().zip(&rects).enumerate()
          {
            view.set_frame(local_frame(parents[index], &rects, *rect));
          }

          for (index, text) in changed {
            host.views[index].set_text(&text);
          }
        }
        // Structure changed (items added/removed): rebuild the
        // container's widgets once from the new stream.
        None => {
          clear_subviews(&host.container);

          let cmds = runtime.shared.lock().unwrap().clone();
          let mtm = MainThreadMarker::from(self);
          let (tree, views) = render_into(&cmds, &host.container, self, mtm);

          host.tree = tree;
          host.views = views;
        }
      }
    });
  }
}

/// Builds native views for solved leaves and frames them onto the
/// superview each `Placement` names. Stateless beyond its borrows —
/// `place` returns the created `PlacedView` so the host keeps it for
/// reconciliation.
struct ViewBuilder<'a> {
  cmds: &'a [UiCommand],
  target: &'a SceneDelegate,
  mtm: MainThreadMarker,
  /// Stylesheet image catalog — a container's `background_image`
  /// handle indexes it (same catalog the root backdrop reads).
  images: &'a [String],
}

/// The per-leaf placement inputs for `place`: where to attach the
/// native view, the frame in that superview's coordinate space, and
/// the resolved + author styles that drive its paint. Bundled so
/// `place` stays within the argument budget.
struct Placement<'a> {
  /// The superview to attach to — the top-level container for a flat
  /// leaf, or a glass effect view's `contentView` for a nested one.
  superview: &'a UIView,
  /// The frame in `superview`'s local space (already offset by the
  /// parent's origin when nested).
  frame: CGRect,
  style: &'a ComputedStyle,
  author: &'a StylePatch,
}

impl<'a> ViewBuilder<'a> {
  fn new(
    cmds: &'a [UiCommand],
    target: &'a SceneDelegate,
    mtm: MainThreadMarker,
    images: &'a [String],
  ) -> Self {
    Self {
      cmds,
      target,
      mtm,
      images,
    }
  }

  /// Build, frame, and attach the native view for the placed leaf at
  /// `commands[index]`. `Element{Button}` → `UIButton` titled by its
  /// collapsed text and tagged with its lowering widget id (wired to
  /// `buttonTapped:`); a text tag or free-standing `Text` → `UILabel`.
  /// Other leaves (image, input) get a bare reserved view. The
  /// `placement` says where to attach (top-level container or a glass
  /// effect view's `contentView`), the local frame, and the styles.
  fn place(&self, index: usize, placement: &Placement) -> PlacedView {
    let &Placement {
      superview,
      frame,
      style,
      author,
    } = placement;

    match &self.cmds[index] {
      UiCommand::Element { tag, attrs, .. } if *tag == ElementTag::Button => {
        let button = UIButton::buttonWithType(UIButtonType::System, self.mtm);
        let title = NSString::from_str(&collapse_text(self.cmds, index + 1));

        button.setTitle_forState(Some(&title), UIControlState::Normal);

        if let Some(label) = button.titleLabel() {
          // SAFETY: a system font of a positive size is always valid.
          unsafe { label.setFont(Some(&font_of(style))) };
        }

        match glass_of(style) {
          // A glass `UIButton.Configuration` owns the material, the
          // rounded shape, and the interactive tap highlight. A
          // declared `background` tints via the button's `tintColor`.
          Some(glass) => {
            button.setConfiguration(Some(&glass_button_config(
              glass, author, self.mtm,
            )));

            if author.background.is_some() {
              // SAFETY: a non-nil colour on the live main-thread button
              // drives the tinted-glass configuration's tint.
              unsafe {
                button.setTintColor(Some(&ui_color(style.background)));
              }
            }
          }
          // Solid: declared `color` recolours the title (else the
          // system tint stands); declared `background` fills the box.
          None => {
            if author.color.is_some() {
              button.setTitleColor_forState(
                Some(&ui_color(style.color)),
                UIControlState::Normal,
              );
            }

            if author.background.is_some() {
              button.setBackgroundColor(Some(&ui_color(style.background)));
            }
          }
        }

        // The `data-id` carries the widget id the `Event` command
        // references; stash it in `tag` so `buttonTapped:` can route
        // back to the handler.
        if let Some(id) = widget_id(attrs) {
          button.setTag(id);
        }

        // SAFETY: the scene delegate is live, and `buttonTapped:` is a
        // registered selector taking the sender.
        let target_object: &AnyObject = self.target.as_ref();

        unsafe {
          button.addTarget_action_forControlEvents(
            Some(target_object),
            sel!(buttonTapped:),
            UIControlEvents::TouchUpInside,
          );
        }

        button.setFrame(frame);
        superview.addSubview(&button);

        PlacedView::Button(button)
      }

      // `<input type="checkbox">` -> `UISwitch`.
      UiCommand::Element {
        tag: ElementTag::Input,
        attrs,
        ..
      } if attr_text(attrs, "type").as_deref() == Some("checkbox") => {
        let toggle = UISwitch::initWithFrame(UISwitch::alloc(self.mtm), frame);

        toggle.setOn(attr_bool(attrs, "checked"));

        if let Some(id) = widget_id(attrs) {
          toggle.setTag(id);
        }

        // SAFETY: the scene delegate is live; `switchToggled:` is a
        // registered selector taking the sender. `ValueChanged`
        // fires on each toggle (-> `@change`).
        let target_object: &AnyObject = self.target.as_ref();

        unsafe {
          toggle.addTarget_action_forControlEvents(
            Some(target_object),
            sel!(switchToggled:),
            UIControlEvents::ValueChanged,
          );
        }

        superview.addSubview(&toggle);

        PlacedView::Switch(toggle)
      }

      // `<input type="radio">` -> a `UIButton` toggle. iOS has no
      // native radio; the button shows the filled/empty glyph from
      // `checked` and fires `@change` on tap (the handler closure
      // carries which value it selects, like the desktop path).
      UiCommand::Element {
        tag: ElementTag::Input,
        attrs,
        ..
      } if attr_text(attrs, "type").as_deref() == Some("radio") => {
        let button = UIButton::buttonWithType(UIButtonType::System, self.mtm);
        let glyph = if attr_bool(attrs, "checked") {
          RADIO_SELECTED
        } else {
          RADIO_UNSELECTED
        };

        button.setTitle_forState(
          Some(&NSString::from_str(glyph)),
          UIControlState::Normal,
        );

        if let Some(id) = widget_id(attrs) {
          button.setTag(id);
        }

        // SAFETY: the scene delegate is live; `radioTapped:` is a
        // registered selector taking the sender.
        let target_object: &AnyObject = self.target.as_ref();

        unsafe {
          button.addTarget_action_forControlEvents(
            Some(target_object),
            sel!(radioTapped:),
            UIControlEvents::TouchUpInside,
          );
        }

        button.setFrame(frame);
        superview.addSubview(&button);

        PlacedView::Button(button)
      }

      // `<select>` -> `UISegmentedControl` over its `<option>`
      // children (gathered from the stream; Select is a layout leaf,
      // so its options are not placed separately).
      UiCommand::Element {
        tag: ElementTag::Select,
        attrs,
        ..
      } => {
        let options = gather_options(self.cmds, index);
        let items = NSArray::from_retained_slice(
          &options
            .iter()
            .map(|o| NSString::from_str(o))
            .collect::<Vec<_>>(),
        );

        // `initWithItems:` takes an untyped `&NSArray`; cast the
        // typed `NSArray<NSString>` (a no-op reinterpret — every
        // element is already an object). Both this and the init are
        // unsafe per objc2's FFI contract.
        let items: Retained<NSArray> =
          unsafe { Retained::cast_unchecked(items) };
        let control = unsafe {
          UISegmentedControl::initWithItems(
            UISegmentedControl::alloc(self.mtm),
            Some(&items),
          )
        };
        let value = attr_text(attrs, "value").unwrap_or_default();
        let selected = options.iter().position(|o| *o == value).unwrap_or(0);

        control.setSelectedSegmentIndex(selected as isize);
        control.setFrame(frame);

        if let Some(id) = widget_id(attrs) {
          control.setTag(id);
        }

        // SAFETY: the scene delegate is live; `segmentChanged:` is a
        // registered selector taking the sender.
        let target_object: &AnyObject = self.target.as_ref();

        unsafe {
          control.addTarget_action_forControlEvents(
            Some(target_object),
            sel!(segmentChanged:),
            UIControlEvents::ValueChanged,
          );
        }

        superview.addSubview(&control);

        PlacedView::Segmented(control)
      }

      UiCommand::Element {
        tag: ElementTag::Input | ElementTag::Textarea,
        attrs,
        ..
      } => {
        let field =
          UITextField::initWithFrame(UITextField::alloc(self.mtm), frame);

        // A rounded border makes the (otherwise borderless) field
        // visible, matching the desktop renderer's box.
        field.setBorderStyle(UITextBorderStyle::RoundedRect);
        field.setFont(Some(&font_of(style)));

        if let Some(placeholder) = attr_text(attrs, "placeholder") {
          field.setPlaceholder(Some(&NSString::from_str(&placeholder)));
        }

        if let Some(value) = attr_text(attrs, "value") {
          field.setText(Some(&NSString::from_str(&value)));
        }

        // Same widget-id routing as the button — `textChanged:` /
        // `textSubmitted:` read it back from `tag`.
        if let Some(id) = widget_id(attrs) {
          field.setTag(id);
        }

        // SAFETY: the scene delegate is live; `textChanged:` /
        // `textSubmitted:` are registered selectors taking the
        // sender. `EditingChanged` fires per edit (→ `@input`);
        // `EditingDidEndOnExit` fires on the return key
        // (→ `@submit`).
        let target_object: &AnyObject = self.target.as_ref();

        unsafe {
          field.addTarget_action_forControlEvents(
            Some(target_object),
            sel!(textChanged:),
            UIControlEvents::EditingChanged,
          );
          field.addTarget_action_forControlEvents(
            Some(target_object),
            sel!(textSubmitted:),
            UIControlEvents::EditingDidEndOnExit,
          );
        }

        superview.addSubview(&field);

        PlacedView::Field(field)
      }

      UiCommand::Element { tag, .. } if tag.is_text_tag() => {
        self.label(&collapse_text(self.cmds, index + 1), placement)
      }

      UiCommand::Text(content) => self.label(content, placement),

      // `<img>` is a reserved leaf — iOS image painting is a separate
      // gap; it keeps its solved geometry but paints nothing yet.
      UiCommand::Element { tag, .. } if *tag == ElementTag::Img => {
        let view = UIView::initWithFrame(UIView::alloc(self.mtm), frame);

        superview.addSubview(&view);

        PlacedView::Other(view)
      }

      // Any other placed `Element` is a paintable container — layout
      // placed it only because it declared a surface. Paint that
      // surface as a backmost sibling; its children sit on top (a glass
      // surface instead nests them in `contentView` — see the loop).
      UiCommand::Element { .. } => self.container_backdrop(placement),

      _ => {
        let view = UIView::initWithFrame(UIView::alloc(self.mtm), frame);

        superview.addSubview(&view);

        PlacedView::Other(view)
      }
    }
  }

  /// Build, frame, and attach a `UILabel` rendered at the style's
  /// font + colour, so its text fits the box the solver measured and
  /// matches the cascade. A declared `background` fills the label.
  /// Attaches to `placement.superview` (the top-level container, or a
  /// glass effect view's `contentView` when the label nests in glass).
  fn label(&self, text: &str, placement: &Placement) -> PlacedView {
    let &Placement {
      superview,
      frame,
      style,
      author,
    } = placement;
    let label = UILabel::new(self.mtm);

    label.setText(Some(&NSString::from_str(text)));
    // SAFETY: a system font of a positive size, and a non-nil colour.
    unsafe {
      label.setFont(Some(&font_of(style)));
      label.setTextColor(Some(&ui_color(style.color)));
    }

    match glass_of(style) {
      // Glass label: it rides on the panel's glass content, crisp on the
      // glass that refracts whatever sits behind it.
      Some(glass) => {
        let panel = glass_panel(glass, self.mtm);

        panel.content().addSubview(&label);
        panel.set_frame(frame);
        label.setFrame(CGRect::new(CGPoint::new(0.0, 0.0), frame.size));
        superview.addSubview(&panel.outer);

        PlacedView::Glass {
          panel,
          inner: Box::new(PlacedView::Label(label)),
        }
      }
      // Solid: a declared `background` fills the label directly.
      None => {
        if author.background.is_some() {
          label.setBackgroundColor(Some(&ui_color(style.background)));
        }

        label.setFrame(frame);
        superview.addSubview(&label);

        PlacedView::Label(label)
      }
    }
  }

  /// Build a paintable container's surface as a backmost sibling at
  /// `placement.frame` — glass, then a background image, then a
  /// declared colour. Layout places a container only when it declared
  /// one of these. A glass surface returns the typed effect view
  /// (`GlassBackdrop`) so the placement loop can reparent its children
  /// into `contentView`; a colour / image surface stays a flat sibling.
  fn container_backdrop(&self, placement: &Placement) -> PlacedView {
    let &Placement {
      superview,
      frame,
      style,
      author,
    } = placement;

    // Glass first: a glass card frosts the surface behind it. Keep the
    // panel typed so its children reparent into the glass content.
    if let Some(glass) = glass_of(style) {
      let panel = glass_panel(glass, self.mtm);

      panel.set_frame(frame);
      superview.addSubview(&panel.outer);

      return PlacedView::GlassBackdrop(panel);
    }

    // A background image fills the card behind its content.
    if let Some(id) = style.background_image
      && let Some(url) = self.images.get(id as usize)
      && let Some(image) = load_ui_image(url)
    {
      let view = backdrop_view(&image, frame, self.mtm);

      superview.addSubview(&view);

      return PlacedView::Backdrop(view.into_super());
    }

    // A declared colour fills the card.
    let view = UIView::initWithFrame(UIView::alloc(self.mtm), frame);

    if author.background.is_some() {
      view.setBackgroundColor(Some(&ui_color(style.background)));
    }

    superview.addSubview(&view);

    PlacedView::Backdrop(view)
  }
}

/// The system font sized to a computed style, so native text renders
/// at the same `font_size` the deterministic measure assumed.
fn font_of(style: &ComputedStyle) -> Retained<UIFont> {
  UIFont::systemFontOfSize(style.font_size as f64)
}

/// A target-agnostic `Rgba` → UIKit `UIColor` (components 0–1).
fn ui_color(color: Rgba) -> Retained<UIColor> {
  UIColor::colorWithRed_green_blue_alpha(
    color.r as f64 / 255.0,
    color.g as f64 / 255.0,
    color.b as f64 / 255.0,
    color.a as f64 / 255.0,
  )
}

/// Radio-button glyph when selected — a filled circle (●, U+25CF).
/// iOS has no native radio control, so the `UIButton` toggle shows
/// this in place of a dot.
const RADIO_SELECTED: &str = "\u{25cf}";

/// Radio-button glyph when unselected — a hollow circle (○, U+25CB).
const RADIO_UNSELECTED: &str = "\u{25cb}";

/// Corner radius (pt) of a glass panel until a `border-radius` property
/// exists. The glass material is clipped to this rounded box.
#[cfg(target_os = "ios")]
const GLASS_CORNER_RADIUS: CGFloat = 16.0;

/// Width (pt) of the specular rim around a glass panel — the thin bright
/// edge that defines the pane even when it is near-transparent.
#[cfg(target_os = "ios")]
const GLASS_RIM_WIDTH: CGFloat = 1.0;

/// Opacity (0–1) of the white specular rim.
#[cfg(target_os = "ios")]
const GLASS_RIM_ALPHA: CGFloat = 0.3;

/// White-tint opacity (0–1) of the Simulator's translucent stand-in for
/// a `Clear` glass — barely there, so the backdrop reads through sharp.
const GLASS_TINT_ALPHA_CLEAR: CGFloat = 0.1;

/// White-tint opacity (0–1) for a `Regular` glass in the Simulator — a
/// frosted veil that still lets the backdrop show.
const GLASS_TINT_ALPHA_REGULAR: CGFloat = 0.18;

/// Opacity (0–1) of the soft drop shadow that lifts a glass panel off
/// the backdrop.
#[cfg(target_os = "ios")]
const GLASS_SHADOW_OPACITY: c_float = 0.18;

/// Blur radius (pt) of the panel's drop shadow — wide and soft, not a
/// hard edge.
#[cfg(target_os = "ios")]
const GLASS_SHADOW_RADIUS: CGFloat = 16.0;

/// Downward offset (pt) of the panel's drop shadow, so the light reads
/// as coming from above.
#[cfg(target_os = "ios")]
const GLASS_SHADOW_OFFSET_Y: CGFloat = 8.0;

/// The glass style this element asks for, but only when the OS can
/// render it. `UIGlassEffect` and the glass `UIButton.Configuration`s
/// are iOS 26+ and the deployment target is 15.0, so every glass path
/// funnels through this guard. watchOS has no Liquid Glass surface
/// (and no public CALayer access for the rim/shadow styling), so
/// glass resolves to solid there.
fn glass_of(style: &ComputedStyle) -> Option<GlassStyle> {
  if cfg!(target_os = "watchos") {
    return None;
  }

  match style.material {
    Material::Glass(glass) if glass_available() => Some(glass),
    _ => None,
  }
}

/// `true` on iOS 26+, where the glass APIs exist. Cached — the OS
/// version cannot change mid-process.
fn glass_available() -> bool {
  static AVAILABLE: OnceLock<bool> = OnceLock::new();

  *AVAILABLE.get_or_init(|| {
    let version = NSOperatingSystemVersion {
      majorVersion: 26,
      minorVersion: 0,
      patchVersion: 0,
    };

    NSProcessInfo::processInfo().isOperatingSystemAtLeastVersion(version)
  })
}

/// The glass `UIButton.Configuration` for a style: clear glass for the
/// `Clear` variant, tinted glass when a `background` is declared, plain
/// glass otherwise. The configuration owns the shape + tap highlight.
fn glass_button_config(
  glass: GlassStyle,
  author: &StylePatch,
  mtm: MainThreadMarker,
) -> Retained<UIButtonConfiguration> {
  match (glass, author.background.is_some()) {
    (GlassStyle::Clear, _) => {
      UIButtonConfiguration::clearGlassButtonConfiguration(mtm)
    }
    (GlassStyle::Regular, true) => {
      UIButtonConfiguration::tintedGlassButtonConfiguration(mtm)
    }
    (GlassStyle::Regular, false) => {
      UIButtonConfiguration::glassButtonConfiguration(mtm)
    }
  }
}

/// A Liquid Glass panel: a shadow-bearing outer host wrapping a glass
/// `surface` (genuine `UIGlassEffect` on device, a translucent tinted
/// view in the Simulator — see `glass_surface`). Built by `glass_panel`;
/// the caller frames it with `set_frame` and attaches leaves to
/// `content`, where they ride crisp on the glass.
struct GlassPanel {
  /// The shadow host, framed by the solver — kept un-clipped so the
  /// soft drop shadow extends past the rounded glass panel.
  outer: Retained<UIView>,
  /// The glass surface, rounded + clipped + rimmed; fills `outer`.
  surface: Retained<UIView>,
  /// The view the panel's leaves attach to: the effect view's content
  /// view on device, the `surface` itself in the Simulator.
  host: Retained<UIView>,
}

impl GlassPanel {
  /// Frame the whole stack: `outer` takes the solved rect and the
  /// surface fills it at local origin. No Auto-Layout — the solver's
  /// geometry stays the single source of truth.
  fn set_frame(&self, frame: CGRect) {
    let local = CGRect::new(CGPoint::new(0.0, 0.0), frame.size);

    self.outer.setFrame(frame);
    self.surface.setFrame(local);
  }

  /// The view a panel's leaves attach to, so labels and buttons render
  /// crisp on the glass.
  fn content(&self) -> Retained<UIView> {
    self.host.clone()
  }
}

/// Build a `GlassPanel` for a style: a glass surface (see
/// `glass_surface`) rounded, clipped, and rimmed, lifted by a soft drop
/// shadow on the un-clipped outer host.
fn glass_panel(glass: GlassStyle, mtm: MainThreadMarker) -> GlassPanel {
  let (surface, host) = glass_surface(glass, mtm);
  let outer = UIView::new(mtm);

  // The rim + shadow styling needs CALayer access, which the UIKit
  // bindings expose only off watchOS. `glass_of` never yields glass on
  // watchOS, so the un-styled fallback there is dead code that merely
  // has to compile.
  #[cfg(target_os = "ios")]
  {
    // Round the panel, clip to the rounded box, then draw a thin white
    // rim — the specular edge that defines the pane even when
    // near-transparent.
    let layer = surface.layer();

    layer.setCornerRadius(GLASS_CORNER_RADIUS);
    layer.setMasksToBounds(true);
    layer.setBorderWidth(GLASS_RIM_WIDTH);

    // SAFETY: `CGColor` bridges the colour on the main thread; the rim
    // is a plain translucent white.
    let rim =
      unsafe { UIColor::colorWithWhite_alpha(1.0, GLASS_RIM_ALPHA).CGColor() };

    layer.setBorderColor(Some(&rim));

    // The outer host carries the drop shadow. It must NOT clip to
    // bounds (the surface does), so the soft shadow spreads past the
    // rounded box.
    let outer_layer = outer.layer();

    // SAFETY: `CGColor` bridges the colour on the main thread; the
    // shadow is opaque black, faded by `setShadowOpacity`.
    let shadow = unsafe { UIColor::colorWithWhite_alpha(0.0, 1.0).CGColor() };

    outer_layer.setShadowColor(Some(&shadow));
    outer_layer.setShadowOpacity(GLASS_SHADOW_OPACITY);
    outer_layer.setShadowRadius(GLASS_SHADOW_RADIUS);
    outer_layer.setShadowOffset(CGSize::new(0.0, GLASS_SHADOW_OFFSET_Y));
  }

  outer.addSubview(&surface);

  GlassPanel {
    outer,
    surface,
    host,
  }
}

/// The glass surface and the view its leaves attach to — genuine Liquid
/// Glass on device, a translucent tinted view in the Simulator.
///
/// `UIGlassEffect` (and any `UIVisualEffectView`) refracts its backdrop
/// by reading the framebuffer in a fragment shader. The Simulator's
/// Metal stack — an Apple-family-2 GPU — has no programmable blending, so
/// a shader can't read a color attachment, and the effect renders as an
/// opaque fill (white or black, by appearance) that hides the backdrop.
/// A plain translucent view instead lets the backdrop photo show through
/// by ordinary alpha compositing — the honest stand-in until a device
/// renders the real glass with live lensing.
fn glass_surface(
  glass: GlassStyle,
  mtm: MainThreadMarker,
) -> (Retained<UIView>, Retained<UIView>) {
  if is_simulator() {
    // `Clear` is barely tinted (the backdrop reads through sharp);
    // `Regular` carries a frosted veil. Leaves mount on the view itself.
    let alpha = match glass {
      GlassStyle::Clear => GLASS_TINT_ALPHA_CLEAR,
      GlassStyle::Regular => GLASS_TINT_ALPHA_REGULAR,
    };
    let view = UIView::new(mtm);

    view.setBackgroundColor(Some(&UIColor::colorWithWhite_alpha(1.0, alpha)));

    return (view.clone(), view);
  }

  let style = match glass {
    GlassStyle::Clear => UIGlassEffectStyle::Clear,
    GlassStyle::Regular => UIGlassEffectStyle::Regular,
  };
  let effect = UIGlassEffect::effectWithStyle(style, mtm);

  // Interactive glass reacts to touches with the system's fluid
  // highlight — the card holds buttons, so let it feel live.
  effect.setInteractive(true);

  let effect_view = UIVisualEffectView::initWithEffect(
    UIVisualEffectView::alloc(mtm),
    Some(&effect),
  );
  // Leaves mount in the effect view's content view; the surface is the
  // effect view itself, upcast so both targets share one panel shape.
  let host = effect_view.contentView();

  (effect_view.into_super(), host)
}

/// `true` inside the iOS Simulator, which exports `SIMULATOR_UDID` into
/// every app's environment. The glass path reads this to substitute a
/// translucent view for `UIGlassEffect`, which the Simulator cannot
/// composite.
fn is_simulator() -> bool {
  std::env::var_os("SIMULATOR_UDID").is_some()
}

/// Load a `UIImage` from a catalog ref. The bytes come through the
/// one shared loader (`zo-runtime-render::asset`) that egui uses too —
/// local file or URL — after resolving the ref to a readable path;
/// UIKit decodes them. `None` on any failure — a missing backdrop
/// must never crash the app.
fn load_ui_image(src: &str) -> Option<Retained<UIImage>> {
  let path = resolve_asset_path(src);
  let bytes = load_image_bytes(&path).ok()?;
  let data = NSData::from_vec(bytes);

  UIImage::imageWithData(&data)
}

/// Map a catalog ref to a path the loader can read: a URL is left
/// alone, an absolute file that exists is left alone (Simulator /
/// desktop parity), else the asset's basename inside the app bundle —
/// where a `--target=ios` build copied it (the device-correct home).
fn resolve_asset_path(src: &str) -> String {
  if src.starts_with("http://") || src.starts_with("https://") {
    return src.to_string();
  }

  let path = Path::new(src);

  if path.is_absolute() && path.exists() {
    return src.to_string();
  }

  let basename = src.rsplit('/').next().unwrap_or(src);

  bundle_resource_path(basename).unwrap_or_else(|| src.to_string())
}

/// `<App.app>/<name>` via the main bundle's resource directory (the
/// bundle root on iOS), where the bundler placed copied assets.
fn bundle_resource_path(name: &str) -> Option<String> {
  let resource_path = NSBundle::mainBundle().resourcePath()?.to_string();

  Some(format!("{resource_path}/{name}"))
}

/// A full-bounds `UIImageView` for the container backdrop: aspect-fill
/// so the image covers the screen, clipped so it never overflows.
fn backdrop_view(
  image: &UIImage,
  bounds: CGRect,
  mtm: MainThreadMarker,
) -> Retained<UIImageView> {
  let view = UIImageView::initWithImage(UIImageView::alloc(mtm), Some(image));

  view.setFrame(bounds);
  view.setContentMode(UIViewContentMode::ScaleAspectFill);
  view.setClipsToBounds(true);

  view
}

/// Solve `cmds` against the container's bounds and place a native view
/// per leaf, returning the persistent tree + view list the host
/// reconciles against. Paints the root backdrop first (backmost).
fn render_into(
  cmds: &[UiCommand],
  container: &UIView,
  target: &SceneDelegate,
  mtm: MainThreadMarker,
) -> (LayoutTree, Vec<PlacedView>) {
  let bounds = container.bounds();
  let mut tree = LayoutTree::build(cmds);

  // Paint the container backdrop from the `body` rule. With a backdrop
  // image, keep the container itself CLEAR and let the photo be the
  // only thing behind a glass surface — an opaque container colour is
  // what a `UIVisualEffectView` samples, so a white container makes the
  // glass read solid white instead of refracting the photo.
  let root_style = tree.root_style();

  if let Some(id) = root_style.background_image
    && let Some(url) = tree.images().get(id as usize)
    && let Some(image) = load_ui_image(url)
  {
    container.setBackgroundColor(None);
    container.addSubview(&backdrop_view(&image, bounds, mtm));
  } else {
    container.setBackgroundColor(Some(&ui_color(root_style.background)));
  }

  let rects = tree.solve((bounds.size.width as f32, bounds.size.height as f32));

  // Styles, author patches, and nesting parents parallel the solved
  // leaves; clone so the tree is free to move into the host alongside
  // the views. The image catalog rides along so a container backdrop
  // can resolve its handle.
  let styles = tree.styles().to_vec();
  let authors = tree.authors().to_vec();
  let parents = tree.parents().to_vec();
  let images = tree.images().to_vec();
  let builder = ViewBuilder::new(cmds, target, mtm, &images);

  // Place in solve order so a glass container always exists in `views`
  // before its children (layout records a paintable container before
  // the leaves it wraps). A nested child attaches to its parent glass
  // effect view's `contentView`, framed in that view's local space.
  let mut views: Vec<PlacedView> = Vec::with_capacity(rects.len());

  for (i, (index, rect)) in rects.iter().enumerate() {
    let (superview, frame) =
      attach_target(container, &views, parents[i], &rects, *rect);

    let placement = Placement {
      superview: &superview,
      frame,
      style: &styles[i],
      author: &authors[i],
    };

    views.push(builder.place(*index, &placement));
  }

  (tree, views)
}

/// Resolve where a placed leaf attaches and at what frame. A leaf with
/// no nesting parent sits on `container` at its absolute rect. A leaf
/// nested in a glass container attaches to that container's effect view
/// `contentView`, framed in the effect view's local space. A nesting
/// parent is always placed before the leaf, so `views[p]` already
/// exists.
///
/// Layout records a nesting `parent` only for a glass container (the
/// one surface UIKit composites its content into), so `parents[i]` is
/// `Some` exactly when `views[p]` is a `GlassBackdrop`. The same
/// invariant lets `reconcile` re-frame from `parents` alone, without
/// the view types.
fn attach_target(
  container: &UIView,
  views: &[PlacedView],
  parent: Option<usize>,
  rects: &[(usize, Rect)],
  rect: Rect,
) -> (Retained<UIView>, CGRect) {
  let frame = local_frame(parent, rects, rect);

  match parent {
    Some(p) => match &views[p] {
      PlacedView::GlassBackdrop(panel) => (panel.content(), frame),
      // Layout never nests a leaf under a non-glass surface, so this
      // would be a builder/runtime mismatch — fail loud rather than
      // misplace against an offset frame.
      _ => panic!("nesting parent {p} is not a glass container"),
    },
    None => (Retained::from(container), frame),
  }
}

/// The frame a placed leaf is drawn at: its absolute rect when flat,
/// or — when nested in a glass container — its rect offset into that
/// container's effect view local space (absolute rect minus the
/// parent's absolute origin). One source of truth for placement and
/// reconcile so a re-frame matches the original attach.
fn local_frame(
  parent: Option<usize>,
  rects: &[(usize, Rect)],
  rect: Rect,
) -> CGRect {
  match parent {
    Some(p) => {
      let parent_rect = rects[p].1;

      frame_of(Rect {
        x: rect.x - parent_rect.x,
        y: rect.y - parent_rect.y,
        width: rect.width,
        height: rect.height,
      })
    }
    None => frame_of(rect),
  }
}

/// Detach every child view so a structural rebuild starts clean.
fn clear_subviews(container: &UIView) {
  for view in container.subviews().iter() {
    view.removeFromSuperview();
  }
}

/// A solved `Rect` → UIKit `CGRect`.
fn frame_of(rect: Rect) -> CGRect {
  CGRect::new(
    CGPoint::new(rect.x as f64, rect.y as f64),
    CGSize::new(rect.width as f64, rect.height as f64),
  )
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

/// The `data-id` as a string, regardless of whether it was stored
/// as a number (`<input>` ids) or a string (`select_N` / tag ids).
/// `attr_text` alone misses the numeric form. Used to match a
/// command against a tapped widget's `tag().to_string()`.
fn widget_id_str(attrs: &[Attr]) -> Option<String> {
  widget_id(attrs).map(|id| id.to_string())
}

/// The string value of attribute `name` (a `Prop` or `Dynamic`),
/// used to seed a text field's placeholder / initial value.
fn attr_text(attrs: &[Attr], name: &str) -> Option<String> {
  attrs
    .iter()
    .find(|a| a.name() == name)
    .and_then(|a| a.as_str())
    .map(str::to_string)
}

/// Read a boolean attribute: present-and-truthy. A bare `checked`
/// or `checked="true"` reads true; `checked="false"` (the reactive
/// bool's rendered form) reads false.
fn attr_bool(attrs: &[Attr], name: &str) -> bool {
  match attrs.iter().find(|a| a.name() == name) {
    Some(attr) => attr.as_str().map(|v| v != "false").unwrap_or(true),
    None => false,
  }
}

/// Apply radio group exclusivity after a radio tap: the tapped
/// radio shows the filled glyph and every other radio sharing its
/// `name` shows the hollow one. Reads the group from the command
/// stream and repaints the matching placed `UIButton`s in place —
/// a plain `UIButton` has no built-in toggle, so the renderer owns
/// the visual selection (the desktop egui path gets this free from
/// its per-frame group map).
fn select_radio_in_group(
  host: &mut ViewHost,
  cmds: &[UiCommand],
  tapped: &str,
) {
  // The tapped widget is only a radio when its command carries
  // `type="radio"`; resolve its group `name` (else this was a
  // checkbox / select change — nothing to do).
  let group = cmds.iter().find_map(|cmd| match cmd {
    UiCommand::Element {
      tag: ElementTag::Input,
      attrs,
      ..
    } if widget_id_str(attrs).as_deref() == Some(tapped)
      && attr_text(attrs, "type").as_deref() == Some("radio") =>
    {
      Some(attr_text(attrs, "name").unwrap_or_default())
    }
    _ => None,
  });

  let Some(group) = group else {
    return;
  };

  // Clone the placement→command map so the views can be borrowed
  // mutably while we walk it (the slice borrows `host.tree`).
  let placements: Vec<usize> = host.tree.cmd_index().to_vec();

  for (placement, cmd_idx) in placements.into_iter().enumerate() {
    let Some(UiCommand::Element {
      tag: ElementTag::Input,
      attrs,
      ..
    }) = cmds.get(cmd_idx)
    else {
      continue;
    };

    if attr_text(attrs, "type").as_deref() != Some("radio")
      || attr_text(attrs, "name").unwrap_or_default() != group
    {
      continue;
    }

    let glyph = if widget_id_str(attrs).as_deref() == Some(tapped) {
      RADIO_SELECTED
    } else {
      RADIO_UNSELECTED
    };

    if let Some(PlacedView::Button(button)) = host.views.get(placement) {
      button.setTitle_forState(
        Some(&NSString::from_str(glyph)),
        UIControlState::Normal,
      );
    }
  }
}

/// Gather a `<select>`'s `<option>` choice strings by scanning the
/// command stream from the select element to its matching
/// `EndElement`. Mirrors the desktop renderer's `gather_options`.
fn gather_options(cmds: &[UiCommand], select_idx: usize) -> Vec<String> {
  let mut options = Vec::new();
  let mut depth = 0i32;
  let mut index = select_idx;

  while index < cmds.len() {
    match &cmds[index] {
      UiCommand::Element { tag, .. } => {
        if *tag == ElementTag::Option {
          options.push(collapse_text(cmds, index + 1));
        }

        depth += 1;
      }
      UiCommand::EndElement => {
        depth -= 1;

        if depth == 0 {
          break;
        }
      }
      _ => {}
    }

    index += 1;
  }

  options
}

/// Realize the Carousel scene-specification classes before the run
/// loop brings the scene XPC in. FrontBoard delivers a watch app's
/// scene as a `CUISWKApplicationSceneSpecification`; if that class is
/// not registered in this process when the message arrives, FrontBoard
/// aborts scene creation and the app never reaches the screen.
/// WatchKit links CarouselUIServices — load both, exactly as a watch
/// app that links WatchKit would.
#[cfg(target_os = "watchos")]
fn preload_scene_frameworks() {
  /// `dlopen` mode: resolve symbols eagerly, matching what a load
  /// command would do.
  const RTLD_NOW: c_int = 0x2;

  unsafe extern "C" {
    fn dlopen(path: *const c_char, flag: c_int) -> *mut std::ffi::c_void;
  }

  const FRAMEWORKS: [&std::ffi::CStr; 2] = [
    c"/System/Library/Frameworks/WatchKit.framework/WatchKit",
    c"/System/Library/PrivateFrameworks\
      /CarouselUIServices.framework/CarouselUIServices",
  ];

  for path in FRAMEWORKS {
    // SAFETY: a NUL-terminated constant path. A null return (missing
    // framework) is tolerable — the scene-decode failure that follows
    // is observable in the system log.
    unsafe { dlopen(path.as_ptr(), RTLD_NOW) };
  }
}

/// Launch the UIKit run loop. Blocks until the app exits. The
/// delegates are constructed by UIKit from their registered class
/// names, so the classes must be registered first.
pub(crate) fn run() {
  let _mtm = MainThreadMarker::new()
    .expect("zo_run_native must be called on the main thread");

  #[cfg(target_os = "watchos")]
  preload_scene_frameworks();

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
