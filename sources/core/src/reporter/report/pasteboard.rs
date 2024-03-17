use super::{Error, Report, ReportError, ReportKind, REPORT_TITLE_ERROR};

use crate::color;
use crate::span::Span;

use ariadne::Fmt;

#[derive(Debug)]
pub enum Pasteboard {
  NotSupported(String),
  Unknown(String),
}

impl Pasteboard {
  #[inline]
  pub fn not_supported(error: impl ToString) -> ReportError {
    ReportError::Pasteboard(Pasteboard::NotSupported(error.to_string()))
  }

  #[inline]
  pub fn unknown(error: impl ToString) -> ReportError {
    ReportError::Pasteboard(Pasteboard::Unknown(error.to_string()))
  }
}

impl Error for Pasteboard {
  fn report(&self) -> Report {
    match self {
      Self::NotSupported(error) => Report {
        kind: ReportKind::Error(REPORT_TITLE_ERROR),
        message: format!(
          "{}",
          "clipboard is not supported.".fg(color::title())
        )
        .into(),
        labels: vec![(Span::ZERO, error.into(), color::error())],
        ..Default::default()
      },
      Self::Unknown(error) => Report {
        kind: ReportKind::Error(REPORT_TITLE_ERROR),
        message: format!("{}", "unknown error.".fg(color::title())).into(),
        labels: vec![(Span::ZERO, error.into(), color::error())],
        ..Default::default()
      },
    }
  }
}
