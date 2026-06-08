//! The wry webview runtime — loads and serves the program's UI in a
//! desktop webview. Rendering itself is codegen (`zo-codegen-web`);
//! this crate is the *runtime* that displays the result and, for now,
//! drives host-side reactivity.

pub mod runtime;

pub use runtime::Runtime;
