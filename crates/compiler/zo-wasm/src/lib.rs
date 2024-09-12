use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
pub struct Repl {}

impl Repl {
  #[inline]
  pub fn new() -> Self {
    Self {}
  }

  #[inline]
  pub fn add_value(mut self, value: &str) -> Self {
    self.value = value;
  }

  #[inline]
  pub fn eval(self) -> String {
    String::with_capacity(0usize)
  }
}
