//! Map shim Rust types onto zo types at the FFI boundary.
//!
//! @note — see the type table in `PLAN_ZO_BINDER.md`. Prefer
//! the public alias (`int`, `uint`, `float`) so a `pub ffi`
//! never exposes a raw width name.

use crate::model::{BindError, RustTy, ZoTy};

/// Map a Rust parameter or return type to its zo equivalent.
///
/// @param func — the enclosing function, for diagnostics.
pub fn map_ty(ty: &RustTy, func: &str) -> Result<ZoTy, BindError> {
  match ty {
    RustTy::Path(name) => {
      map_named(name).ok_or_else(|| unsupported(func, name))
    }
    RustTy::Ptr { inner, .. } => Ok(map_ptr(inner)),
    RustTy::Unit => Ok(ZoTy::Unit),
    RustTy::Other(spelling) => Err(unsupported(func, spelling)),
  }
}

/// Map a primitive Rust type name to its zo spelling.
fn map_named(name: &str) -> Option<ZoTy> {
  Some(ZoTy::Named(match name {
    "i8" => "s8",
    "i16" => "s16",
    "i32" => "int",
    "i64" | "isize" => "s64",
    "u8" => "u8",
    "u16" => "u16",
    "u32" => "uint",
    "u64" | "usize" => "u64",
    "f32" => "float",
    "f64" => "f64",
    "bool" => "bool",
    _ => return None,
  }))
}

/// Map a pointer type: `*const c_char` is a C string (`CStr`),
/// every other pointer is an opaque pointer-width handle.
fn map_ptr(inner: &RustTy) -> ZoTy {
  match inner {
    RustTy::Path(name) if name == "c_char" => ZoTy::Named("CStr"),
    _ => ZoTy::Named("s64"),
  }
}

/// Build an `UnsupportedType` error for `func`'s `rust_ty`.
fn unsupported(func: &str, rust_ty: &str) -> BindError {
  BindError::UnsupportedType {
    func: func.to_string(),
    rust_ty: rust_ty.to_string(),
  }
}
