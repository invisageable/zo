pub mod camelcase;
pub mod kebabcase;
pub mod pascalcase;
pub mod pluralcase;
pub mod singularcase;
pub mod snakecase;
pub mod traincase;

pub enum StrCase {
  Camel,
  Kebab,
  Pascal,
  Snake,
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
