//! #### types.
//!
//! **Type unit.**
//!
//! - `()`
//!
//! **Types for integers.**
//!
//! - `int`
//! - `s8`, `s16`, `s32`, `s64`, `s128`
//! - `u8`, `u16`, `u32`, `u64`, `u128`
//!
//! **Types for floating-point.**
//!
//! - `float`
//! - `f32`, `f64`
//!
//! **Type for booleans.**
//!
//! - `bool`
//!
//! **Type for strings.**
//!
//! - `str`
//!
//! **Type for arrays.**
//!
//! - `int[]`
//! - `int[2]`

use smol_str::SmolStr;
use swisskit::span::Span;

use hashbrown::HashSet;

/// The representation of a type.
#[derive(Clone, Debug, PartialEq)]
pub struct Ty {
  /// The kind of a type — see also [`TyKind`].
  pub kind: TyKind,
  /// The span of a type — see also [`Span`].
  pub span: Span,
}

impl Ty {
  /// The unit type, it is used as a placeholder.
  pub const UNIT: Self = Self::new(TyKind::Unit, Span::ZERO);

  /// Creates a new type.
  #[inline]
  pub const fn new(kind: TyKind, span: Span) -> Self {
    Self { kind, span }
  }

  /// Creates a new unit type.
  #[inline]
  pub const fn unit(span: Span) -> Self {
    Self::new(TyKind::Unit, span)
  }

  /// Creates a new infered type.
  #[inline]
  pub const fn infer(span: Span) -> Self {
    Self::new(TyKind::Infer, span)
  }

  /// Creates a new integer type.
  #[inline]
  pub const fn int(int: LitIntTy, span: Span) -> Self {
    Self::new(TyKind::Int(int), span)
  }

  /// Creates a new float type.
  #[inline]
  pub const fn float(float: LitFloatTy, span: Span) -> Self {
    Self::new(TyKind::Float(float), span)
  }

  /// Creates a new array type.
  #[inline]
  pub fn array(ty: Ty, maybe_size: Option<usize>, span: Span) -> Self {
    Self::new(TyKind::Array(Box::new(ty), maybe_size), span)
  }

  /// Creates a new tuple type.
  #[inline]
  pub fn tuple(tys: Vec<Ty>, span: Span) -> Self {
    Self::new(TyKind::Tuple(tys), span)
  }

  /// Retrieves the set of quantified type variables.
  #[inline]
  pub fn ty_vars(&self) -> HashSet<usize> {
    self.kind.ty_vars()
  }
}

/// The representation of different kind of type.
#[derive(Clone, Debug, PartialEq)]
pub enum TyKind {
  /// unit — `()`.
  Unit,
  /// infer — `:=`.
  Infer,
  /// integer — `int`, `s32`, `u32`.
  Int(LitIntTy),
  /// float — `float`, `f32`, `f64`.
  Float(LitFloatTy),
  /// array — `int[]`, `str[3]`.
  Array(Box<Ty>, Option<usize>),
  /// tuple — `(int, float)`.
  Tuple(Vec<Ty>),
  /// constructed type.
  Con(SmolStr, Vec<Ty>),
}

impl TyKind {
  /// Checks if a type is a specific kind.
  #[inline]
  pub fn is(&self, kind: TyKind) -> bool {
    *self == kind
  }

  /// Checks if a type is a numeric kind.
  #[inline]
  pub const fn is_numeric(&self) -> bool {
    matches!(self, Self::Int(..) | Self::Float(..))
  }

  /// Retrieves the set of quantified type variables.
  #[inline]
  fn ty_vars(&self) -> HashSet<usize> {
    match self {
      Self::Con(_, tys) => {
        let mut ty_vars = HashSet::new();

        for ty in tys.iter() {
          ty_vars.extend(ty.ty_vars());
        }
        ty_vars
      }
      _ => panic!(),
    }
  }
}

/// The representation of an integer type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LitIntTy {
  Int(IntTy),
  Signed(SintTy),
  Unsigned(UintTy),
  Unsuffixed,
}

/// The representation of a float type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LitFloatTy {
  Suffixed(FloatTy),
  Unsuffixed,
}

/// The representation of different kind of an integer type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IntTy {
  /// `int` — by default is `s64`.
  Int,
}

/// The representation of different kind of a signed integer type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SintTy {
  /// 8-bits signed integer type.
  S8,
  /// 16-bits signed integer type.
  S16,
  /// 32-bits signed integer type.
  S32,
  /// 64-bits signed integer type.
  S64,
  /// 128-bits signed integer type.
  S128,
}

/// The representation of different kind of an unsigned integer type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UintTy {
  /// 8-bits unsigned integer type.
  U8,
  /// 16-bits unsigned integer type.
  U16,
  /// 32-bits unsigned integer type.
  U32,
  /// 64-bits unsigned integer type.
  U64,
  /// 128-bits unsigned integer type.
  U128,
}

/// The representation of different kind of a float type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FloatTy {
  /// `float` — by default is `f32`.
  Float,
  /// 32-bits floating-point type.
  F32,
  /// 64-bits floating-point type.
  F64,
}
