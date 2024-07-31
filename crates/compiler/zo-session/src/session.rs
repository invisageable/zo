use zo_interner::interner::Interner;

pub struct Session {
  pub interner: Interner,
}

impl Session {
  #[inline]
  pub fn new() -> Self {
    Self {
      interner: Interner::new(),
    }
  }
}
