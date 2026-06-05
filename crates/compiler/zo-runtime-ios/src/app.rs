//! UIKit application bootstrap + `UiCommand` → native-view renderer.

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{ClassType, MainThreadMarker, MainThreadOnly, define_class};

use objc2_foundation::{NSDictionary, NSObjectProtocol, NSString};
use objc2_ui_kit::{
  UIApplication, UIApplicationDelegate, UIApplicationLaunchOptionsKey,
  UIButton, UIButtonType, UIColor, UIControlState, UILabel,
  UILayoutConstraintAxis, UIScreen, UIStackView, UIStackViewAlignment,
  UIViewController, UIWindow,
};

use zo_ui_protocol::{ElementTag, UiCommand};

use std::cell::RefCell;
use std::ffi::{c_char, c_int};
use std::ptr::NonNull;

thread_local! {
  /// Retains the key window for the process lifetime. UIKit holds
  /// only a weak reference, so without an owner here the window
  /// deallocates the moment `didFinishLaunchingWithOptions` returns
  /// and the screen goes black. A `thread_local` (not a `static`)
  /// because `Retained` is not `Sync`, and everything here runs on
  /// the UIKit main thread.
  static WINDOW: RefCell<Option<Retained<UIWindow>>> =
    const { RefCell::new(None) };
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
    #[unsafe(method(application:didFinishLaunchingWithOptions:))]
    fn did_finish_launching(
      &self,
      _application: &UIApplication,
      _launch_options: Option<
        &NSDictionary<UIApplicationLaunchOptionsKey, AnyObject>,
      >,
    ) -> bool {
      let mtm = MainThreadMarker::from(self);
      let bounds = UIScreen::mainScreen(mtm).bounds();
      let window = UIWindow::initWithFrame(UIWindow::alloc(mtm), bounds);
      let controller = UIViewController::new(mtm);
      let stack = build_view(&crate::ffi::commands(), mtm);

      if let Some(root) = controller.view() {
        root.setBackgroundColor(Some(&UIColor::whiteColor()));
        stack.setFrame(root.bounds());
        root.addSubview(&stack);
      }

      window.setRootViewController(Some(&controller));
      window.makeKeyAndVisible();
      WINDOW.with(|w| *w.borrow_mut() = Some(window));

      true
    }
  }
);

/// Build the native view tree from the command stream. M1: a vertical
/// `UIStackView`; `Element{Button}` → `UIButton` whose title is the
/// `Text` that follows before `EndElement`; a free-standing `Text` →
/// `UILabel`. Events / styles are ignored for now.
fn build_view(
  cmds: &[UiCommand],
  mtm: MainThreadMarker,
) -> Retained<UIStackView> {
  let stack = UIStackView::new(mtm);

  stack.setAxis(UILayoutConstraintAxis::Vertical);
  stack.setAlignment(UIStackViewAlignment::Center);
  stack.setSpacing(24.0);

  let mut pending_button: Option<Retained<UIButton>> = None;

  for cmd in cmds {
    match cmd {
      UiCommand::Element { tag, .. } if *tag == ElementTag::Button => {
        let button = UIButton::buttonWithType(UIButtonType::System, mtm);

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
          }
        }
      }
      UiCommand::EndElement => pending_button = None,
      _ => {}
    }
  }

  stack
}

/// Launch the UIKit run loop. Blocks until the app exits. The delegate
/// is constructed by `UIApplicationMain` from its registered class
/// name, so the class must be registered first.
pub(crate) fn run() {
  let _mtm = MainThreadMarker::new()
    .expect("zo_run_native must be called on the main thread");

  // Force class registration so the ObjC runtime can resolve
  // "ZoAppDelegate" by name when `UIApplicationMain` instantiates it.
  let _ = AppDelegate::class();

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
