use super::brick::js;
use super::brick::py;

use zhoo_session::backend::BackendKind;
use zhoo_session::session::Session;

use zo_core::Result;

pub fn build(session: &mut Session, bytecode: &[u8]) -> Result<()> {
  match &session.settings.backend.kind {
    BackendKind::Js => js::build(bytecode),
    BackendKind::Py => py::build(bytecode),
    backend => panic!("backend `{backend} not supported.`"),
  }
}
