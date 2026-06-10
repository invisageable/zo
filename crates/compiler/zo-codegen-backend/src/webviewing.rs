//! Whether a build targets the webview runtime.

/// Whether `#render` lowers to the webview runtime entry (`_zo_run_web`,
/// wry) or the native one (`_zo_run_native`, eframe).
///
/// The webview and native desktop targets compile to the same host
/// triple and the same SIR — only the runtime entry symbol the codegen
/// emits differs. This flag carries that one bit from the driver
/// (`--target webview`) down to the backend.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum Webviewing {
  /// Native desktop (eframe) — the default.
  #[default]
  No,
  /// Webview desktop (wry).
  Yes,
}
