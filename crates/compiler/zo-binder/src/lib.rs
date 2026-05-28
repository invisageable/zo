//! zo-binder — generate zo FFI bindings from Rust shims.
//!
//! Reads a C-ABI shim crate's source and emits the
//! `provider/<lib>/<lib>.zo` shape the AAPCS-from-signature
//! pipeline consumes. Ahead-of-time only — never on the
//! `zo run` path.

pub mod model;
pub mod parse;

#[cfg(test)]
mod tests;
