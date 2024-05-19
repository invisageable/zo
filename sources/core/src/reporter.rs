//! The [`Reporter`] used inside the compiler stack. It contains — `error`
//! messages and data structures to displays friendly errors.

pub mod report;

use super::source::SourceMap;
use super::span::Span;
use super::EXIT_FAILURE;

use report::{Error, ReportError};

#[derive(Debug)]
pub struct Reporter {
  has_errors: std::sync::Arc<std::sync::Mutex<bool>>,
  source_map: SourceMap,
}

impl Reporter {
  #[inline]
  pub fn new() -> Self {
    Self::default()
  }

  #[inline]
  pub fn add_source(
    &mut self,
    pathname: impl Into<std::path::PathBuf>,
  ) -> std::io::Result<usize> {
    self.source_map.add_source(pathname.into())
  }

  #[inline]
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

  #[inline]
  pub fn errors(&self, has_errors: bool) {
    *self.has_errors.lock().unwrap() = has_errors;
  }

  #[inline]
  pub fn has_errors(&self) -> bool {
    *self.has_errors.lock().unwrap()
  }

  pub fn add_report(&self, error: ReportError) {
    let report = match &error {
      ReportError::Io(error) => error.report(),
      ReportError::Chan(error) => error.report(),
      ReportError::Lexical(error) => error.report(),
      ReportError::Syntax(error) => error.report(),
      ReportError::Semantic(error) => error.report(),
      ReportError::Eval(error) => error.report(),
    };

    let span = report
      .labels
      .first()
      .map(|label| label.0)
      .unwrap_or(Span::ZERO);

    let source_id = self.source_id(span);
    let code = self.code(source_id);
    let code = if code.is_empty() { "\n" } else { code };
    let pathname = self.pathname(span).display();

    let mut report_builder =
      ariadne::Report::build(report.kind.into(), pathname.to_string(), span.lo)
        .with_code(error)
        .with_message(report.message);

    for (span, message, color) in report.labels {
      report_builder = report_builder.with_label(
        ariadne::Label::new((pathname.to_string(), span.into()))
          .with_message(message)
          .with_color(color),
      );
    }

    for note in report.notes {
      report_builder = report_builder.with_note(note);
    }

    for help in report.helps {
      report_builder = report_builder.with_help(help);
    }

    eprintln!();

    report_builder
      .with_config(ariadne::Config::default())
      .finish()
      .write((pathname.to_string(), code.into()), std::io::stderr())
      .unwrap();

    self.errors(true);
  }

  #[inline]
  pub fn raise(&self, error: ReportError) -> ! {
    self.add_report(error);
    self.abort()
  }

  #[inline]
  pub fn abort_if_has_errors(&self) {
    if self.has_errors() {
      self.abort();
    }
  }

  #[inline]
  fn abort(&self) -> ! {
    std::process::exit(EXIT_FAILURE)
  }
}

impl Default for Reporter {
  fn default() -> Self {
    Self {
      has_errors: std::sync::Arc::new(std::sync::Mutex::new(false)),
      source_map: SourceMap::new(),
    }
  }
}
