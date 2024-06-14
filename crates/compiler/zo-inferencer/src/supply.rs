//! ...

#[derive(Debug, Default)]
pub struct Supply(usize);

impl Supply {
  #[inline]
  pub fn new() -> Self {
    Self(0)
  }

  pub fn next(&mut self) -> usize {
    let var = self.0;

    self.0 += 1;

    var
  }
}
