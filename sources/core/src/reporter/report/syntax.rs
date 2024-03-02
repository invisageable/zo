use super::{Error, Report, ReportKind, REPORT_TITLE_ERROR};

use crate::color;
use crate::span::Span;

use ariadne::Fmt;

#[derive(Debug)]
pub enum Syntax {
  UnexpectedToken(Span, String),
  ExpectedItem(Span, String),
}

impl Error for Syntax {
  fn report(&self) -> Report {
    match self {
      Self::UnexpectedToken(span, token) => Report {
        kind: ReportKind::Error(REPORT_TITLE_ERROR),
        message: format!("{}", "unexpected token.".fg(color::title())).into(),
        labels: vec![(
          *span,
          format!(
            "{}: `{token}`",
            "what's this language i only spoke zhoo lang".fg(color::error()),
          )
          .into(),
          color::error(),
        )],
        ..Default::default()
      },
      Self::ExpectedItem(span, token) => Report {
        kind: ReportKind::Error(REPORT_TITLE_ERROR),
        message: format!("{}", "expected item.".fg(color::title())).into(),
        labels: vec![(
          *span,
          format!(
            "{}: `{token}`",
            "what's this language i only spoke zhoo lang".fg(color::error()),
          )
          .into(),
          color::error(),
        )],
        ..Default::default()
      },
    }
  }
}
