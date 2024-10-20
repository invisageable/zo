use zor_interner::interner::Interner;

/// The representation of a session.
#[derive(Debug)]
pub struct Session {
  /// A string interner.
  pub interner: Interner,
}

impl Default for Session {
  /// Creates a new session with default values.
  fn default() -> Self {
    Self {
      interner: Interner::new(),
    }
  }
}
