/// The representation of a supply.
#[derive(Default)]
pub struct Supply(usize);

impl Supply {
  /// Creates a new supply.
  #[inline]
  pub fn new() -> Self {
    Self(0usize)
  }

  /// Gets the next supply id.
  #[inline]
  pub fn inc(&mut self) -> usize {
    let var = self.0;

    self.0 += 1;

    var
  }
}
