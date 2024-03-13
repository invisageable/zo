//! ...

use super::brick::arm;
use super::brick::clif;
use super::brick::js;
use super::brick::llvm;
use super::brick::py;
use super::brick::wasm;

use zhoo_session::backend::BackendKind;
use zhoo_session::session::Session;

use zo_core::Result;

#[derive(Debug)]
struct Builder;

impl Builder {
  #[inline]
  fn build(&self, session: &mut Session, bytecode: &[u8]) -> Result<()> {
    match &session.settings.backend.kind {
      BackendKind::Arm => arm::build(bytecode),
      BackendKind::Clif => clif::build(bytecode),
      BackendKind::Js => js::build(bytecode),
      BackendKind::Llvm => llvm::build(bytecode),
      BackendKind::Py => py::build(bytecode),
      BackendKind::Wasm => wasm::build(bytecode),
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
