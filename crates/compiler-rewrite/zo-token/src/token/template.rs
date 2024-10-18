mod tag;

pub use tag::{Attr, Name, Tag, TagKind};

/// The representation of a template token kind.
#[derive(Clone, Debug, PartialEq)]
pub enum Template {
  /// A character token kind.
  Character(char),
  /// A tag token kind.
  Tag(Tag),
}

impl std::fmt::Display for Template {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Character(ch) => write!(f, "{ch}"),
      Self::Tag(_tag) => write!(f, "template tag"),
    }
  }
}
