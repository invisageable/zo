use zo_reporter::error::io::Io;
use zo_reporter::reporter::Reporter;
use zo_reporter::Result;
use zo_session::session::Session;

/// The representation of a reader.
#[derive(Debug)]
struct Reader<'path> {
  reporter: &'path mut Reporter,
}

impl<'path> Reader<'path> {
  /// Creates a new reader instance.
  #[inline]
  fn new(reporter: &'path mut Reporter) -> Self {
    Self { reporter }
  }

  /// Reads from file.
  ///
  /// A wrapper of [`Reporter::add_source`] to generates the source map of the
  /// program.
  ///
  /// #### result.
  ///
  /// The resulting is a vector of bytes.
  fn read(&mut self, pathname: impl AsRef<std::ffi::OsStr>) -> Result<Vec<u8>> {
    self
      .reporter
      .add_source(pathname.as_ref())
      .map_err(Io::error)
      .map(|source_id| {
        let source_code = self.reporter.source_code(source_id.get() as u32);
        let source_bytes = source_code.as_bytes();

        source_bytes.into()
      })
  }
}

/// A wrapper of [`Reader::new`] and [`Reader::read`].
///
/// ```ignore
/// use zo_reader::reader;
/// use zo_session::session::Session;
///
/// let mut session = Session::default();
///
/// reader::read(&mut session, "path/to/file");
/// ```
pub fn read(
  session: &mut Session,
  pathname: impl AsRef<std::ffi::OsStr>,
) -> Result<Vec<u8>> {
  Reader::new(&mut session.reporter).read(pathname)
}
