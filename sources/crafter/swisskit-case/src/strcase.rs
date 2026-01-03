//! Functions built on top of `inflector` for naming convention transformation
//! and validation such as `camelcase`, `capitalcase`, `lowercase`, `snakecase`.
//!
//! note — for more support go to:
//!
//! - [`crate::case::bitcase`].

pub mod camelcase;
pub mod kebabcase;
pub mod pascalcase;
pub mod pluralcase;
pub mod singularcase;
pub mod snakecase;
pub mod traincase;

/// The representation of a string case.
pub enum StrCase {
  /// The camel case.
  Camel,
  /// The kebab case.
  Kebab,
  /// The pascal case.
  Pascal,
  /// The snake case.
  Snake,
  /// The snake screaming case.
  SnakeScreaming,
}

impl std::fmt::Display for StrCase {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Camel => write!(f, "camel case"),
      Self::Kebab => write!(f, "kebab case"),
      Self::Pascal => write!(f, "pascal case"),
      Self::Snake => write!(f, "snake case"),
      Self::SnakeScreaming => write!(f, "snake screaming case"),
    }
  }
}
