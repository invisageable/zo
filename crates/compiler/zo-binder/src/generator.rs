//! Pure codegen. No state, no I/O. Given a [`SpecEntry`],
//! produce a zo `pub ffi` declaration string and the
//! corresponding C symbol name (with the platform-leading
//! underscore Mach-O expects).

use crate::binder::SpecEntry;

/// Format the spec entry as a zo source line:
///
///   pub ffi <zo_name><signature>[ -> <return_ty>];
///
/// The output is ingested by the executor's load handler as
/// if the user had typed it directly.
pub fn emit_ffi_decl(entry: &SpecEntry) -> String {
  match entry.return_ty {
    Some(ret) => format!(
      "pub ffi {name}{sig} -> {ret};",
      name = entry.zo_name,
      sig = entry.signature,
      ret = ret,
    ),
    None => format!(
      "pub ffi {name}{sig};",
      name = entry.zo_name,
      sig = entry.signature,
    ),
  }
}

/// Mach-O extern symbol for this entry. The leading
/// underscore is the platform convention for C symbols on
/// macOS (clang emits `_InitWindow` in the symbol table for
/// a C function declared as `void InitWindow(...)`).
pub fn c_symbol(entry: &SpecEntry) -> String {
  format!("_{}", entry.c_symbol)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn entry(
    zo: &'static str,
    c: &'static str,
    sig: &'static str,
    ret: Option<&'static str>,
  ) -> SpecEntry {
    SpecEntry {
      zo_name: zo,
      c_symbol: c,
      signature: sig,
      return_ty: ret,
    }
  }

  #[test]
  fn ffi_decl_no_return() {
    let e = entry(
      "init_window",
      "InitWindow",
      "(width: int, height: int, title: str)",
      None,
    );
    assert_eq!(
      emit_ffi_decl(&e),
      "pub ffi init_window(width: int, height: int, title: str);"
    );
  }

  #[test]
  fn ffi_decl_with_return() {
    let e = entry(
      "window_should_close",
      "WindowShouldClose",
      "()",
      Some("bool"),
    );
    assert_eq!(emit_ffi_decl(&e), "pub ffi window_should_close() -> bool;");
  }

  #[test]
  fn c_symbol_has_leading_underscore() {
    let e = entry("init_window", "InitWindow", "()", None);
    assert_eq!(c_symbol(&e), "_InitWindow");
  }
}
