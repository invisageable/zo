use zo_reporter::reporter::Reporter;
use zo_reporter::{error, Result};
use zo_session::session::Session;

/// The representation of a reader.
struct Reader<'path> {
  /// A reporter — see also [`Reporter`] for more information.
  reporter: &'path mut Reporter,
}

impl<'path> Reader<'path> {
  /// Creates a new reader instance.
  #[inline(always)]
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
      .map_err(error::internal::io)
      .map(|source_id| {
        let source_code = self.reporter.source_code(source_id.get() as u32);
        let source_bytes = source_code.as_bytes();

        source_bytes.into()
      })
  }

  /// Reads file from pathname.
  fn read_file(
    &self,
    pathname: impl AsRef<std::ffi::OsStr>,
  ) -> Result<Box<[u8]>> {
    use std::io::Read;

    let pathname = std::path::Path::new(&pathname);
    let file = std::fs::File::open(pathname).map_err(error::internal::io)?;
    let metadata = file.metadata().map_err(error::internal::io)?;
    let mut source_code = String::with_capacity(metadata.len() as usize);

    std::io::BufReader::new(file)
      .read_to_string(&mut source_code)
      .map_err(error::internal::io)?;

    Ok(source_code.as_bytes().into())
  }

  /// Reads line.
  fn read_line(&self) -> Result<Vec<u8>> {
    use std::io::Write;

    let stdout = std::io::stdout();
    let stdin = std::io::stdin();
    let mut input = String::with_capacity(0usize);

    stdout.lock().flush().map_err(error::internal::io)?;
    print!("📡 ");
    stdout.lock().flush().map_err(error::internal::io)?;
    stdin.read_line(&mut input).map_err(error::internal::io)?;

    let line = input.as_bytes().into();

    input.clear();

    Ok(line)
  }
}

/// A wrapper of [`Reader::new`] and [`Reader::read`].
///
/// #### examples.
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

/// A wrapper of [`Reader::new`] and [`Reader::read_file`].
///
/// #### examples.
///
/// ```ignore
/// use zo_reader::reader;
/// use zo_session::session::Session;
///
/// let mut session = Session::default();
///
/// reader::read_file(&mut session, "path/to/file");
/// ```
pub fn read_file(session: &mut Session) -> Result<Box<[u8]>> {
  Reader::new(&mut session.reporter).read_file(session.settings.input.as_str())
}

/// A wrapper of [`Reader::new`] and [`Reader::read_line`].
///
/// #### examples.
///
/// ```ignore
/// use zo_reader::reader;
/// use zo_session::session::Session;
///
/// let mut session = Session::default();
///
/// reader::read_line();
/// ```
pub fn read_line(session: &mut Session) -> Result<Vec<u8>> {
  Reader::new(&mut session.reporter).read_line()
}
