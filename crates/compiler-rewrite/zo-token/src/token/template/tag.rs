/// The representation of a tag token.
#[derive(Clone, Debug, PartialEq)]
pub struct Tag {
  /// A tag kind — see also [`TagKind`].
  pub kind: TagKind,
  /// A tag name — see also [`Name`].
  pub name: Name,
  /// A self closing tag flag.
  pub self_closing: bool,
  /// A fragment tag flag.
  pub frag: bool,
  /// A list of attributes — see also [`Attr`].
  pub attrs: Vec<Attr>,
}

/// The representation of tag kind.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TagKind {
  /// An opening tag.
  Opening,
  /// A closing tag.
  Closing,
}

/// The representation of a tag name.
///
/// A name must follow the kebab-case naming convention.
#[derive(Clone, Debug, PartialEq)]
pub enum Name {
  /// A custom name.
  Custom(Custom),
}

/// The representation of a custom tag name.
///
/// A name must follow the kebab-case naming convention.
#[derive(Clone, Debug, PartialEq)]
pub enum Custom {
  /// A fragment tag name.
  Fragment,
  /// A custom tag name.
  Name(String),
}

/// The representation of an attribute.
#[derive(Clone, Debug, PartialEq)]
pub enum Attr {
  /// A static attribute — `foo="bar"`.
  Static(String, Option<String>),
  /// A dynamic attribute — `foo={bar}`, `{bar}`.
  Dynamic(String, Option<String>),
}
