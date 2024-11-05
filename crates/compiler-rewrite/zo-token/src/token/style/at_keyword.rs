use super::Style;

use crate::token::TokenKind;

/// The representation of at keyword.
#[derive(Clone, Debug, PartialEq)]
pub enum AtKeyword {
  /// A `@charset` keyword.
  Charset,
  /// A `@color-profile` keyword.
  ColorProfile,
  /// A `@container` keyword.
  Container,
  /// A `@counter-style` keyword.
  CounterStyle,
  /// A `@font-face` keyword.
  FontFace,
  /// A `@font-feature-values` keyword.
  FontFeatureValues,
  /// A `@font-palette-values` keyword.
  FontPaletteValues,
  /// A `@import` keyword.
  Import,
  /// A `@keyframes` keyword.
  Keyframes,
  /// A `@layer` keyword.
  Layer,
  /// A `@media` keyword.
  Media,
  /// A `@namespace` keyword.
  Namespace,
  /// A `@page` keyword.
  Page,
  /// A `@position-try` keyword.
  PositionTry,
  /// A `@property` keyword.
  Property,
  /// A `@scope` keyword.
  Scope,
  /// A `@starting-style` keyword.
  StartingStyle,
  /// A `@supports` keyword.
  Supports,
  /// A `@view-transition` keyword.
  ViewTransition,
}

impl From<&str> for AtKeyword {
  fn from(at_kw: &str) -> Self {
    match at_kw {
      "@charset" => Self::Charset,
      "@color-profile" => Self::ColorProfile,
      "@container" => Self::Container,
      "@counter-style" => Self::CounterStyle,
      "@font-face" => Self::FontFace,
      "@font-feature-values" => Self::FontFeatureValues,
      "@font-palette-values" => Self::FontPaletteValues,
      "@import" => Self::Import,
      "@keyframes" => Self::Keyframes,
      "@layer" => Self::Layer,
      "@media" => Self::Media,
      "@namespace" => Self::Namespace,
      "@page" => Self::Page,
      "@position-try" => Self::PositionTry,
      "@property" => Self::Property,
      "@scope" => Self::Scope,
      "@starting-style" => Self::StartingStyle,
      "@supports" => Self::Supports,
      "@view-transition" => Self::ViewTransition,
      _ => unreachable!("{at_kw}"),
    }
  }
}

impl std::fmt::Display for AtKeyword {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Charset => write!(f, "@charset"),
      Self::ColorProfile => write!(f, "@color-profile"),
      Self::Container => write!(f, "@container"),
      Self::CounterStyle => write!(f, "@counter-style"),
      Self::FontFace => write!(f, "@font-face"),
      Self::FontFeatureValues => write!(f, "@font-feature-values"),
      Self::FontPaletteValues => write!(f, "@font-palette-values"),
      Self::Import => write!(f, "@import"),
      Self::Keyframes => write!(f, "@keyframes"),
      Self::Layer => write!(f, "@layer"),
      Self::Media => write!(f, "@media"),
      Self::Namespace => write!(f, "@namespace"),
      Self::Page => write!(f, "@page"),
      Self::PositionTry => write!(f, "@position-try"),
      Self::Property => write!(f, "@property"),
      Self::Scope => write!(f, "@scope"),
      Self::StartingStyle => write!(f, "@starting-style"),
      Self::Supports => write!(f, "@supports"),
      Self::ViewTransition => write!(f, "@view-transition"),
    }
  }
}

/// The At keywords dictionnary.
pub fn keywords() -> std::collections::HashMap<&'static str, TokenKind> {
  std::collections::HashMap::from([
    (
      "@charset",
      TokenKind::Style(Style::AtKeyword(AtKeyword::Charset)),
    ),
    (
      "@color-profile",
      TokenKind::Style(Style::AtKeyword(AtKeyword::ColorProfile)),
    ),
    (
      "@container",
      TokenKind::Style(Style::AtKeyword(AtKeyword::Container)),
    ),
    (
      "@counter-style",
      TokenKind::Style(Style::AtKeyword(AtKeyword::CounterStyle)),
    ),
    (
      "@font-face",
      TokenKind::Style(Style::AtKeyword(AtKeyword::FontFace)),
    ),
    (
      "@font-feature-values",
      TokenKind::Style(Style::AtKeyword(AtKeyword::FontFeatureValues)),
    ),
    (
      "@font-palette-values",
      TokenKind::Style(Style::AtKeyword(AtKeyword::FontPaletteValues)),
    ),
    (
      "@import",
      TokenKind::Style(Style::AtKeyword(AtKeyword::Import)),
    ),
    (
      "@keyframes",
      TokenKind::Style(Style::AtKeyword(AtKeyword::Keyframes)),
    ),
    (
      "@layer",
      TokenKind::Style(Style::AtKeyword(AtKeyword::Layer)),
    ),
    (
      "@media",
      TokenKind::Style(Style::AtKeyword(AtKeyword::Media)),
    ),
    (
      "@namespace",
      TokenKind::Style(Style::AtKeyword(AtKeyword::Namespace)),
    ),
    ("@page", TokenKind::Style(Style::AtKeyword(AtKeyword::Page))),
    (
      "@position-try",
      TokenKind::Style(Style::AtKeyword(AtKeyword::PositionTry)),
    ),
    (
      "@property",
      TokenKind::Style(Style::AtKeyword(AtKeyword::Property)),
    ),
    (
      "@scope",
      TokenKind::Style(Style::AtKeyword(AtKeyword::Scope)),
    ),
    (
      "@starting-style",
      TokenKind::Style(Style::AtKeyword(AtKeyword::StartingStyle)),
    ),
    (
      "@supports",
      TokenKind::Style(Style::AtKeyword(AtKeyword::Supports)),
    ),
    (
      "@view-transition",
      TokenKind::Style(Style::AtKeyword(AtKeyword::ViewTransition)),
    ),
  ])
}
