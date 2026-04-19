//! `cc`-driven link step: bytes-in, executable-out.

use crate::error::LinkError;

use zo_codegen_backend::Target;

use std::io;
use std::path::Path;
use std::process::Command;

/// Writes `object_bytes` to a temp `.o`, invokes `cc` to link
/// it into an executable at `output_path`, and cleans up the
/// temp file. The temp file is owned by a `tempfile::NamedTempFile`
/// so it's removed even if the linker fails.
///
/// Contract: `output_path`'s parent directory must exist. The
/// function overwrites `output_path` if it already exists.
pub fn link_to_executable(
  object_bytes: &[u8],
  output_path: &Path,
  target: Target,
) -> Result<(), LinkError> {
  ensure_target_supported(target)?;

  let obj_file = write_temp_object(object_bytes)?;
  let obj_path = obj_file.path();

  invoke_cc(obj_path, output_path, target)

  // `obj_file` drops here — `NamedTempFile`'s Drop removes the
  // temp file automatically, success or failure.
}

/// Non-Windows, non-wasm is supported by `cc`. Everything else
/// needs platform-specific tooling we haven't wired yet.
fn ensure_target_supported(target: Target) -> Result<(), LinkError> {
  match target {
    Target::Arm64AppleDarwin
    | Target::X8664AppleDarwin
    | Target::Arm64UnknownLinuxGnu
    | Target::X8664UnknownLinuxGnu => Ok(()),
    Target::Arm64PcWindowsMsvc
    | Target::X8664PcWindowsMsvc
    | Target::Wasm32UnknownUnknown => Err(LinkError::CrossNotSupported(target)),
  }
}

/// Writes `bytes` to a temp `.o` file via `tempfile`. The
/// returned [`tempfile::NamedTempFile`] auto-deletes on drop —
/// the caller just needs to hold it until `cc` exits.
fn write_temp_object(
  bytes: &[u8],
) -> Result<tempfile::NamedTempFile, LinkError> {
  use std::io::Write as _;

  let mut file = tempfile::Builder::new()
    .prefix("zo-")
    .suffix(".o")
    .tempfile()
    .map_err(LinkError::Io)?;

  file.write_all(bytes).map_err(LinkError::Io)?;
  file.flush().map_err(LinkError::Io)?;

  Ok(file)
}

/// `cc {obj} -o {exe}` with stderr captured. `cc` pulls in
/// `crt0` / `crt1` (the C startup that calls `main`) and
/// libc / libSystem, so FFI imports like `printf` resolve
/// at link time. No `-l` flags needed for the Phase 4 scope
/// — additional libraries would be added via a future
/// `LinkerOptions` struct if zo programs start linking against
/// non-libc code.
///
/// On Apple targets an explicit `-arch` is passed so an
/// arm64-darwin host can link x86_64 Mach-O bytes (and vice
/// versa); without the flag Apple's `cc` defaults to the host
/// arch and rejects the object file.
fn invoke_cc(
  obj: &Path,
  output: &Path,
  target: Target,
) -> Result<(), LinkError> {
  let mut cmd = Command::new("cc");

  match target {
    Target::Arm64AppleDarwin => {
      cmd.arg("-arch").arg("arm64");
    }
    Target::X8664AppleDarwin => {
      cmd.arg("-arch").arg("x86_64");
    }
    // Linux targets: `-arch` is Apple-specific. Cross-linking
    // to Linux from macOS needs a dedicated `x86_64-linux-gnu-gcc`
    // and stays out of scope for this phase.
    _ => {}
  }

  cmd.arg(obj).arg("-o").arg(output);

  let out = cmd.output().map_err(|err| match err.kind() {
    io::ErrorKind::NotFound => LinkError::ToolMissing(
      "`cc` not found on PATH. Install Xcode Command Line Tools \
       (macOS: `xcode-select --install`) or a C toolchain \
       (Linux: `apt install build-essential` / `dnf install gcc`)."
        .to_string(),
    ),
    _ => LinkError::Io(err),
  })?;

  if !out.status.success() {
    return Err(LinkError::InvocationFailed {
      status: out.status.code(),
      stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    });
  }

  Ok(())
}
