use zo_interner::interner::symbol::Symbol;

use swisskit::fmt::sep_space;

use thin_vec::ThinVec;

/// The representation of zo syntax extension (zsx).
#[derive(Clone, Debug, PartialEq)]
pub struct Tag {
  /// A tag kind — see also [`TagKind`].
  pub kind: TagKind,
  /// A name — see also [`Name`].
  pub name: Name,
  /// A self closing tag flag.
  pub self_closing: bool,
  /// A list of attributes — see also [`Attr`].
  pub attrs: ThinVec<Attr>,
}

impl std::fmt::Display for Tag {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let Self {
      kind,
      name,
      self_closing,
      attrs,
    } = self;

    match kind {
      TagKind::Opening => {
        if *self_closing {
          write!(f, "<{name} {attrs} />", attrs = sep_space(attrs))
        } else {
          write!(f, "<{name} {attrs}>", attrs = sep_space(attrs))
        }
      }
      TagKind::Closing => write!(f, "</tag-closing>"),
    }
  }
}

/// The representation of zo syntax extension (zsx).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TagKind {
  /// An opening tag.
  Opening,
  /// A closing tag.
  Closing,
}

impl std::fmt::Display for TagKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Opening => write!(f, "<tag-opening>"),
      Self::Closing => write!(f, "</tag-closing>"),
    }
  }
}

/// The representation of an name.
///
/// A name must follow the kebab-case naming convention.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Name {
  /// A html name.
  Html(Html),
  /// A custom name.
  Custom(Symbol),
}

impl std::fmt::Display for Name {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Html(html) => write!(f, "{html}"),
      Self::Custom(sym) => write!(f, "{sym}"),
    }
  }
}

/// The representation of an attribute.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Attr {
  /// A static attribute — `foo="bar"`.
  Static(Symbol, Option<Symbol>),
  /// A dynamic attribute — `foo={bar}`, `{bar}`.
  Dynamic(Symbol, Option<Symbol>),
}

impl std::fmt::Display for Attr {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Static(sym, maybe_value) => {
        if let Some(value) = maybe_value {
          write!(f, "{sym}=\"{value}\"")
        } else {
          write!(f, "{sym}")
        }
      }
      Self::Dynamic(sym, maybe_value) => {
        if let Some(value) = maybe_value {
          write!(f, "{sym}={{{value}}}")
        } else {
          write!(f, "{{{sym}}}")
        }
      }
    }
  }
}

/// The representation of html tag name.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Html {
  /// An anchor tag name — `<a>`.
  A,
  /// An div tag name — `<div>`.
  Div,
}

impl std::fmt::Display for Html {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::A => write!(f, "a"),
      Self::Div => write!(f, "div"),
    }
  }
}
