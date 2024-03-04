use super::{Error, Report, ReportKind, REPORT_TITLE_WARNING};

use crate::color;
use crate::span::Span;

use ariadne::Fmt;

#[derive(Debug)]
pub enum Semantic {
  NotFoundIdent(Span, String),
  NotFoundEntry(Span, String),
  NotFoundFun(Span, String),
  NamingConvention(String, String, Span),
}

impl Error for Semantic {
  fn report(&self) -> Report {
    match self {
      Self::NotFoundIdent(span, string) => todo!("{span}-{string}"),
      Self::NotFoundEntry(span, string) => todo!("{span}-{string}"),
      Self::NotFoundFun(span, string) => todo!("{span}-{string}"),
      Self::NamingConvention(identifier, naming, span) => Report {
        kind: ReportKind::Warning(REPORT_TITLE_WARNING),
        message: format!(
          "{} {} {} {}",
          "variable".fg(color::title()),
          format!("`{identifier}`").fg(color::hint()),
          "should have a".fg(color::title()),
          format!("`{naming}`").fg(color::title()),
        )
        .into(),
        labels: vec![(
          *span,
          format!(
            "change this identifier to {naming} convention: `{identifier}`"
          )
          .into(),
          color::warning(),
        )],
        ..Default::default()
      },
    }
  }
}
