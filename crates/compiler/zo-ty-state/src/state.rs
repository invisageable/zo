/// Represents a [`TyState`] instance.
pub struct TyState {}
impl TyState {
  /// Creates a new [`TyState`] instance.
  pub const fn new() -> Self {
    Self {}
  }
}
impl Default for TyState {
  fn default() -> Self {
    Self::new()
  }
}
