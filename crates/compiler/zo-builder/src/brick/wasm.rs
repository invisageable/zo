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
  const PATHNAME: &str = "program/wasm";
  const FILENAME_WAT: &str = "program/wasm/main.wat";
  const FILENAME_WASM: &str = "program/wasm/main.wasm";

  let mut files = Files::new();

  Writer::write_from_bytes(PATHNAME, FILENAME_WAT, bytecode).map(|_| {
    files.add_file(File {
      kind: FileKind::Wat,
      pathname: FILENAME_WAT.into(),
    })
  })?;

  Writer::write_from_bytes(
    PATHNAME,
    FILENAME_WASM,
    &wat::parse_bytes(bytecode).unwrap(),
  )
  .map(|_| {
    files.add_file(File {
      kind: FileKind::Wasm,
      pathname: FILENAME_WASM.into(),
    })
  })?;

  Ok(Output {
    backend: backend.to_owned(),
    files,
  })
}
