use super::{Error, Report, ReportKind, REPORT_TITLE_ERROR};

use crate::color;
use crate::span::Span;

use ariadne::Fmt;

#[derive(Debug)]
pub enum Lexical {
  Unknown(Span, char),
  InvalidNum(Span, char),
  ReservedKeyword(Span, String),
}

impl Error for Lexical {
  fn report(&self) -> Report {
    match self {
      Self::Unknown(span, ch) => Report {
        kind: ReportKind::Error(REPORT_TITLE_ERROR),
        message: format!("{}", "unknown character.".fg(color::title())).into(),
        labels: vec![(
          *span,
          format!(
            "{}: `{ch}`",
            "this character does not ring a bell".fg(color::error())
          )
          .into(),
          color::error(),
        )],
        helps: vec![format!(
          "{}",
          "👉 please go read the doc: <doc-link>".fg(color::help())
        )
        .into()],
        notes: vec![format!(
          "{}",
          "🤖 what language are you trying to speak to me in? i only speak zhoo."
            .fg(color::note())
        )
        .into()],
      },
      Self::InvalidNum(span, ch) => todo!("invalid num — {span}-{ch}"),
      Self::ReservedKeyword(span, word) => todo!("reserved keyword — {span}-{word}"),
    }
  }
}
