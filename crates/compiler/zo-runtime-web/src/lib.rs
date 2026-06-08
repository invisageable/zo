//! The web runtime — runs the program's UI from the codegen output
//! (`zo-codegen-web`). Two paths: a desktop webview (wry) that displays
//! the bundle and drives host-side reactivity, and a static file server
//! that serves the `public/` bundle to the system browser.

pub mod runtime;
pub mod serve;

pub use runtime::Runtime;
pub use serve::{Browsering, Server};
