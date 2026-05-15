//! Orchestrator. Owns the per-target spec table, drives
//! [`crate::generator`] for the concrete codegen, returns the
//! aggregated [`crate::BindResult`].
//!
//! Holds NO state between calls — every `bind()` is pure.

use crate::generator;
use crate::{BindError, BindResult, BindTarget};

use std::collections::HashMap;

/// One entry in a target's spec table.
///
/// `zo_name` is the snake_case identifier the user writes
/// (`init_window`). `c_symbol` is the actual extern symbol
/// in the dylib (`InitWindow` — the linker prepends the
/// platform underscore to make `_InitWindow` on Mach-O).
/// `signature` is the zo type signature emitted into the
/// `pub ffi` declaration.
#[derive(Clone, Copy)]
pub struct SpecEntry {
  pub zo_name: &'static str,
  pub c_symbol: &'static str,
  pub signature: &'static str, // e.g. "(width: int, height: int, title: str)"
  pub return_ty: Option<&'static str>, // None ⇒ no `-> T` clause
}

/// Hardcoded raylib MVP — exactly the M1 surface (open and
/// close a window). Grows to M2/M3/M4 by adding entries.
const RAYLIB_SPEC: &[SpecEntry] = &[
  SpecEntry {
    zo_name: "init_window",
    c_symbol: "InitWindow",
    signature: "(width: int, height: int, title: str)",
    return_ty: None,
  },
  SpecEntry {
    zo_name: "window_should_close",
    c_symbol: "WindowShouldClose",
    signature: "()",
    return_ty: Some("bool"),
  },
  SpecEntry {
    zo_name: "close_window",
    c_symbol: "CloseWindow",
    signature: "()",
    return_ty: None,
  },
];

const RAYLIB_DYLIB: &str = "raylib";

pub fn bind(
  target: BindTarget,
  items: &[&str],
) -> Result<BindResult, BindError> {
  let spec = match target {
    BindTarget::Raylib => RAYLIB_SPEC,
  };
  let dylib = match target {
    BindTarget::Raylib => RAYLIB_DYLIB,
  };

  let mut ffi_decls = Vec::with_capacity(items.len());
  let mut symbol_dylib = HashMap::with_capacity(items.len());

  for item in items {
    let entry = spec.iter().find(|e| e.zo_name == *item).ok_or_else(|| {
      BindError::UnknownItem {
        target,
        item: (*item).to_string(),
      }
    })?;

    ffi_decls.push(generator::emit_ffi_decl(entry));
    symbol_dylib.insert(generator::c_symbol(entry), dylib.to_string());
  }

  Ok(BindResult {
    ffi_decls,
    link_libs: vec![dylib.to_string()],
    symbol_dylib,
  })
}
