//! System linker invocation for the Cranelift backend.
//!
//! `zo-codegen-clif` emits a relocatable object file; turning
//! that into a runnable executable needs a linker. This crate
//! shells out to `cc` (the platform C compiler front-end) which
//! pulls in the C runtime (`crt0` / `crt1`) and resolves any
//! FFI imports against libc / libSystem.
//!
//! Supported targets: host-matching Unix only
//! (`arm64-apple-darwin`, `x86_64-apple-darwin`,
//! `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`).
//! Windows MSVC (`link.exe`) and wasm (`wasm-ld`) need
//! different tooling and are flagged as `CrossNotSupported`.

mod error;
mod linker;
mod linker_macho;

use std::io;
use std::path::Path;

use zo_codegen_backend::{LinkObject, Target};

pub use error::LinkError;
pub use linker::link_to_executable;
pub use linker_macho::link_macho;

/// Top-level link entry point — turns codegen's
/// [`LinkObject`] into an executable file.
///
/// `LinkObject::Macho` runs the in-process mach-o
/// assembler ([`link_macho`]) and writes the executable
/// directly. `LinkObject::Object` shells out to `cc`
/// ([`link_to_executable`]) which provides `crt0` /
/// `crt1` and resolves FFI imports against
/// libc / libSystem.
///
/// Errors from `cc` are surfaced via the returned
/// `LinkError`; the user's `output_path` is left
/// untouched on failure.
pub fn link(
  link_obj: LinkObject,
  output_path: &Path,
  target: Target,
) -> Result<(), LinkError> {
  match link_obj {
    LinkObject::Macho(m) => {
      let executable = link_macho(*m);

      write_executable(&executable, output_path).map_err(LinkError::Io)
    }
    LinkObject::Object(code) => link_to_executable(&code, output_path, target),
  }
}

/// Write executable bytes to disk and mark the file as
/// executable on Unix. Public because tests in the
/// arm/clift backends round-trip raw bytes (e.g.
/// hello-world, code-signing) and they share the same
/// disk-I/O contract as the production link path.
pub fn write_executable(bytes: &[u8], output_path: &Path) -> io::Result<()> {
  std::fs::write(output_path, bytes)?;

  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;

    let mut perms = std::fs::metadata(output_path)?.permissions();

    perms.set_mode(0o755);
    std::fs::set_permissions(output_path, perms)?;
  }

  Ok(())
}
