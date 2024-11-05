/// The representation of a cursor.
pub struct Cursor {
  /// A position of a cursor within a source file.
  pos: usize,
  /// A current source file.
  source: String,
}

impl Cursor {
  /// Creates a new cursor instance.
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

  /// Gets the current source file.
  pub fn source(&self) -> &str {
    self.source.as_str()
  }

  /// Peeks the current character.
  pub fn peek(&self) -> Option<char> {
    self.source.chars().nth(self.pos())
  }

  /// Moves to the next character.
  pub fn next(&mut self) -> Option<char> {
    if self.pos < self.source.len() {
      let maybe_ch = self.peek();

      self.pos += 1usize;

      maybe_ch
    } else {
      None
    }
  }

  /// Moves cursor to the next position.
  pub fn front(&mut self) -> char {
    self
      .source
      .chars()
      .nth(self.pos() + 1usize)
      .unwrap_or_default()
  }

  /// Consumes while the next character is a whitespace character.
  pub fn consume_whitespace(&mut self) {
    self.consume_while(char::is_whitespace);
  }

  /// Consumes while the next character matches from the condition.
  pub fn consume_while(&mut self, condition: impl Fn(char) -> bool) -> String {
    let mut buf = String::with_capacity(0usize);

    while let Some(ch) = self.source[self.pos()..].chars().next() {
      if condition(ch) {
        buf.push(ch);
        self.pos += ch.len_utf8();
      } else {
        break;
      }
    }

    buf
  }
}

#[cfg(test)]
mod tests {}
