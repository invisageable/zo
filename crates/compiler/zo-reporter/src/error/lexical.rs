use super::{Diagnostic, Error};

use crate::color;
use crate::report::{Report, ReportKind};

use ariadne::Fmt;
use swisskit::span::Span;

/// The representation of lexical analysis errors.
#[derive(Debug)]
pub enum Lexical {
  /// An unknown character error.
  Unknown(Span, u8),
  /// An invalid number error.
  InvalidNumber(Span, u8),
}

impl<'a> Diagnostic<'a> for Lexical {
  #[inline]
  fn report(&self) -> Report<'a> {
    match self {
      Self::Unknown(span, byte) => Report {
        kind: ReportKind::ERROR,
        message: format!("{}", "invalid character.".fg(color::title())).into(),
        labels: vec![(
          *span,
          format!(
            "{}: `{}`",
            "this character does not ring a bell.".fg(color::error()),
            *byte as char
          )
          .into(),
          color::error(),
        )],
        notes: vec![format!(
          "{}",
          "🤖 bruh! what language are you speaking to me in? speeks zo."
            .fg(color::note())
        )
        .into()],
        helps: vec![format!(
          "{}",
          "👉 please go read the doc: <doc-link>".fg(color::help())
        )
        .into()],
      },
      Self::InvalidNumber(span, byte) => Report {
        kind: ReportKind::ERROR,
        message: format!("{}", "leading zero.".fg(color::title())).into(),
        labels: vec![(
          *span,
          format!(
            "{}: `{}`",
            "this digit is leading by zero.".fg(color::error()),
            *byte as char
          )
          .into(),
          color::error(),
        )],
        ..Default::default()
      },
    }
  }
}

/// The unknown character error.
#[inline(always)]
pub const fn unknown(span: Span, byte: u8) -> Error {
  Error::Lexical(Lexical::Unknown(span, byte))
}

/// The invalid number error.
#[inline(always)]
pub const fn invalid_number(span: Span, byte: u8) -> Error {
  Error::Lexical(Lexical::InvalidNumber(span, byte))
}
