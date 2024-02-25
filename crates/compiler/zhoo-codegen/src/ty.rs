use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Ty {
  pub kind: TyKind,
}

impl Ty {
  pub const UNIT: Self = Self::of(TyKind::Int);

  pub const fn of(kind: TyKind) -> Self {
    Self { kind }
  }

  pub const fn fun() -> Self {
    Self::of(TyKind::Fun)
  }

  pub const fn int() -> Self {
    Self::of(TyKind::Int)
  }

  pub const fn float() -> Self {
    Self::of(TyKind::Float)
  }

  pub const fn ident(ident: String) -> Self {
    Self::of(TyKind::Ident(ident))
  }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TyKind {
  Unit,
  Int,
  Float,
  Ident(String),
  Fun,
}
