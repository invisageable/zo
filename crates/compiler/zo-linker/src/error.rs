//! `zo-linker` error surface.

use zo_codegen_backend::Target;

use std::io;

/// Every way [`crate::link_to_executable`] can fail. Kept as a
/// small enum so callers can surface each case with a targeted
/// message (e.g. "install Xcode CLT" on `ToolMissing`).
#[derive(Debug)]
pub enum LinkError {
  /// `cc` isn't on `PATH`. The message includes install hints
  /// for common platforms.
  ToolMissing(String),
  /// The target isn't yet supported by this crate — Windows
  /// MSVC (needs `link.exe`) and wasm (needs `wasm-ld`) both
  /// land here.
  CrossNotSupported(Target),
  /// `cc` ran but exited non-zero. `stderr` is captured for
  /// the user.
  InvocationFailed { status: Option<i32>, stderr: String },
  /// I/O error — temp file write, output rename, etc.
  Io(io::Error),
}

impl std::fmt::Display for LinkError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::ToolMissing(message) => write!(f, "linker tool missing: {message}"),
      Self::CrossNotSupported(target) => {
        write!(f, "linking {target:?} is not yet supported by zo-linker")
      }
      Self::InvocationFailed { status, stderr } => {
        write!(f, "cc exited with status {status:?}:\n{stderr}")
      }
      Self::Io(err) => write!(f, "linker io error: {err}"),
    }
  }
}

impl std::error::Error for LinkError {}
