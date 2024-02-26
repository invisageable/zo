use super::span::Span;

use std::io::Read;

#[derive(Clone, Debug)]
pub struct Source {
  pub id: usize,
  pub path: std::path::PathBuf,
}

impl Source {
  #[inline]
  pub fn new<P: Into<std::path::PathBuf>>(id: usize, path: P) -> Self {
    Self {
      id,
      path: path.into(),
    }
  }
}

#[derive(Clone, Debug, Default)]
pub struct SourceMap {
  pub code: String,
  pub sources: Vec<Source>,
}

impl SourceMap {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn add_source(
    &mut self,
    pathname: std::path::PathBuf,
  ) -> std::io::Result<usize> {
    let source_id = self.sources.len() as u32;
    let offset = self.code.len();
    let file = std::fs::File::open(&pathname)?;
    let mut buf_reader = std::io::BufReader::new(file);

    buf_reader.read_to_string(&mut self.code)?;
    self.sources.push(Source::new(offset, &pathname));

    Ok(source_id as usize)
  }

  pub fn code(&self, source_id: u32) -> &str {
    let source_id = source_id as usize;
    let lo = self.sources[source_id].id;

    let hi = self
      .sources
      .get(source_id + 1)
      .map(|s| s.id)
      .unwrap_or(self.code.len());

    &self.code[lo..hi]
  }

  pub fn source_id(&self, span: Span) -> u32 {
    self
      .sources
      .iter()
      .enumerate()
      .find(|(_, s)| s.id > span.lo)
      .map(|(i, _)| i - 1)
      .unwrap_or(self.sources.len() - 1) as u32
  }

  pub fn pathname(&self, span: Span) -> &std::path::Path {
    &self.sources[self.source_id(span) as usize].path
  }
}
