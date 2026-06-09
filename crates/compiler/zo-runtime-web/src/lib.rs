//! The web runtime — runs the program's UI from the codegen output
//! (`zo-codegen-web`). Three paths: a desktop webview (wry) that
//! displays the bundle and drives host-side reactivity; the
//! `_zo_run_web` C-ABI entry an AOT webview binary calls; and a static
//! file server that serves the `public/` bundle to the system browser.

pub mod ffi;
pub mod runtime;
pub mod serve;

pub use ffi::zo_run_web;
pub use runtime::Runtime;
pub use serve::{Browsering, Server};
