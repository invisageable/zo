//! ...

use crate::output::{File, FileKind, Files, Output};

use zo_session::backend::Backend;

use zo_core::reporter::report::io::Io;
use zo_core::writer::Writer;
use zo_core::Result;

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn build(backend: &Backend, bytecode: &[u8]) -> Result<Output> {
  const PATHNAME: &str = "program/clif";
  const FILENAME_OBJ: &str = "program/clif/main.o";
  const FILENAME_EXE: &str = "program/clif/main";

  let mut files = Files::new();

  Writer::write_from_bytes(PATHNAME, FILENAME_OBJ, bytecode).map(|_| {
    files.add_file(File {
      kind: FileKind::Obj,
      pathname: FILENAME_OBJ.into(),
    })
  })?;

  let ld = std::process::Command::new("ld")
    .arg("-v")
    .output()
    .map_err(Io::error)?;

  let stderr = String::from_utf8(ld.stderr).unwrap();

  let link_args: &[&str] = if stderr.contains("ld-classic") {
    &["-Xlinker", "-ld_classic"]
  } else {
    &[]
  };

  std::process::Command::new("gcc")
    .args(&["-v"])
    .args(&["-c"])
    .args(&["-ldl"])
    .args(&["-o"])
    .args(&[FILENAME_EXE])
    .args(link_args)
    .args(&[FILENAME_OBJ])
    .status()
    .map_err(Io::error)
    .map(|exit_status| {
      if exit_status.success() && std::path::Path::new(FILENAME_EXE).exists() {
        files.add_file(File {
          kind: FileKind::Exe,
          pathname: FILENAME_EXE.into(),
        })
      }
    })?;

  Ok(Output {
    backend: backend.to_owned(),
    files,
  })
}
