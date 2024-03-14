use super::{Error, Report, ReportError, ReportKind, REPORT_TITLE_ERROR};

use crate::color;
use crate::span::Span;

use ariadne::Fmt;

#[derive(Debug)]
pub enum Chan {
  NotFoundSignal(String),
}

impl Error for Chan {
  fn report(&self) -> Report {
    match self {
      Self::NotFoundSignal(error) => Report {
        kind: ReportKind::Error(REPORT_TITLE_ERROR),
        message: format!("{}", "no signal found.".fg(color::title())).into(),
        labels: vec![(Span::ZERO, error.into(), color::error())],
        ..Default::default()
      },
    }
  }
}

impl Chan {
  pub fn error(error: impl ToString) -> ReportError {
    ReportError::Chan(Chan::NotFoundSignal(error.to_string()))
  }
}
