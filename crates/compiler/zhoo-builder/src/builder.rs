//! ...

use super::brick::cranelift;
use super::brick::wasm;

use zhoo_session::backend::BackendKind;
use zhoo_session::session::Session;

use zo_core::Result;

#[derive(Debug)]
struct Builder;

impl Builder {
  #[inline]
  fn build(&self, session: &mut Session, bytecode: &[u8]) -> Result<()> {
    let backend = &session.settings.backend;

    match &backend.kind {
      BackendKind::Cranelift => cranelift::build(backend, bytecode),
      BackendKind::Wasm => wasm::build(backend, bytecode),
    }
  }
}

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn build(session: &mut Session, bytecode: &[u8]) -> Result<()> {
  Builder.build(session, bytecode)
}
