//! iOS (UIKit) runtime for AOT-compiled zo programs.
//!
//! Mirrors `zo-runtime-native`: the platform-agnostic ABI + reactive
//! plumbing lives in `zo_runtime_render::aot` (shared with every
//! backend); this crate is the UIKit half — `_zo_run_native` decodes
//! the template through `aot`, then bootstraps a UIKit app whose
//! delegate renders the `UiCommand` stream into native views.

// The runtime is UIKit, so it compiles only for iOS; on every other
// target (a host `cargo test --all`) this crate is an empty lib.
#[cfg(target_os = "ios")]
mod app;
#[cfg(target_os = "ios")]
mod ffi;

/// Re-exported so the aggregating `zo-runtime` cdylib can reference
/// (and thus force the linker to keep + export) the `_zo_run_native`
/// entry — nothing in `zo-runtime` calls into this crate's code, so
/// without a reference the symbol the AOT binary needs is stripped.
#[cfg(target_os = "ios")]
pub use ffi::zo_run_native;
