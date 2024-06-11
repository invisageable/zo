//! ...

use crate::output::{File, FileKind, Files, Output};

use zo_session::backend::Backend;

use zo_core::writer::Writer;
use zo_core::Result;

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn build(backend: &Backend, bytecode: &[u8]) -> Result<Output> {
  const PATHNAME: &str = "program/py";
  const FILENAME_PY: &str = "program/py/main.py";

  let mut files = Files::new();

  Writer::write_from_bytes(PATHNAME, FILENAME_PY, bytecode).map(|_| {
    files.add_file(File {
      kind: FileKind::Py,
      pathname: FILENAME_PY.into(),
    })
  })?;

  Ok(Output {
    backend: backend.to_owned(),
    files,
  })
}
