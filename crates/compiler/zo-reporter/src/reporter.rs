use super::error::{Diagnostic, Error};

use swisskit::global::EXIT_FAILURE;
use swisskit::span::source::{SourceId, SourceMap};
use swisskit::span::Span;

/// The representation of a reporter.
#[derive(Clone, Debug)]
pub struct Reporter {
  /// This flag is used for displaying diagnostics.
  ///
  /// `true` means we should display all diagnostics retrieved
  /// `false` means *"shut your mouth and do nothing bro"*.
  has_errors: std::sync::Arc<std::sync::Mutex<bool>>,
  /// The source map of a program.
  source_map: SourceMap,
}

impl Reporter {
  /// Creates a new reporter instance.
  #[inline(always)]
  pub fn new() -> Self {
    Self::default()
  }

  /// Wrapper of [`SourceMap::add_source`].
  pub fn add_source(
    &mut self,
    pathname: impl Into<std::path::PathBuf>,
  ) -> std::io::Result<SourceId> {
    self.source_map.add_source(pathname.into())
  }

  /// Wrapper of [`SourceMap::source_id`].
  fn source_id(&self, span: Span) -> u32 {
    self.source_map.source_id(span)
  }

  /// Wrapper of [`SourceMap::source_code`].
  pub fn source_code(&self, source_id: u32) -> &str {
    self.source_map.source_code(source_id)
  }

  /// Wrapper of [`SourceMap::pathname`].
  #[inline]
  pub fn pathname(&self, span: Span) -> &std::path::Path {
    self.source_map.pathname(span)
  }

  /// Sets the errors flag.
  #[inline]
  pub fn errors(&self, has_errors: bool) {
    // todo(ivs) — implement an internal error.
    match self.has_errors.lock() {
      Err(_) => {}
      Ok(mut mguard) => *mguard = has_errors,
    }
  }

  /// Checks if reporter contains any errors.
  #[inline]
  pub fn has_errors(&self) -> bool {
    // todo(ivs) — implement an internal error.
    match self.has_errors.lock() {
      Err(_) => false,
      Ok(mguard) => *mguard,
    }
  }

  /// Aborts the entire program. Game over, out!
  #[inline(always)]
  fn abort(&self) -> ! {
    std::process::exit(EXIT_FAILURE)
  }

  /// Aborts only if we got errors.
  #[inline]
  pub fn abort_if_has_errors(&self) {
    if self.has_errors() {
      self.abort();
    }
  }

  /// Handles an error and aborts the entire program.
  #[inline]
  pub fn raise(&self, error: Error) -> ! {
    self.add_report(error);
    self.abort()
  }

  /// Adds an diagnostic's error report.
  pub fn add_report(&self, error: Error) {
    let report = match &error {
      Error::Internal(diagnostic) => diagnostic.report(),
      Error::Lexical(diagnostic) => diagnostic.report(),
      Error::Syntax(diagnostic) => diagnostic.report(),
      Error::Semantic(diagnostic) => diagnostic.report(),
      Error::Generate(diagnostic) => diagnostic.report(),
      Error::Eval(diagnostic) => diagnostic.report(),
    };

    let span = report
      .labels
      .first()
      .map(|label| label.0)
      .unwrap_or(Span::ZERO);

    let source_id = self.source_id(span);
    let code = self.source_code(source_id);
    let code = if code.is_empty() { "\n" } else { code };
    let pathname = self.pathname(span).display();

    let mut report_builder = ariadne::Report::build(
      report.kind.into(),
      pathname.to_string(),
      span.lo as usize,
    )
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
}

impl Default for Reporter {
  /// Creates a default reporter instance.
  #[inline(always)]
  fn default() -> Self {
    Self {
      has_errors: std::sync::Arc::default(),
      source_map: SourceMap::new(),
    }
  }
}
