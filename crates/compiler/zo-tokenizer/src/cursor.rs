/// The representation of a cursor.
///
/// It implements the trait [`Iterator`] to iterate over characters from a
/// source file.   
///   
/// #### examples.
///
/// ```ignore
/// use zo_tokenizer::cursor::Cursor;
///
/// let source = "40 + 2";
/// let cursor = Cursor::new(source);
///
/// while let Some(ch) = cursor.next() {
///   println!("{ch}");
/// }
/// ```
#[derive(Debug)]
pub struct Cursor<'source> {
  /// The position of a cursor within a source file.
  pos: std::cell::Cell<usize>,
  /// The current source file.
  source: &'source str,
}

impl<'source> Cursor<'source> {
  /// A cursor zero.
  pub const ZERO: Self = Self::new("");

  /// The reprensentation of a cursor.
  #[inline(always)]
  pub const fn new(source: &'source str) -> Self {
    Self {
      pos: std::cell::Cell::new(0usize),
      source,
    }
  }

  /// Gets the current position.
  #[inline]
  pub fn pos(&self) -> usize {
    self.pos.get()
  }

  /// Gets the current source file.
  pub fn source(&self) -> &'source str {
    self.source
  }

  /// Peeks the current character.
  #[inline]
  pub fn peek(&self) -> Option<char> {
    self.source.chars().nth(self.pos())
  }

  /// moves cursor to the previous position.
  #[inline]
  pub fn back(&mut self) -> char {
    self
      .source
      .chars()
      // .clone()
      .nth(self.pos() - 1usize)
      .unwrap_or_default()
  }

  /// moves cursor to the next position.
  #[inline]
  pub fn front(&mut self) -> char {
    self
      .source
      .chars()
      // .clone()
      .nth(self.pos() + 1usize)
      .unwrap_or_default()
  }

  /// Consumes while the next character is a whitespace character.
  pub fn consume_whitespace(&mut self) {
    self.consume_while(char::is_whitespace);
  }

  /// Consumes while the next character matches from the condition.
  pub fn consume_while(&mut self, condition: impl Fn(char) -> bool) -> String {
    let mut result = String::with_capacity(0usize);

    while let Some(ch) = self.source[self.pos()..].chars().next() {
      if condition(ch) {
        result.push(ch);
        self.pos.set(self.pos() + ch.len_utf8());
      } else {
        break;
      }
    }

    result
  }
}

impl<'source> Default for Cursor<'source> {
  /// Creates a default cursor — default values is sets to zero.
  #[inline(always)]
  fn default() -> Self {
    Self::ZERO
  }
}

impl<'a> Iterator for Cursor<'a> {
  type Item = char;

  /// Moves to the next character.
  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    if self.pos.get() < self.source.len() {
      let maybe_ch = self.peek();

      self.pos.set(self.pos.get() + 1usize);

      maybe_ch
    } else {
      None
    }
  }
}
