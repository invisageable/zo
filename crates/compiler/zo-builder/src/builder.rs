use super::output::Output;

use zo_reporter::{error, Result};
use zo_session::backend::Backend;
use zo_session::session::Session;

// The representation of a buiulder.
#[derive(Debug)]
struct Builder;

impl Builder {
  /// Builds the output result from session and bytecode.
  #[inline]
  fn build(&self, session: &mut Session, _bytecode: &[u8]) -> Result<Output> {
    match &session.settings.backend {
      Backend::Py => Ok(Output::default()),
      Backend::Wasm => Ok(Output::default()),
      backend => Err(error::internal::expected_backend(
        vec![Backend::Py, Backend::Wasm],
        *backend,
      )),
    }
  }
}

/// Builds the output result from session and bytecode — see also
/// [`Builder::build`].
pub fn build(session: &mut Session, bytecode: &[u8]) -> Result<Output> {
  Builder.build(session, bytecode)
}
