use super::value::{Value, ValueKind};

use swisskit::fmt;

impl std::fmt::Display for Value {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", self.kind)
  }
}

impl std::fmt::Display for ValueKind {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Unit => write!(f, "()"),
      Self::Int(int) => write!(f, "{int}"),
      Self::Float(float) => write!(f, "{float}"),
      Self::Bool(boolean) => write!(f, "{boolean}"),
      Self::Array(array) => write!(f, "[{}]", fmt::sep_comma(array)),
    }
  }
}
