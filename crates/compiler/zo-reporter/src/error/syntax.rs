use super::Diagnostic;

use crate::report::Report;

use swisskit::span::Span;

pub enum Syntax {}

impl<'a> Diagnostic<'a> for Syntax {
  fn report(&self) -> Report<'a> {
    todo!()
  }
}
