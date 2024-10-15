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
  /// An eof before tag name error.
  EofBeforeTagname(Span, u8),
  /// An eof in tag error.
  EofInTag(Span, u8),
  /// An invalid number error.
  InvalidNumber(Span, u8),
  /// An unexpected null character error.
  UnexpectedQuestionMark(Span, u8),
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
      Self::EofBeforeTagname(_span, _char) => Report {
        kind: ReportKind::ERROR,
        message: format!(
          "{}",
          "End of line before tag name.".fg(color::title())
        )
        .into(),
        ..Default::default()
      },
      Self::EofInTag(_span, _char) => Report {
        kind: ReportKind::ERROR,
        message: format!("{}", "End of line in tag.".fg(color::title())).into(),
        ..Default::default()
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
      Self::UnexpectedQuestionMark(span, char) => Report {
        kind: ReportKind::ERROR,
        message: format!(
          "{}",
          "Unexpected Question mark instead of tag name.".fg(color::title())
        )
        .into(),
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

/// The EOF before tag name error.
#[inline(always)]
pub const fn eof_before_tag_name(span: Span, byte: u8) -> Error {
  Error::Lexical(Lexical::EofBeforeTagname(span, byte))
}

/// The EOF in tag error.
#[inline(always)]
pub const fn eof_in_tag(span: Span, byte: u8) -> Error {
  Error::Lexical(Lexical::EofInTag(span, byte))
}

/// The invalid number error.
#[inline(always)]
pub const fn invalid_number(span: Span, byte: u8) -> Error {
  Error::Lexical(Lexical::InvalidNumber(span, byte))
}

/// The unexpected null character error.
#[inline(always)]
pub const fn unexpected_question_mark(span: Span, byte: u8) -> Error {
  Error::Lexical(Lexical::UnexpectedQuestionMark(span, byte))
}
