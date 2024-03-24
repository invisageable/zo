use super::{Error, Report, ReportKind, REPORT_TITLE_WARNING};

use crate::color;
use crate::span::Span;

use ariadne::Fmt;

#[derive(Debug)]
pub enum Semantic {
  InvalidIndex(Span, String),
  NameClash(Span, String),
  NamingConvention(Span, String, String),
  NotFoundEntry(Span, String),
  NotFoundFun(Span, String),
  NotFoundIdent(Span, String),
  OutOfLoop(String),
  TypeMismatch(Span, String, String),
}

impl Error for Semantic {
  fn report(&self) -> Report {
    match self {
      Self::InvalidIndex(span, name) => todo!("{span}-{name}"),
      Self::NameClash(span, name) => todo!("{span}-{name}"),
      Self::NamingConvention(span, ident, naming) => Report {
        kind: ReportKind::Warning(REPORT_TITLE_WARNING),
        message: format!(
          "{} {} {} {}",
          "variable".fg(color::title()),
          format!("`{ident}`").fg(color::hint()),
          "should have a".fg(color::title()),
          format!("`{naming}`").fg(color::title()),
        )
        .into(),
        labels: vec![(
          *span,
          format!("change this identifier to {naming} convention: `{ident}`")
            .into(),
          color::warning(),
        )],
        ..Default::default()
      },
      Self::NotFoundEntry(span, string) => todo!("{span}-{string}"),
      Self::NotFoundFun(span, string) => todo!("{span}-{string}"),
      Self::NotFoundIdent(span, string) => todo!("{span}-{string}"),
      Self::OutOfLoop(name) => todo!("{name}"),
      Self::TypeMismatch(span, lhs, rhs) => todo!("{span}-{lhs}-{rhs}"),
    }
  }
}
