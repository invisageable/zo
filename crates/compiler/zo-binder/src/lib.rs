//! zo-binder — generate zo FFI bindings from Rust shims.
//!
//! Reads a C-ABI shim crate's source and emits the
//! `provider/<lib>/<lib>.zo` shape the AAPCS-from-signature
//! pipeline consumes. Ahead-of-time only — never on the
//! `zo run` path.

pub mod bind;
pub mod cheader;
pub mod ctymap;
pub mod emit;
pub mod model;
pub mod parse;
pub mod tymap;

#[cfg(test)]
mod tests;
