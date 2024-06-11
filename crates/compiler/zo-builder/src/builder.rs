//! ...

use super::brick::py;
use super::brick::wasm;
use super::output::Output;

use zo_session::backend::BackendKind;
use zo_session::session::Session;

use zo_core::Result;

#[derive(Debug)]
struct Builder;

impl Builder {
  #[inline]
  fn build(&self, session: &mut Session, bytecode: &[u8]) -> Result<Output> {
    match &session.settings.backend.kind {
      BackendKind::Py => py::build(&session.settings.backend, bytecode),
      BackendKind::Wasm => wasm::build(&session.settings.backend, bytecode),
    }
  }
}

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn build(session: &mut Session, bytecode: &[u8]) -> Result<Output> {
  Builder.build(session, bytecode)
}
