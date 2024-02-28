use super::{Error, Report};

use crate::span::Span;

#[derive(Debug)]
pub enum Lexical {
  Dummy,
  Unknown(Span, char),
  InvalidNum(Span, char),
}

impl Error for Lexical {
  fn report(&self) -> Report {
    match self {
      Self::Dummy => Report::default(),
      Self::Unknown(span, ch) => todo!("unknown char — {span}-{ch}"),
      Self::InvalidNum(span, ch) => todo!("invalid num — {span}-{ch}"),
    }
  }
}
