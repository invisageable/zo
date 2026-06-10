//! `cc`-driven link step: bytes-in, executable-out.

use crate::error::LinkError;

use zo_codegen_backend::Target;

use std::io;
use std::path::Path;
use std::process::Command;

/// Embedded C runtime — wrappers around variadic libc calls
/// (`snprintf`) with fixed per-type signatures so CLIF can
/// declare them without hitting "one sig per external name".
/// Compiled on every link via `cc` alongside the user's `.o`.
const RUNTIME_C: &str = include_str!("runtime.c");

/// Writes `object_bytes` to a temp `.o`, drops the embedded
/// C runtime into a sibling temp file, invokes `cc` to
/// compile-and-link both into an executable at `output_path`,
/// and cleans up both temp files. Each `NamedTempFile` owns
/// its path — Drop removes the file even if `cc` fails.
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
  let runtime_file = write_temp_runtime()?;

  invoke_cc(obj_file.path(), runtime_file.path(), output_path, target)

  // Both temp files drop here — `NamedTempFile`'s Drop removes
  // the backing file automatically, success or failure.
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
    | Target::Wasm32UnknownUnknown
    | Target::Arm64AppleIos
    | Target::Arm64AppleIosSim
    | Target::Arm64AppleWatchOsSim
    | Target::Aarch64LinuxAndroid
    | Target::Web => Err(LinkError::CrossNotSupported(target)),
  }
}

/// Writes `bytes` to a temp file with the given suffix via
/// `tempfile`. The returned [`tempfile::NamedTempFile`] owns
/// the path and auto-deletes on drop — the caller just holds
/// it until `cc` exits.
fn write_temp(
  suffix: &str,
  bytes: &[u8],
) -> Result<tempfile::NamedTempFile, LinkError> {
  use std::io::Write as _;

  let mut file = tempfile::Builder::new()
    .prefix("zo-")
    .suffix(suffix)
    .tempfile()
    .map_err(LinkError::Io)?;

  file.write_all(bytes).map_err(LinkError::Io)?;
  file.flush().map_err(LinkError::Io)?;

  Ok(file)
}

fn write_temp_object(
  bytes: &[u8],
) -> Result<tempfile::NamedTempFile, LinkError> {
  write_temp(".o", bytes)
}

fn write_temp_runtime() -> Result<tempfile::NamedTempFile, LinkError> {
  write_temp(".c", RUNTIME_C.as_bytes())
}

/// `cc {obj} {runtime.c} -o {exe}` with stderr captured.
/// `cc` accepts mixed `.o` + `.c` inputs natively — it
/// compiles the C source and links everything with `crt0` /
/// `crt1` and libc / libSystem in a single invocation.
///
/// On Apple targets an explicit `-arch` is passed so an
/// arm64-darwin host can build x86_64 Mach-O bytes (and vice
/// versa); without the flag Apple's `cc` defaults to the host
/// arch and rejects the object file.
fn invoke_cc(
  obj: &Path,
  runtime: &Path,
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

  cmd.arg(obj).arg(runtime).arg("-o").arg(output);

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
