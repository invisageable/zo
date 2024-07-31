use super::Span;

/// The representation of a source id.
#[derive(Clone, Copy, Debug)]
pub struct SourceId(usize);

impl SourceId {
  /// Creates a new source id.
  #[inline]
  pub fn new(id: usize) -> Self {
    Self(id)
  }
}

/// The representation of a source file.
#[derive(Clone, Debug)]
pub struct Source {
  pub id: SourceId,
  pub pathname: std::path::PathBuf,
}

impl Source {
  /// Creates a new source from id and a pathname
  #[inline]
  pub fn new(id: usize, pathname: impl Into<std::path::PathBuf>) -> Self {
    Self {
      id: SourceId(id),
      pathname: pathname.into(),
    }
  }
}

/// The representation of a source map database.
pub struct SourceMap {
  pub code: String,
  pub sources: Vec<Source>,
}

impl SourceMap {
  /// The offset constant of sources.
  const OFFSET: usize = 1;

  /// Creates a new source map.
  #[inline]
  pub fn new() -> Self {
    Self {
      code: String::with_capacity(0usize),
      sources: Vec::with_capacity(0usize),
    }
  }

  /// Adds a new source to the map from a pathname.
  pub fn add_source(
    &mut self,
    pathname: std::path::PathBuf,
  ) -> std::io::Result<SourceId> {
    use std::io::Read;

    let id = self.sources.len();
    let offset = self.code.len();
    let file = std::fs::File::open(&pathname)?;
    let mut buf_reader = std::io::BufReader::new(file);

    buf_reader.read_to_string(&mut self.code)?;
    self.sources.push(Source::new(offset, &pathname));

    Ok(SourceId::new(id))
  }

  /// Gets the related source id from span.
  pub fn source_id(&self, span: Span) -> u32 {
    self
      .sources
      .iter()
      .enumerate()
      .find(|(_, s)| s.id.0 > span.lo)
      .map(|(idx, _)| idx - Self::OFFSET)
      .unwrap_or(self.sources.len() - Self::OFFSET) as u32
  }

  /// Gets the source code from the related source id.
  pub fn source_code(&self, id: u32) -> &str {
    let source_id = id as usize;
    let lo = self.sources[source_id].id;

    let hi = self
      .sources
      .get(source_id + Self::OFFSET)
      .map(|s| s.id)
      .unwrap_or(SourceId::new(self.code.len()));

    &self.code[lo.0..hi.0]
  }

  /// Gets the pathname of a source from a span.
  #[inline]
  pub fn pathname(&self, span: Span) -> &std::path::Path {
    &self.sources[self.source_id(span) as usize].pathname
  }
}
