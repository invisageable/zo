use super::value::{Value, ValueKind};

use swisskit::fmt::sep_comma;

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
      Self::Array(array) => write!(f, "[{}]", sep_comma(array)),
      Self::Tuple(tuple) => write!(f, "({})", sep_comma(tuple)),
      Self::Loop(body) => write!(f, "loop {body}"),
      Self::While(condition, body) => write!(f, "while {condition} {body}"),
      Self::Return(value) => write!(f, "return {value};"),
      Self::Break(value) => write!(f, "break {value};"),
      Self::Continue => write!(f, "continue;"),
      Self::Closure(prototype, body) => {
        if body.len() == 1 {
          return write!(f, "fn {prototype} -> {}", body[0]);
        }

        write!(f, "fn {prototype} {body}")
      }
      Self::Builtin(builtin) => write!(f, "{builtin:?}"),
    }
  }
}
