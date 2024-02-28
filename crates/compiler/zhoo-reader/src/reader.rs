use zhoo_session::session::Session;

use zo_core::reporter::report::io::Io;
use zo_core::reporter::Reporter;
use zo_core::Result;

use std::io::{Read, Write};

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

  fn read_file(
    &self,
    pathname: impl AsRef<std::ffi::OsStr>,
  ) -> Result<Box<[u8]>> {
    std::fs::File::open(std::path::Path::new(&pathname))
      .map_err(Io::error)
      .and_then(|file| {
        file.metadata().map_err(Io::error).and_then(|metadata| {
          let mut source_code = String::with_capacity(metadata.len() as usize);

          std::io::BufReader::new(file)
            .read_to_string(&mut source_code)
            .map_err(Io::error)?;

          Ok(source_code.as_bytes().into())
        })
      })
  }

  fn read_line(&self) -> Result<Box<[u8]>> {
    let stdout = std::io::stdout();
    let stdin = std::io::stdin();
    let mut buf = String::with_capacity(0usize);

    stdout.lock().flush().map_err(Io::error)?;
    print!("📡 ");
    stdout.lock().flush().map_err(Io::error)?;
    stdin.read_line(&mut buf).map_err(Io::error)?;

    Ok(buf.as_bytes().into())
  }
}

/// ...
pub fn read(session: &mut Session) -> Result<Box<[u8]>> {
  Reader::new(&mut session.reporter).read(session.settings.input.as_str())
}

/// ...
pub fn read_file(session: &mut Session) -> Result<Box<[u8]>> {
  Reader::new(&mut session.reporter).read_file(session.settings.input.as_str())
}

/// ...
pub fn read_line(session: &mut Session) -> Result<Box<[u8]>> {
  Reader::new(&mut session.reporter).read_line()
}
