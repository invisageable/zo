//! ...

pub enum Unit {
  Ns,
  Us,
  Ms,
  S,
}

impl Unit {
  #[inline]
  pub fn as_factor(&self) -> f64 {
    match self {
      Self::Ns => 1.0,
      Self::Us => 1_000.0,
      Self::Ms => 1_000_000.0,
      Self::S => 1_000_000_000.0,
    }
  }
}

impl std::fmt::Display for Unit {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Ns => write!(f, "ns"),
      Self::Us => write!(f, "us"),
      Self::Ms => write!(f, "ms"),
      Self::S => write!(f, "s"),
    }
  }
}

impl From<&'static str> for Unit {
  fn from(unit: &'static str) -> Self {
    match unit {
      "ns" => Unit::Ns,
      "us" => Unit::Us,
      "ms" => Unit::Ms,
      "s" => Unit::S,
      _ => unreachable!(),
    }
  }
}

impl From<Unit> for &'static str {
  fn from(unit: Unit) -> Self {
    match unit {
      Unit::Ns => "ns",
      Unit::Us => "us",
      Unit::Ms => "ms",
      Unit::S => "s",
    }
  }
}
