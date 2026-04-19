//! Cranelift backend for zo.
//!
//! Covers every `Target` that is NOT `arm64-apple-darwin` or
//! `aarch64-unknown-linux-gnu` (those stay on the hand-written
//! `zo-codegen-arm` path). Scope at phase 1:
//!
//! - `x86_64-apple-darwin`
//! - `x86_64-unknown-linux-gnu`
//! - `x86_64-pc-windows-msvc`
//! - `aarch64-pc-windows-msvc`
//!
//! Phase 1 landing bar: emit a valid object file for an empty
//! `main` function. No SIR translation yet — just the plumbing
//! (ISA selection from `Target`, `ObjectModule` setup, trivial
//! CLIF function, `ObjectProduct::emit()` round-trip).

mod codegen;
mod translate;
mod types;

pub use codegen::CliftGen;
