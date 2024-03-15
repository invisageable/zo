#![allow(dead_code)]

#[derive(Debug, Default)]
pub(crate) struct DataBuilder {
  index: u32,
}

impl DataBuilder {
  #[inline]
  pub(crate) fn new() -> Self {
    Self::default()
  }
}
