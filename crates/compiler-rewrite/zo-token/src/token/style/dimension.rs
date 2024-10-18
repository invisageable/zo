/// The representation of dimension tokens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Dimension {
  /// A centimeter dimension.
  Cm,
  /// A millimeter dimension.
  Mm,
  /// A inch dimension.
  In,
  /// A pixel dimension.
  Px,
  /// A point dimension.
  Pt,
  /// A pica dimension.
  Pc,
  // Em,
  // Ex,
  // Ch,
  // Rem,
  // Vw,
  // Vh,
  // Vmin,
  // Vmax,
}

impl From<&str> for Dimension {
  fn from(dim: &str) -> Self {
    match dim {
      "cm" => Self::Cm,
      "mm" => Self::Mm,
      "in" => Self::In,
      "px" => Self::Px,
      "pt" => Self::Pt,
      "pc" => Self::Pc,
      _ => unreachable!("{dim}"),
    }
  }
}

impl std::fmt::Display for Dimension {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Cm => write!(f, "cm"),
      Self::Mm => write!(f, "mm"),
      Self::In => write!(f, "in"),
      Self::Px => write!(f, "px"),
      Self::Pt => write!(f, "pt"),
      Self::Pc => write!(f, "pc"),
    }
  }
}
