//! ...

use zo_session::session::Session;

use zo_core::reporter::report::io::Io;
use zo_core::reporter::Reporter;
use zo_core::Result;

#[derive(Debug)]
struct Reader<'bytes> {
  reporter: &'bytes mut Reporter,
}

impl<'bytes> Reader<'bytes> {
  #[inline]
  fn new(reporter: &'bytes mut Reporter) -> Self {
    Self { reporter }
  }

  fn read(
    &mut self,
    pathname: impl AsRef<std::ffi::OsStr>,
  ) -> Result<Box<[u8]>> {
    self
      .reporter
      .add_source(pathname.as_ref())
      .map_err(Io::error)
      .map(|source_id| {
        let source_code = self.reporter.code(source_id as u32);
        let source_bytes = source_code.as_bytes();

        source_bytes.into()
      })
  }
}

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn read(session: &mut Session) -> Result<Box<[u8]>> {
  Reader::new(&mut session.reporter).read(session.settings.input.as_str())
}
