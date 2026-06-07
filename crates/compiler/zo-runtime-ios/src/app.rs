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

use objc2_core_foundation::{CGFloat, CGPoint, CGRect, CGSize};
use objc2_foundation::{
  NSDictionary, NSObjectProtocol, NSOperatingSystemVersion, NSProcessInfo,
  NSString,
};
use objc2_ui_kit::{
  UIApplication, UIApplicationDelegate, UIApplicationLaunchOptionsKey,
  UIButton, UIButtonConfiguration, UIButtonType, UIColor, UIControlEvents,
  UIControlState, UICornerConfiguration, UICornerRadius, UIFont, UIGlassEffect,
  UIGlassEffectStyle, UILabel, UIScene, UISceneConnectionOptions,
  UISceneDelegate, UISceneSession, UITextBorderStyle, UITextField, UIView,
  UIViewController, UIVisualEffectView, UIWindow, UIWindowScene,
  UIWindowSceneDelegate,
};

use zo_runtime_render::layout::{LayoutTree, Rect, collapse_text};
use zo_runtime_render::render::{EventPayload, EventRegistry, build_event_map};
use zo_ui_protocol::style::{
  ComputedStyle, GlassStyle, Material, Rgba, StylePatch,
};
use zo_ui_protocol::{Attr, ElementTag, EventKind, UiCommand};

use std::cell::RefCell;
use std::ffi::{c_char, c_int};
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

/// Per-process reactive wiring, built once at startup from the
/// `ZoRuntimeContext` and consumed by the scene delegate.
struct Runtime {
  /// Dispatches a handler name into its compiled zo closure, then
  /// refreshes reactive bindings into `shared`.
  registry: EventRegistry,
  /// The command buffer handlers mutate; reread after each tap to
  /// reconcile the view tree.
  shared: Arc<Mutex<Vec<UiCommand>>>,
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
  /// Reserved geometry for leaves the iOS path does not paint yet
  /// (image).
  Other(Retained<UIView>),
  /// A glass-backed leaf: `effect` is the `UIVisualEffectView` framed
  /// and attached to the container; `inner` is its real content,
  /// sized to the effect view's local bounds inside `contentView`.
  Glass {
    effect: Retained<UIVisualEffectView>,
    inner: Box<PlacedView>,
  },
}

impl PlacedView {
  fn set_frame(&self, frame: CGRect) {
    match self {
      Self::Button(button) => button.setFrame(frame),
      Self::Label(label) => label.setFrame(frame),
      Self::Field(field) => field.setFrame(frame),
      Self::Other(view) => view.setFrame(frame),
      // Frame the glass wrapper; the inner view fills it at local
      // origin, so the solver's geometry stays the source of truth.
      Self::Glass { effect, inner } => {
        effect.setFrame(frame);
        inner.set_frame(CGRect::new(CGPoint::new(0.0, 0.0), frame.size));
      }
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
      // The glass wrapper holds no text of its own; defer to its
      // inner content (a label whose text the reconciler refreshes).
      Self::Glass { inner, .. } => inner.set_text(text),
    }
  }
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
      host: None,
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
      let controller = UIViewController::new(mtm);

      let cmds = RUNTIME.with(|r| {
        r.borrow()
          .as_ref()
          .map(|rt| rt.shared.lock().unwrap().clone())
          .unwrap_or_default()
      });

      // One container view fills the screen and owns every widget by
      // frame — no `UIStackView`, no Auto-Layout. It is sized to the
      // scene's screen bounds so the solve's coordinate space matches
      // (centring comes from the root's justify/align).
      let bounds = window_scene.screen().bounds();
      let container = UIView::initWithFrame(UIView::alloc(mtm), bounds);

      container.setBackgroundColor(Some(&UIColor::whiteColor()));
      controller.setView(Some(&container));

      let (tree, views) = render_into(&cmds, &container, self, mtm);

      RUNTIME.with(|r| {
        if let Some(rt) = r.borrow_mut().as_mut() {
          rt.host = Some(ViewHost { container, tree, views });
        }
      });

      window.setRootViewController(Some(&controller));
      window.makeKeyAndVisible();
      WINDOW.with(|w| *w.borrow_mut() = Some(window));
    }
  }

  // A window scene needs a `UIWindowSceneDelegate`; the `window`
  // accessors stay optional (the window is owned via `WINDOW`).
  unsafe impl UIWindowSceneDelegate for SceneDelegate {}
);

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

        build_event_map(&cmds).get(&(widget_id, kind)).cloned()
      };

      if let Some(handler) = handler {
        runtime.registry.dispatch(&handler, &payload);
      }

      // The handler mutated `shared` in place; reconcile the view
      // tree against it.
      let cmds = runtime.shared.lock().unwrap().clone();
      let Some(host) = runtime.host.as_mut() else {
        return;
      };

      match host.tree.reconcile(&cmds) {
        // Fast path: structure unchanged. Re-solve (Taffy only
        // re-measures the dirtied leaves), re-frame every view,
        // and rewrite text on just the changed ones.
        Some(changed) => {
          let bounds = host.container.bounds();
          let rects = host
            .tree
            .solve((bounds.size.width as f32, bounds.size.height as f32));

          for (view, (_, rect)) in host.views.iter().zip(&rects) {
            view.set_frame(frame_of(*rect));
          }

          for (index, text) in changed {
            host.views[index].set_text(&text);
          }
        }
        // Structure changed (items added/removed): rebuild the
        // container's widgets once from the new stream.
        None => {
          clear_subviews(&host.container);

          let mtm = MainThreadMarker::from(self);
          let (tree, views) = render_into(&cmds, &host.container, self, mtm);

          host.tree = tree;
          host.views = views;
        }
      }
    });
  }
}

/// Builds native views for solved leaves and frames them on a
/// container. Stateless beyond its borrows — `place` returns the
/// created `PlacedView` so the host keeps it for reconciliation.
struct ViewBuilder<'a> {
  cmds: &'a [UiCommand],
  container: &'a UIView,
  target: &'a SceneDelegate,
  mtm: MainThreadMarker,
}

impl<'a> ViewBuilder<'a> {
  fn new(
    cmds: &'a [UiCommand],
    container: &'a UIView,
    target: &'a SceneDelegate,
    mtm: MainThreadMarker,
  ) -> Self {
    Self {
      cmds,
      container,
      target,
      mtm,
    }
  }

  /// Build, frame, and attach the native view for the placed leaf at
  /// `commands[idx]`. `Element{Button}` → `UIButton` titled by its
  /// collapsed text and tagged with its lowering widget id (wired to
  /// `buttonTapped:`); a text tag or free-standing `Text` → `UILabel`.
  /// Other leaves (image, input) get a bare reserved view. `style`
  /// drives the font; `author` says which colours the stylesheet
  /// declared, so undeclared widgets keep their native look.
  fn place(
    &self,
    idx: usize,
    frame: CGRect,
    style: &ComputedStyle,
    author: &StylePatch,
  ) -> PlacedView {
    match &self.cmds[idx] {
      UiCommand::Element { tag, attrs, .. } if *tag == ElementTag::Button => {
        let button = UIButton::buttonWithType(UIButtonType::System, self.mtm);
        let title = NSString::from_str(&collapse_text(self.cmds, idx + 1));

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
        self.container.addSubview(&button);

        PlacedView::Button(button)
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

        self.container.addSubview(&field);

        PlacedView::Field(field)
      }

      UiCommand::Element { tag, .. } if tag.is_text_tag() => {
        self.label(&collapse_text(self.cmds, idx + 1), frame, style, author)
      }

      UiCommand::Text(content) => self.label(content, frame, style, author),

      _ => {
        let view = UIView::initWithFrame(UIView::alloc(self.mtm), frame);

        self.container.addSubview(&view);

        PlacedView::Other(view)
      }
    }
  }

  /// Build, frame, and attach a `UILabel` rendered at the style's
  /// font + colour, so its text fits the box the solver measured and
  /// matches the cascade. A declared `background` fills the label.
  fn label(
    &self,
    text: &str,
    frame: CGRect,
    style: &ComputedStyle,
    author: &StylePatch,
  ) -> PlacedView {
    let label = UILabel::new(self.mtm);

    label.setText(Some(&NSString::from_str(text)));
    // SAFETY: a system font of a positive size, and a non-nil colour.
    unsafe {
      label.setFont(Some(&font_of(style)));
      label.setTextColor(Some(&ui_color(style.color)));
    }

    match glass_of(style) {
      // Glass label: the declared `background` tints the glass, not a
      // solid fill. The label rides inside the effect view's content.
      Some(glass) => {
        let tint = author.background.is_some().then_some(style.background);
        let effect = wrap_glass(&label, glass, tint, self.mtm);

        effect.setFrame(frame);
        label.setFrame(CGRect::new(CGPoint::new(0.0, 0.0), frame.size));
        self.container.addSubview(&effect);

        PlacedView::Glass {
          effect,
          inner: Box::new(PlacedView::Label(label)),
        }
      }
      // Solid: a declared `background` fills the label directly.
      None => {
        if author.background.is_some() {
          label.setBackgroundColor(Some(&ui_color(style.background)));
        }

        label.setFrame(frame);
        self.container.addSubview(&label);

        PlacedView::Label(label)
      }
    }
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

/// Default corner radius (pt) for a glass panel until a `border-radius`
/// property exists. Without an explicit corner config a glass effect
/// view defaults to a capsule — wrong for a rectangular panel.
const GLASS_CORNER_RADIUS: CGFloat = 16.0;

/// The glass style this element asks for, but only when the OS can
/// render it. `UIGlassEffect` is iOS 26+ and the deployment target is
/// 15.0, so every glass path funnels through this guard.
fn glass_of(style: &ComputedStyle) -> Option<GlassStyle> {
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

/// Wrap an already-built content view in a glass `UIVisualEffectView`:
/// the glass material, a rounded (non-capsule) shape, and an optional
/// `background`-derived tint. The content moves into the effect view's
/// `contentView`; the caller frames the returned wrapper.
fn wrap_glass(
  content: &UIView,
  glass: GlassStyle,
  tint: Option<Rgba>,
  mtm: MainThreadMarker,
) -> Retained<UIVisualEffectView> {
  let style = match glass {
    GlassStyle::Regular => UIGlassEffectStyle::Regular,
    GlassStyle::Clear => UIGlassEffectStyle::Clear,
  };
  let effect = UIGlassEffect::effectWithStyle(style, mtm);

  if let Some(tint) = tint {
    effect.setTintColor(Some(&ui_color(tint)));
  }

  let view = UIVisualEffectView::initWithEffect(
    UIVisualEffectView::alloc(mtm),
    Some(&effect),
  );
  let radius = UICornerRadius::fixedRadius(GLASS_CORNER_RADIUS);
  let corners = UICornerConfiguration::configurationWithUniformRadius(&radius);

  view.setCornerConfiguration(&corners);
  view.contentView().addSubview(content);

  view
}

/// Solve `cmds` against the container's bounds and place a native
/// view per leaf, returning the persistent tree + view list the host
/// reconciles against.
fn render_into(
  cmds: &[UiCommand],
  container: &UIView,
  target: &SceneDelegate,
  mtm: MainThreadMarker,
) -> (LayoutTree, Vec<PlacedView>) {
  let bounds = container.bounds();
  let mut tree = LayoutTree::build(cmds);
  let rects = tree.solve((bounds.size.width as f32, bounds.size.height as f32));

  // Styles + author patches parallel the solved leaves; clone so the
  // tree is free to move into the host alongside the views.
  let styles = tree.styles().to_vec();
  let authors = tree.authors().to_vec();
  let builder = ViewBuilder::new(cmds, container, target, mtm);
  let views = rects
    .into_iter()
    .enumerate()
    .map(|(i, (idx, rect))| {
      builder.place(idx, frame_of(rect), &styles[i], &authors[i])
    })
    .collect();

  (tree, views)
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

/// The string value of attribute `name` (a `Prop` or `Dynamic`),
/// used to seed a text field's placeholder / initial value.
fn attr_text(attrs: &[Attr], name: &str) -> Option<String> {
  attrs
    .iter()
    .find(|a| a.name() == name)
    .and_then(|a| a.as_str())
    .map(str::to_string)
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
