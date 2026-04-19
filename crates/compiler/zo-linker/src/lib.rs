//! System linker invocation for the Cranelift backend.
//!
//! `zo-codegen-clif` emits a relocatable object file; turning
//! that into a runnable executable needs a linker. This crate
//! shells out to `cc` (the platform C compiler front-end) which
//! pulls in the C runtime (`crt0` / `crt1`) and resolves any
//! FFI imports against libc / libSystem.
//!
//! Scope (Phase 4): host-matching Unix targets only
//! (`arm64-apple-darwin`, `x86_64-apple-darwin`,
//! `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`).
//! Windows MSVC (`link.exe`) and wasm (`wasm-ld`) need
//! different tooling and are flagged as `CrossNotSupported`.

mod error;
mod linker;

pub use error::LinkError;
pub use linker::link_to_executable;
