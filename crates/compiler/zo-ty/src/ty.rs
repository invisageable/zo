//! ...

use zo_core::interner::symbol::Symbol;
use zo_core::span::Span;

use hashbrown::HashSet;

#[derive(Clone, Debug, PartialEq)]
pub struct Ty {
  pub kind: TyKind,
  pub span: Span,
}

impl Ty {
  pub const UNIT: Self = Self {
    kind: TyKind::Unit,
    span: Span::ZERO,
  };

  #[inline]
  pub const fn new(kind: TyKind, span: Span) -> Self {
    Self { kind, span }
  }

  #[inline]
  pub fn unit(span: Span) -> Self {
    Self::new(TyKind::Unit, span)
  }

  #[inline]
  pub fn infer(span: Span) -> Self {
    Self::new(TyKind::Infer, span)
  }

  #[inline]
  pub fn int(int: LitIntTy, span: Span) -> Self {
    Self::new(TyKind::Int(int), span)
  }

  #[inline]
  pub fn float(float: LitFloatTy, span: Span) -> Self {
    Self::new(TyKind::Float(float), span)
  }

  #[inline]
  pub fn bool(span: Span) -> Self {
    Self::new(TyKind::Bool, span)
  }

  #[inline]
  pub fn char(span: Span) -> Self {
    Self::new(TyKind::Char, span)
  }

  #[inline]
  pub fn str(span: Span) -> Self {
    Self::new(TyKind::Str, span)
  }

  #[inline]
  pub fn alias(alias: Symbol, span: Span) -> Self {
    Self::new(TyKind::Alias(alias), span)
  }

  #[inline]
  pub fn var(var: usize, span: Span) -> Self {
    Self::new(TyKind::Var(var), span)
  }

  #[inline]
  pub fn array(ty: Ty, span: Span) -> Self {
    Self::new(TyKind::Array(Box::new(ty)), span)
  }

  #[inline]
  pub fn closure(inputs: Vec<Ty>, output: Ty, span: Span) -> Self {
    Self::new(TyKind::Fn(inputs, Box::new(output)), span)
  }

  #[inline]
  pub fn is(&self, kind: TyKind) -> bool {
    self.kind.is(kind)
  }

  #[inline]
  pub const fn is_numeric(&self) -> bool {
    self.kind.is_numeric()
  }

  pub fn ty_vars(&self) -> HashSet<usize> {
    self.kind.ty_vars()
  }
}

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
  /// type alias — `Bar`.
  Alias(Symbol),
  /// boolean — `bool`.
  Bool,
  /// character — `char`.
  Char,
  /// string — `str`.
  Str,
  /// variable.
  Var(usize),
  /// array — `[]int`, `[3]str`.
  Array(Box<Ty>),
  /// closure — `fn()`, `fn(int): int`.
  Fn(Vec<Ty>, Box<Ty>),
}

impl TyKind {
  #[inline]
  pub fn is(&self, kind: TyKind) -> bool {
    *self == kind
  }

  #[inline]
  pub const fn is_numeric(&self) -> bool {
    matches!(self, Self::Int(..) | Self::Float(..))
  }

  pub fn ty_vars(&self) -> HashSet<usize> {
    match self {
      Self::Unit => todo!(),
      Self::Var(var) => {
        let mut ty_vars = HashSet::new();

        ty_vars.insert(*var);

        ty_vars
      }
      _ => todo!(),
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LitIntTy {
  Int(IntTy),
  Signed(SintTy),
  Unsigned(UintTy),
  Unsuffixed,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LitFloatTy {
  Suffixed(FloatTy),
  Unsuffixed,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IntTy {
  Int,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SintTy {
  S8,
  S16,
  S32,
  S64,
  S128,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UintTy {
  U8,
  U16,
  U32,
  U64,
  U128,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FloatTy {
  Float,
  F32,
  F64,
}
