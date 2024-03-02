use zhoo_session::backend::Backend;

use zo_core::writer::Writer;
use zo_core::Result;

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn build(_backend: &Backend, bytecode: &[u8]) -> Result<()> {
  const PATHNAME: &str = "program/wasm";
  const FILENAME_WAT: &str = "program/wasm/main.wat";
  const FILENAME_WASM: &str = "program/wasm/main.wasm";

  Writer::write_from_bytes(PATHNAME, FILENAME_WAT, bytecode)?;

  Writer::write_from_bytes(
    PATHNAME,
    FILENAME_WASM,
    &wat::parse_bytes(bytecode).unwrap(),
  )?;

  Ok(())
}
