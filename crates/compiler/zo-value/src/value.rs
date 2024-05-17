#[derive(Debug)]
pub struct Value {
  pub kind: ValueKind,
}

impl Value {
  pub const UNIT: Self = Self {
    kind: ValueKind::Unit,
  };

  pub fn int(int: i64) -> Self {
    Self {
      kind: ValueKind::Int(int),
    }
  }

  pub fn float(float: f64) -> Self {
    Self {
      kind: ValueKind::Float(float),
    }
  }
}

#[derive(Debug)]
pub enum ValueKind {
  Unit,
  Int(i64),
  Float(f64),
}
