use super::{Error, Report};

use crate::span::Span;

#[derive(Debug)]
pub enum Lexical {
  Dummy,
  Unknown(Span, char),
}

impl Error for Lexical {
  fn report(&self) -> Report {
    match self {
      Self::Dummy => Report::default(),
      Self::Unknown(span, ch) => todo!("{span}-{ch}"),
    }
  }
}
