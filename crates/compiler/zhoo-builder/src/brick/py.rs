use zo_core::writer::Writer;
use zo_core::Result;

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn build(bytecode: &[u8]) -> Result<()> {
  const PATHNAME: &str = "program/py";
  const FILENAME_PY: &str = "program/py/main.py";

  Writer::write_from_bytes(PATHNAME, FILENAME_PY, bytecode)?;

  Ok(())
}
