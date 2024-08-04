use zo_reporter::Result;
use zo_session::backend::Backend;
use zo_session::session::Session;

// The representation of a buiulder.
#[derive(Debug)]
struct Builder;

impl Builder {
  /// Builds the output result from session and bytecode.
  #[inline]
  fn build(&self, session: &mut Session, _bytecode: &[u8]) -> Result<Output> {
    match session.settings.backend {
      Backend::Py => Ok(Output),
      Backend::Wasm => Ok(Output),
      _ => panic!(),
    }
  }
}

/// Builds the output result from session and bytecode — see also
/// [`Builder::build`].
pub fn build(session: &mut Session, bytecode: &[u8]) -> Result<Output> {
  Builder.build(session, bytecode)
}

/// The output information — pathname, backend, etc.
#[derive(Clone, Debug)]
pub struct Output;

impl std::fmt::Display for Output {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    todo!()
  }
}
