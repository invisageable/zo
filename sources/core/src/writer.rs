use super::Result;

use crate::reporter::report::io::Io;

use std::fmt::Write;

pub struct Writer {
  out: String,
  int: itoa::Buffer,
  float: ryu::Buffer,
  indent: usize,
  depth: usize,
}

impl Writer {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn as_bytes(&mut self) -> Box<[u8]> {
    self.out.as_bytes().into()
  }

  pub fn indent(&mut self) {
    self.depth += 1;
  }

  pub fn dedent(&mut self) {
    if self.depth > 0 {
      self.depth -= 1;
    }
  }

  pub fn new_line(&mut self) -> Result<()> {
    writeln!(self.out).map(|_| Ok(())).unwrap()
  }

  pub fn space(&mut self) -> Result<()> {
    write!(self.out, " ").map(|_| Ok(())).unwrap()
  }

  pub fn comma(&mut self) -> Result<()> {
    write!(self.out, ",").map(|_| Ok(())).unwrap()
  }

  pub fn period(&mut self) -> Result<()> {
    write!(self.out, ".").map(|_| Ok(())).unwrap()
  }

  pub fn colon(&mut self) -> Result<()> {
    write!(self.out, ":").map(|_| Ok(())).unwrap()
  }

  pub fn semicolon(&mut self) -> Result<()> {
    write!(self.out, ";").map(|_| Ok(())).unwrap()
  }

  // pub fn write(&mut self, code: impl std::fmt::Display) -> Result<()> {
  //   write!(self.out, "{code}").map(|_| Ok(())).unwrap()
  // }

  pub fn write(&mut self, code: impl std::fmt::Display) -> Result<()> {
    let code = format!("{}{}", " ".repeat(self.indent * self.depth), code);

    write!(self.out, "{code}").map(|_| Ok(())).unwrap()
  }

  pub fn write_int(&mut self, int: impl itoa::Integer) -> Result<()> {
    write!(self.out, "{}", self.int.format(int))
      .map(|_| Ok(()))
      .unwrap()
  }

  pub fn write_float(&mut self, float: impl ryu::Float) -> Result<()> {
    write!(self.out, "{}", self.float.format(float))
      .map(|_| Ok(()))
      .unwrap()
  }

  pub fn write_bytes(&mut self, code: &[u8]) -> Result<()> {
    write!(self.out, "{}", String::from_utf8_lossy(code))
      .map(|_| Ok(()))
      .unwrap()
  }

  pub fn writeln(&mut self, code: impl std::fmt::Display) -> Result<()> {
    let code = format!("{}{}", " ".repeat(self.indent * self.depth), code);

    writeln!(self.out, "{code}").map(|_| Ok(())).unwrap()
  }

  pub fn writeln_bytes(&mut self, code: &[u8]) -> Result<()> {
    writeln!(self.out, "{}", String::from_utf8_lossy(code))
      .map(|_| Ok(()))
      .unwrap()
  }

  pub fn write_from_bytes<'bytes>(
    pathname: &str,
    filename: &str,
    bytes: &'bytes [u8],
  ) -> Result<Bytes<'bytes>> {
    let filename = std::path::Path::new(filename);

    make_dir(pathname)?;
    make_file(filename.display(), bytes)?;

    Ok(Bytes {
      raw: bytes,
      filename: filename.to_path_buf(),
    })
  }
}

impl Default for Writer {
  fn default() -> Self {
    Self {
      out: String::default(),
      int: itoa::Buffer::default(),
      float: ryu::Buffer::default(),
      indent: 2usize,
      depth: 0usize,
    }
  }
}

impl std::fmt::Debug for Writer {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{self}")
  }
}

impl std::fmt::Display for Writer {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.out)
  }
}

pub fn make_dir(pathname: impl AsRef<std::path::Path>) -> Result<()> {
  if pathname.as_ref().is_dir() {
    return Ok(());
  }

  std::fs::create_dir_all(pathname).map_err(Io::error)
}

pub fn make_file(
  pathname: impl ToString,
  bytes: impl AsRef<[u8]>,
) -> Result<()> {
  use std::io::Write;

  std::fs::File::create(pathname.to_string())
    .map(|mut file| file.write_all(bytes.as_ref()).map_err(Io::error))
    .unwrap()
}

#[derive(Debug)]
pub struct Bytes<'bytes> {
  pub raw: &'bytes [u8],
  pub filename: std::path::PathBuf,
}
