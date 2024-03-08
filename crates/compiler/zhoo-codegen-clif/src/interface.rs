#![allow(dead_code)]

use zhoo_ty::ty::{Ty, TyKind};

use cranelift_codegen::ir::{types, Type};

pub(crate) trait Clif {
  fn as_clif(&self) -> Type;
}

impl Clif for Ty {
  fn as_clif(&self) -> Type {
    self.kind.as_clif()
  }
}

impl Clif for TyKind {
  fn as_clif(&self) -> Type {
    match self {
      Self::Int => types::I64,
      Self::Float => types::F64,
      Self::Bool => types::I8,
      Self::Char => types::I8,
      Self::Str => types::I8,
      Self::Unit => types::I8,
      _ => unreachable!(),
    }
  }
}
