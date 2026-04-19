//! Cranelift backend for zo.
//!
//! Covers every `Target` that is NOT `arm64-apple-darwin` or
//! `aarch64-unknown-linux-gnu` (those stay on the hand-written
//! `zo-codegen-arm` path):
//!
//! - `x86_64-apple-darwin`
//! - `x86_64-unknown-linux-gnu`
//! - `x86_64-pc-windows-msvc`
//! - `aarch64-pc-windows-msvc`
//!
//! Entry point [`CliftGen`] selects an ISA from the `Target`,
//! builds an `ObjectModule`, translates SIR into CLIF, and
//! hands the resulting object bytes to `zo-linker` for the
//! final `cc` invocation.

mod codegen;
mod translate;
mod types;

pub use codegen::CliftGen;
