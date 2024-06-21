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
pub fn build(backend: &Backend, _bytecode: &[u8]) -> Result<Output> {
  const PATHNAME: &str = "program/clif";
  const FILENAME_CLIF: &str = "program/clif/main.exe";

  let mut files = Files::new();

  Ok(Output {
    backend: backend.to_owned(),
    files,
  })
}
