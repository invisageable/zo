//! ...

use super::value::{StructExprKey, Value, ValueKind, Var};

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
      Self::Ident(symbol) => write!(f, "{symbol}"),
      Self::Bool(boolean) => write!(f, "{boolean}"),
      Self::Char(ch) => write!(f, "'{ch}'"),
      Self::Str(string) => write!(f, "\"{string}\""),
      Self::Fn(prototype, block) => {
        write!(
          f,
          "fn {} ({}): {} ",
          prototype.pattern,
          sep_comma(&prototype.inputs),
          prototype.output_ty
        )?;

        if block.len() == 1 {
          return write!(f, "-> {}", block[0]);
        }

        write!(f, "{block}")
      }
      Self::Return(value) => write!(f, "return {value};"),
      Self::Builtin(..) => write!(f, "NIY"),
      Self::Array(array) => write!(f, "[{}]", sep_comma(array)),
      Self::Var(var) => write!(f, "{var}"),
      Self::Struct(ident, fields) => write!(f, "struct {ident} {fields}"),
      Self::StructExpr(pairs) => {
        let mut pairs = pairs
          .iter()
          .map(|(key, value)| format!("{key} = {value}"))
          .collect::<Vec<String>>();

        pairs.sort();

        write!(f, "{{ {} }}", sep_comma(&pairs))
      }
      Self::Fun(prototype, block) => write!(f, "fun {prototype} {block}"),
      Self::While(condition, body) => write!(f, "while {condition} {body}"),
    }
  }
}

impl std::fmt::Display for StructExprKey {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Ident(ident) => write!(f, "{ident}"),
    }
  }
}

impl std::fmt::Display for Var {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    let kind = &self.kind;
    let pattern = &self.pattern;
    let value = &self.value;

    write!(f, "{kind} {pattern} = {value}")
  }
}
