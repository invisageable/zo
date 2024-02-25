use super::{Error, Report};

use crate::span::Span;

#[derive(Debug)]
pub enum Syntax {
  Dummy,
  UnexpectedToken(Span, String),
}

impl Error for Syntax {
  fn report(&self) -> Report {
    match self {
      Self::Dummy => Report::default(),
      Self::UnexpectedToken(span, token) => todo!("{span}-{token}"),
    }
  }
}
