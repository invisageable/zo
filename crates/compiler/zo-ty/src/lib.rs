mod annotation;
mod ty;

pub use annotation::Annotation;

pub use ty::{
  ArrayTy, ArrayTyId, FloatWidth, FunTy, FunTyId, InferVarId, IntWidth,
  Mutability, RefTy, RefTyId, TupleTy, TupleTyId, Ty, TyId, TyTable,
};
