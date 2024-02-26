pub mod report;

use super::source::SourceMap;
use super::span::Span;

use report::{Error, ReportError};

#[derive(Debug)]
pub struct Reporter {
  has_errors: std::sync::Arc<std::sync::Mutex<bool>>,
  source_map: SourceMap,
}

impl Reporter {
  pub fn new() -> Self {
    Self {
      has_errors: std::sync::Arc::new(std::sync::Mutex::new(false)),
      source_map: SourceMap::new(),
    }
  }

  pub fn add_source(
    &mut self,
    pathname: impl Into<std::path::PathBuf>,
  ) -> std::io::Result<usize> {
    self.source_map.add_source(pathname.into())
  }

  pub fn code(&self, source_id: u32) -> &str {
    self.source_map.code(source_id)
  }

  #[allow(dead_code)]
  fn source_id(&self, span: Span) -> u32 {
    self.source_map.source_id(span)
  }

  pub fn pathname(&self, span: Span) -> &std::path::Path {
    self.source_map.pathname(span)
  }

  pub fn errors(&self, has_errors: bool) {
    // self
    // .has_errors
    // .lock()
    // .map(|mut has_errors| {
    //   *has_errors = true;
    // })
    // .unwrap();

    *self.has_errors.lock().unwrap() = has_errors;
  }

  pub fn has_errors(&self) -> bool {
    *self.has_errors.lock().unwrap()
  }

  pub fn add_report(&self, error: ReportError) {
    let _report = match error {
      ReportError::Io(error) => error.report(),
      ReportError::Chan(error) => error.report(),
      ReportError::Lexical(error) => error.report(),
      ReportError::Syntax(error) => error.report(),
      ReportError::Semantic(error) => error.report(),
      ReportError::Assembly(error) => error.report(),
    };

    self.errors(true);
  }

  pub fn raise(&self, error: ReportError) -> ! {
    self.add_report(error);
    self.abort()
  }

  pub fn abort_if_has_errors(&self) {
    if *self.has_errors.lock().unwrap() {
      self.abort();
    }
  }

  fn abort(&self) -> ! {
    std::process::exit(1i32)
  }
}

impl Default for Reporter {
  fn default() -> Self {
    Self::new()
  }
}
