/// The representation of a value.
#[derive(Clone, Copy, Debug)]
pub struct Value;
impl Value {
  /// The zero value, it is used as a placeholder.
  pub const ZERO: Self = Self;
}
