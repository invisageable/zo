#![allow(dead_code)]

#[derive(Debug, Default)]
pub(crate) struct VarBuilder {
  index: u32,
}

impl VarBuilder {
  #[inline]
  pub(crate) fn new() -> Self {
    Self::default()
  }
}
