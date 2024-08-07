use super::ty::{
  FloatTy, IntTy, LitFloatTy, LitIntTy, SintTy, Ty, TyKind, UintTy,
};

use swisskit::fmt::sep_comma;

impl std::fmt::Display for Ty {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", self.kind)
  }
}

impl std::fmt::Display for TyKind {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Unit => write!(f, "()"),
      Self::Infer => write!(f, "infer"),
      Self::Int(int) => write!(f, "{int}"),
      Self::Float(float) => write!(f, "{float}"),
      Self::Array(ty, maybe_size) => match maybe_size {
        Some(size) => write!(f, "{ty}[{size}]"),
        None => write!(f, "{ty}[]"),
      },
      Self::Tuple(tys) => write!(f, "{}", sep_comma(tys)),
      Self::Con(ident, tys) => write!(f, "{ident} - [{}]", sep_comma(tys)),
    }
  }
}

impl std::fmt::Display for LitIntTy {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Int(int) => write!(f, "{int}"),
      Self::Signed(int) => write!(f, "{int}"),
      Self::Unsigned(int) => write!(f, "{int}"),
      Self::Unsuffixed => write!(f, ""),
    }
  }
}

impl std::fmt::Display for LitFloatTy {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Suffixed(float) => write!(f, "{float}"),
      Self::Unsuffixed => write!(f, ""),
    }
  }
}

impl std::fmt::Display for IntTy {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Int => write!(f, "int"),
    }
  }
}

impl std::fmt::Display for SintTy {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::S8 => write!(f, "s8"),
      Self::S16 => write!(f, "s16"),
      Self::S32 => write!(f, "s32"),
      Self::S64 => write!(f, "s64"),
      Self::S128 => write!(f, "s128"),
    }
  }
}

impl std::fmt::Display for UintTy {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::U8 => write!(f, "u8"),
      Self::U16 => write!(f, "u16"),
      Self::U32 => write!(f, "u32"),
      Self::U64 => write!(f, "u64"),
      Self::U128 => write!(f, "u128"),
    }
  }
}

impl std::fmt::Display for FloatTy {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Float => write!(f, "float"),
      Self::F32 => write!(f, "f32"),
      Self::F64 => write!(f, "f64"),
    }
  }
}
