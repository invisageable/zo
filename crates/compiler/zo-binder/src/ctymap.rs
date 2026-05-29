//! Map C type spellings (from rlparser JSON) to zo types.
//!
//! @note — the C path's counterpart to [`crate::tymap`]; it
//! works on type strings (`"const char *"`, `"Vector2"`)
//! rather than parsed Rust types.

use crate::model::{BindError, ZoTy};

use std::collections::HashSet;

/// Map a C type spelling to a zo type.
///
/// @param known — names of structs/enums/aliases in the API,
/// used to resolve a bare named type to a zo struct.
/// @param func — the enclosing function, for diagnostics.
pub fn map_c_ty(
  c: &str,
  known: &HashSet<String>,
  func: &str,
) -> Result<ZoTy, BindError> {
  let text = c.trim();

  // A pointer: `char *` is a C string, anything else is an
  // opaque pointer-width handle.
  if let Some(pointee) = text.strip_suffix('*') {
    let pointee = pointee.trim().trim_start_matches("const ").trim();

    return Ok(match pointee {
      "char" => ZoTy::Named("CStr"),
      _ => ZoTy::Named("s64"),
    });
  }

  let text = text.trim_start_matches("const ").trim();

  let named = match text {
    "void" => return Ok(ZoTy::Unit),
    "bool" => "bool",
    "char" => "s8",
    "unsigned char" => "u8",
    "short" => "s16",
    "unsigned short" => "u16",
    "int" => "int",
    "unsigned int" | "unsigned" => "uint",
    "long" | "long long" => "s64",
    "unsigned long" | "unsigned long long" => "u64",
    "float" => "float",
    "double" => "f64",
    _ if known.contains(text) => {
      return Ok(ZoTy::Struct(text.to_string()));
    }
    _ => return Err(unsupported(func, c)),
  };

  Ok(ZoTy::Named(named))
}

/// Build an `UnsupportedType` error for `func`'s C type.
fn unsupported(func: &str, c: &str) -> BindError {
  BindError::UnsupportedType {
    func: func.to_string(),
    rust_ty: c.to_string(),
  }
}
