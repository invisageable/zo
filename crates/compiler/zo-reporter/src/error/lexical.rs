use super::{Diagnostic, Error};

use crate::report::Report;

use swisskit::span::Span;

/// The representation of lexical analysis errors.
#[derive(Debug)]
pub enum Lexical {
  /// The unknown character error.
  Unknown(Span, u8),
  /// The invalid number error.
  InvalidNumber(Span, u8),
}

impl<'a> Diagnostic<'a> for Lexical {
  fn report(&self) -> Report<'a> {
    match self {
      Self::Unknown(span, byte) => todo!("invalid num — {span}-{byte}"),
      Self::InvalidNumber(span, byte) => todo!("invalid num — {span}-{byte}"),
    }
  }
}

/// The unknown character error.
#[inline]
pub const fn unknown(span: Span, byte: u8) -> Error {
  Error::Lexical(Lexical::Unknown(span, byte))
}

/// The invalid number error.
#[inline]
pub const fn invalid_number(span: Span, byte: u8) -> Error {
  Error::Lexical(Lexical::InvalidNumber(span, byte))
}
