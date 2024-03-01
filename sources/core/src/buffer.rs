use super::Result;

use std::fmt::Write;

const IDENT_DEFAULT: usize = 2usize;

pub struct Buffer {
  indent: usize,
  depth: usize,
  out: String,
  int: itoa::Buffer,
  float: ryu::Buffer,
}

impl Buffer {
  #[inline]
  pub fn new(indent: usize) -> Self {
    Self {
      indent,
      depth: 0usize,
      out: String::default(),
      int: itoa::Buffer::default(),
      float: ryu::Buffer::default(),
    }
  }

  #[inline]
  pub fn as_bytes(&mut self) -> Box<[u8]> {
    self.out.as_bytes().into()
  }

  #[inline]
  pub fn indent(&mut self) {
    self.depth += 1;
  }

  #[inline]
  pub fn dedent(&mut self) {
    if self.depth > 0 {
      self.depth -= 1;
    }
  }

  #[inline]
  pub fn colon(&mut self) -> Result<()> {
    self.write(':')
  }

  #[inline]
  pub fn semicolon(&mut self) -> Result<()> {
    self.write(';')
  }

  #[inline]
  pub fn write(&mut self, code: impl std::fmt::Display) -> Result<()> {
    let code = format!("{}{}", " ".repeat(self.indent * self.depth), code);

    write!(self.out, "{code}").map(|_| Ok(())).unwrap()
  }

  #[inline]
  pub fn write_int(&mut self, int: impl itoa::Integer) -> Result<()> {
    write!(self.out, "{}", self.int.format(int))
      .map(|_| Ok(()))
      .unwrap()
  }

  #[inline]
  pub fn write_float(&mut self, float: impl ryu::Float) -> Result<()> {
    write!(self.out, "{}", self.float.format(float))
      .map(|_| Ok(()))
      .unwrap()
  }

  #[inline]
  pub fn write_bytes(&mut self, code: &[u8]) -> Result<()> {
    write!(self.out, "{}", String::from_utf8_lossy(code))
      .map(|_| Ok(()))
      .unwrap()
  }

  #[inline]
  pub fn writeln(&mut self, code: impl std::fmt::Display) -> Result<()> {
    let code = format!("{}{}", " ".repeat(self.indent * self.depth), code);

    writeln!(self.out, "{code}").map(|_| Ok(())).unwrap()
  }

  #[inline]
  pub fn writeln_bytes(&mut self, code: &[u8]) -> Result<()> {
    writeln!(self.out, "{}", String::from_utf8_lossy(code))
      .map(|_| Ok(()))
      .unwrap()
  }
}

impl Default for Buffer {
  fn default() -> Self {
    Self::new(IDENT_DEFAULT)
  }
}

impl std::fmt::Debug for Buffer {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{self}")
  }
}

impl std::fmt::Display for Buffer {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.out)
  }
}
