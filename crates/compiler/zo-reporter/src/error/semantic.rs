use super::Diagnostic;

use crate::report::Report;

use swisskit::span::Span;

pub enum Semantic {}

impl<'a> Diagnostic<'a> for Semantic {
  fn report(&self) -> Report<'a> {
    todo!()
  }
}
