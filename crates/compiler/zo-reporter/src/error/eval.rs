use super::Diagnostic;

use crate::report::Report;

use swisskit::span::Span;

/// The representation of evaluation errors.
#[derive(Debug)]
pub enum Eval {}

impl<'a> Diagnostic<'a> for Eval {
  fn report(&self) -> Report<'a> {
    todo!()
  }
}