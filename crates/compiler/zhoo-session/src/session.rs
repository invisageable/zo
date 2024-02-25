use zo_core::interner::Interner;

#[derive(Debug)]
pub struct Session {
  pub interner: Interner,
}

impl Default for Session {
  fn default() -> Self {
    Self {
      interner: Interner::new(),
    }
  }
}
