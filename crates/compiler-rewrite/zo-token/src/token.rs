pub mod program;
pub mod style;
pub mod template;

pub use program::Program;
pub use style::Style;
pub use template::Template;

use swisskit::span::Span;

/// The representation of a token.
#[derive(Clone, Debug)]
pub struct Token {
  /// A token kind.
  pub kind: TokenKind,
  /// A span location within a source file.
  pub span: Span,
}

impl Token {
  /// An end of file token.
  pub const EOF: Self = Token::new(TokenKind::Eof, Span::ZERO);

  /// Creates a new token.
  pub const fn new(kind: TokenKind, span: Span) -> Self {
    Self { kind, span }
  }

  /// Creates a new token.
  pub fn eof(span: Span) -> Self {
    Self {
      kind: TokenKind::Eof,
      span,
    }
  }

  /// Checks if the kind of a token matched from a other token kind.
  pub fn is(&self, kind: TokenKind) -> bool {
    self.kind.is(kind)
  }
}

impl std::fmt::Display for Token {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.kind)
  }
}

/// The representation of a token kind.
#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
  /// A end of file token kind.
  Eof,
  /// A program token kind.
  Program(program::Program),
  /// A style token kind.
  Style(style::Style),
  /// A template token kind.
  Template(template::Template),
}

impl TokenKind {
  /// Checks the equality of a token kind.
  ///
  /// #### returns.
  ///
  /// The resulting returns `true` if the token kind is equal to the right-hand
  /// side.
  pub fn is(&self, kind: TokenKind) -> bool {
    *self == kind
  }

  /// Checks the equality of a token kind as a program kind.
  pub fn is_program(&self) -> bool {
    matches!(self, Self::Program(_))
  }

  /// Checks the equality of a token kind as a style kind.
  pub fn is_style(&self) -> bool {
    matches!(self, Self::Style(_))
  }

  /// Checks the equality of a token kind as a template kind.
  pub fn is_template(&self) -> bool {
    matches!(self, Self::Template(_))
  }
}

impl std::fmt::Display for TokenKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Eof => write!(f, "eof"),
      Self::Program(program) => {
        write!(f, "{program}")
      }
      Self::Style(style) => write!(f, "{style}"),
      Self::Template(template) => write!(f, "{template}"),
    }
  }
}
