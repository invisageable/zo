use zo_core::span::Span;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub enum Comment {
  Line(Span),
  LineDoc(Span),
}

impl std::fmt::Display for Comment {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Line(_) => write!(f, "comment line"),
      Self::LineDoc(_) => write!(f, "comment line doc"),
    }
  }
}
