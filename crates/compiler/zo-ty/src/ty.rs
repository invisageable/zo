use swisskit::span::Span;

/// The representation of a type.
#[derive(Clone, Copy, Debug, PartialEq)]
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
}

/// The representation of different kind of type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TyKind {
  /// unit — `()`.
  Unit,
  /// integer — `int`, `s32`, `u32`.
  Int(LitIntTy),
  /// float — `float`, `f32`, `f64`.
  Float(LitFloatTy),
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
  S8,
  S16,
  S32,
  S64,
  S128,
}

/// The representation of different kind of an unsigned integer type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UintTy {
  U8,
  U16,
  U32,
  U64,
  U128,
}

/// The representation of different kind of a float type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FloatTy {
  /// `float` — by default is `f32`.
  Float,
  F32,
  F64,
}
