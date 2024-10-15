/// The representation of a cursor.
#[derive(Debug)]
pub struct Cursor {
  /// The position of a cursor within a source file.
  pos: usize,
  /// The current source file.
  source: String,
}

impl Cursor {
  /// The reprensentation of a cursor.
  pub fn new(source: &str) -> Self {
    Self {
      pos: 0usize,
      source: source.to_string(),
    }
  }

  /// Gets the current position.
  pub fn pos(&self) -> usize {
    self.pos
  }

  /// Peeks the current character.
  pub fn peek(&self) -> Option<char> {
    self.source.chars().nth(self.pos())
  }
}
