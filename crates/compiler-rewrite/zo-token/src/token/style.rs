mod at_keyword;
mod comment;
mod delim;
mod dimension;
mod group;
mod punctuation;

pub use at_keyword::{keywords, AtKeyword};
pub use comment::Comment;
pub use delim::Delim;
pub use dimension::Dimension;
pub use group::Group;
pub use punctuation::Punctuation;

use zor_interner::symbol::Symbol;

/// The representation of a style token kind.
///
/// @see — https://www.w3.org/TR/css-syntax-3/#typedef-delim-token.
#[derive(Clone, Debug, PartialEq)]
pub enum Style {
  /// A comment token kind.
  Comment(Comment),
  /// An identifier token kind.
  Ident(Symbol),
  /// A function token kind.
  Function(Symbol),
  /// A at keyword token kind.
  AtKeyword(AtKeyword),
  /// A hash token kind.
  Hash(String),
  /// A string token kind.
  String(String),
  /// A url token kind.
  Url(String),
  /// A delimiter token kind.
  Delim(Delim),
  /// A number token kind.
  Number(String),
  /// A percentage token kind.
  Percentage(String),
  /// A dimension token kind.
  Dimension(String, Dimension),
  /// A whitespace token kind.
  Whitespace(String),
  /// A punctuation token kind.
  Punctuation(Punctuation),
  /// A group token kind.
  Group(Group),
}

impl std::fmt::Display for Style {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Comment(comment) => write!(f, "{comment}"),
      Self::Ident(_sym) => write!(f, "style ident"),
      Self::Function(_sym) => write!(f, "style function"),
      Self::AtKeyword(at_kw) => write!(f, "{at_kw}"),
      Self::String(string) => write!(f, "#{string}"),
      Self::Url(url) => write!(f, "#{url}"),
      Self::Hash(hash) => write!(f, "#{hash}"),
      Self::Delim(delim) => write!(f, "{delim}"),
      Self::Number(num) => write!(f, "{num}"),
      Self::Percentage(num) => write!(f, "{num}%"),
      Self::Dimension(num, dim) => write!(f, "{num}{dim}"),
      Self::Whitespace(ws) => write!(f, "{ws}"),
      Self::Punctuation(punctuation) => write!(f, "{punctuation}"),
      Self::Group(group) => write!(f, "{group}"),
    }
  }
}
