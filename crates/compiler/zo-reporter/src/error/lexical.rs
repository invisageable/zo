use super::{Diagnostic, Error};

use crate::report::{Report, ReportKind};

use swisskit::span::Span;

/// The representation of lexical analysis errors.
#[derive(Debug)]
pub enum Lexical {
  Unknown(Span, u8),
}

impl<'a> Diagnostic<'a> for Lexical {
  fn report(&self) -> Report<'a> {
    todo!()
  }
}
