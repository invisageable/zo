mod comment;
mod group;
mod int;
mod kw;
mod punctuation;

pub use comment::Comment;
pub use group::Group;
pub use int::Base;
pub use kw::{keywords, Kw};
pub use punctuation::Punctuation;

use zor_interner::symbol::Symbol;

/// The representation of a program token kind.
#[derive(Clone, Debug, PartialEq)]
pub enum Program {
  /// A comment token kind.
  Comment(Comment),
  /// An integer token kind.
  Int(String, Base),
  /// A float token kind.
  Float(String),
  /// A punctuation token kind.
  Punctuation(Punctuation),
  /// A group token kind.
  Group(Group),
  /// An identifier token kind.
  Ident(Symbol),
  /// A keyword token kind.
  Kw(Kw),
}

impl std::fmt::Display for Program {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Comment(comment) => write!(f, "{comment}"),
      Self::Int(int, base) => {
        write!(f, "{int}({base})")
      }
      Self::Float(sym) => write!(f, "{sym}"),
      Self::Punctuation(punctuation) => {
        write!(f, "{punctuation}")
      }
      Self::Group(group) => write!(f, "{group}"),
      Self::Ident(_sym) => write!(f, "program ident"),
      Self::Kw(kw) => write!(f, "{kw}"),
    }
  }
}
