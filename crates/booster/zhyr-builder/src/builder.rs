use zo_core::writer::Writer;
use zo_core::Result;

pub fn build(bytecode: &[u8]) -> Result<()> {
  const PATHNAME: &str = "program/zo";
  const FILENAME_ZO: &str = "program/zo/main.zo";

  Writer::write_from_bytes(PATHNAME, FILENAME_ZO, bytecode)?;

  Ok(())
}
