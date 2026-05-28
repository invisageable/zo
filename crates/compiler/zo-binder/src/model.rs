//! Data model for parsed FFI items and the binder's errors.

use std::fmt;

/// A C-ABI function extracted from a Rust shim's source.
///
/// @note — `name` is also the exported C symbol, since the function is
/// `#[no_mangle]`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FfiItem {
  /// The Rust function name (and the exported C symbol).
  pub name: String,
  /// Parameters in declaration order.
  pub params: Vec<Param>,
  /// The return type (`RustTy::Unit` when there is none).
  pub ret: RustTy,
  /// Leading `///` doc lines, trimmed, in source order.
  pub doc: Vec<String>,
}

/// A single function parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Param {
  /// The parameter name.
  pub name: String,
  /// The parameter's Rust type.
  pub ty: RustTy,
}

/// A normalized view of a Rust type in a shim signature.
///
/// @note — captures only the shapes a C-ABI shim can express; anything else
/// lands in `Other` for the type mapper to reject with a precise spelling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RustTy {
  /// A named type by its last path segment (`i64`, `c_char`).
  Path(String),
  /// A raw pointer `*const T` / `*mut T`.
  Ptr { mutable: bool, inner: Box<RustTy> },
  /// The unit type `()` / an absent return.
  Unit,
  /// Any other type, kept verbatim for diagnostics.
  Other(String),
}

/// An error raised while binding a Rust shim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BindError {
  /// The Rust source failed to parse.
  Syntax(String),
}

impl fmt::Display for BindError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      BindError::Syntax(msg) => write!(f, "syntax error: {msg}"),
    }
  }
}

impl std::error::Error for BindError {}
