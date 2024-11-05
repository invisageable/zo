mod tag;

pub use tag::{Attr, AttrKind, Tag, TagKind};

/// The representation of a template token kind.
#[derive(Clone, Debug, PartialEq)]
pub enum Template {
  /// An expression token kind.
  Expr(String),
  /// A character token kind.
  Character(char),
  /// A tag token kind.
  Tag(Tag),
}

impl std::fmt::Display for Template {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Expr(expr) => write!(f, "{expr}"),
      Self::Character(ch) => write!(f, "{ch}"),
      Self::Tag(_tag) => write!(f, "template tag"),
    }
  }
}
