use super::Diagnostic;

use crate::report::Report;

use swisskit::span::Span;

/// The representation of syntax analysis errors.
#[derive(Debug)]
pub enum Syntax {}

impl<'a> Diagnostic<'a> for Syntax {
  fn report(&self) -> Report<'a> {
    todo!()
  }
}
