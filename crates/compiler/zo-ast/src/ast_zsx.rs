use super::ast::Expr;

use thin_vec::ThinVec;

#[derive(Clone, Debug)]
pub struct Tag {
  pub kind: TagKind,
  pub attrs: ThinVec<Attr>,
}

#[derive(Clone, Debug)]
pub enum TagKind {
  Name(Name),
  Fragment,
}

#[derive(Clone, Debug)]
pub enum Name {
  Html(Html),
  Custom(Box<Expr>),
}

#[derive(Clone, Debug)]
pub struct Attr {
  pub kind: AttrKind,
}

#[derive(Clone, Debug)]
pub enum AttrKind {
  /// A static attribute — `foo="bar"`, `bar`.
  Static(Box<Expr>, Option<Expr>),
  /// A static attribute — `foo={bar}`, `{bar}`.
  Dynamic(Box<Expr>, Option<Expr>),
}

#[derive(Clone, Debug)]
pub enum Html {
  A,
  Div,
}

impl From<&str> for Html {
  fn from(name: &str) -> Self {
    match name {
      "a" => Self::A,
      "div" => Self::Div,
      _ => panic!(),
    }
  }
}
