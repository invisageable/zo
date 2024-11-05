/// The representation of a tag token.
#[derive(Clone, Debug, PartialEq)]
pub struct Tag {
  /// A tag kind — see also [`TagKind`].
  pub kind: TagKind,
  /// A tag name — see also [`Name`].
  pub name: String,
  /// A self closing tag flag.
  pub self_closing: bool,
  /// A fragment tag flag.
  pub frag: bool,
  /// A list of attributes — see also [`Attr`].
  pub attrs: Vec<Attr>,
}

impl Tag {
  /// Creates a new tag.
  pub fn new(kind: TagKind) -> Self {
    Self {
      name: String::with_capacity(0usize),
      self_closing: false,
      frag: false,
      attrs: Vec::with_capacity(0usize),
      kind,
    }
  }
}

/// The representation of tag kind.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TagKind {
  /// An opening tag.
  Opening,
  /// A closing tag.
  Closing,
}

/// The representation of a custom tag name.
///
/// #### note.
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
pub struct Attr {
  pub kind: AttrKind,
  pub name: String,
  pub value: String,
}

impl Attr {
  /// Creates a new atribute.
  pub fn new() -> Self {
    Self {
      kind: AttrKind::Static,
      name: String::with_capacity(0usize),
      value: String::with_capacity(0usize),
    }
  }

  /// Clear the attribute.
  pub fn clear(&mut self) {
    self.name.truncate(0usize);
    self.value.truncate(0usize);
  }
}

/// The representation of an attribute kind.
#[derive(Clone, Debug, PartialEq)]
pub enum AttrKind {
  /// A static attribute — `foo=bar`, `foo="bar"`, `foo='bar'`.
  Static,
  /// A dynamic attribute — `foo={bar}`, `{bar}`.
  Dynamic,
}
