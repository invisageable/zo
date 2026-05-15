//! Binder — turns a foreign-source spec into zo `pub ffi`
//! declarations + linker hints.
//!
//! Layer 1 of the rendering stack
//! (see `PLAN_RUST_INTEROP.md`). For raylib MVP, the binder
//! holds a hardcoded spec table and emits `pub ffi` strings
//! the executor can ingest.
//!
//! Public surface is one function: [`bind`].

mod binder;
mod generator;

use std::collections::HashMap;

/// Foreign-source target the binder knows how to emit
/// declarations for. New targets become new variants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BindTarget {
  Raylib,
}

/// Output of a successful [`bind`] call.
///
/// `ffi_decls` are zo source strings — each is a complete
/// `pub ffi name(...) -> ...;` line ready to be re-fed into
/// the executor's load handler.
///
/// `link_libs` lists the dylibs whose symbols this binding
/// pulls in (without the `lib` prefix or `.dylib` suffix —
/// e.g. `"raylib"` not `"libraylib.dylib"`).
///
/// `symbol_dylib` maps each emitted C symbol (e.g.
/// `"_InitWindow"`) to the dylib that owns it. The Mach-O
/// linker uses this to route symbols to their
/// `LC_LOAD_DYLIB` entries — falls back to libSystem when
/// a symbol isn't in the map.
pub struct BindResult {
  pub ffi_decls: Vec<String>,
  pub link_libs: Vec<String>,
  pub symbol_dylib: HashMap<String, String>,
}

/// Errors returned by [`bind`]. Kept small for v1 — the spec
/// is hardcoded so most failure modes are programmer errors
/// (typo'd item name).
#[derive(Debug)]
pub enum BindError {
  /// Caller asked for an item that isn't in the target's
  /// spec table.
  UnknownItem { target: BindTarget, item: String },
}

/// Resolve `items` against `target`'s spec, return the zo
/// `pub ffi` declarations + linker routing.
pub fn bind(
  target: BindTarget,
  items: &[&str],
) -> Result<BindResult, BindError> {
  binder::bind(target, items)
}
