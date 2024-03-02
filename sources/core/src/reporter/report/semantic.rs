use super::{Error, Report};

use crate::span::Span;

#[derive(Debug)]
pub enum Semantic {
  NotFoundIdent(Span, String),
  NotFoundEntry(Span, String),
  NotFoundFun(Span, String),
}

impl Error for Semantic {
  fn report(&self) -> Report {
    match self {
      Self::NotFoundIdent(span, string) => todo!("{span}-{string}"),
      Self::NotFoundEntry(span, string) => todo!("{span}-{string}"),
      Self::NotFoundFun(span, string) => todo!("{span}-{string}"),
    }
  }
}
