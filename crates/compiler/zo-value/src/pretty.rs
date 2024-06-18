//! ...

use super::value::{Value, ValueKind};

use zo_core::fmt::sep_comma;

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
      Self::Char(ch) => write!(f, "'{ch}'"),
      Self::Str(string) => write!(f, "\"{string}\""),
      Self::Fn(prototype, block) => {
        write!(f, "fn {prototype} ")?;

        if block.len() == 1 {
          return write!(f, "-> {}", block[0]);
        }

        write!(f, "{block}")
      }
      Self::Return(value) => write!(f, "return {value};"),
      Self::Builtin(..) => write!(f, "NIY"),
      Self::Array(array) => write!(f, "[{}]", sep_comma(array)),
      Self::Record(record) => write!(f, "{{ {:?} }}", record),
    }
  }
}
