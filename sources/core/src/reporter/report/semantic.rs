use super::{Error, Report, ReportKind, REPORT_TITLE_WARNING};

use crate::color;
use crate::span::Span;

use ariadne::Fmt;

#[derive(Debug)]
pub enum Semantic {
  FunClash(Span, String),
  InvalidIndex(Span, String),
  NamingConvention(Span, String, String),
  NotFoundEntry(Span, String),
  NotFoundFun(Span, String),
  NotFoundIdent(Span, String),
  OutOfLoop(String),
  TyClash(Span, String),
  TypeMismatch(Span, String, String),
  VarClash(Span, String),
}

impl Error for Semantic {
  fn report(&self) -> Report {
    match self {
      Self::FunClash(span, name) => todo!("{span}-{name}"),
      Self::InvalidIndex(span, name) => todo!("{span}-{name}"),
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
      Self::TyClash(span, name) => todo!("{span}-{name}"),
      Self::TypeMismatch(span, lhs, rhs) => todo!("{span}-{lhs}-{rhs}"),
      Self::VarClash(span, name) => todo!("{span}-{name}"),
    }
  }
}
