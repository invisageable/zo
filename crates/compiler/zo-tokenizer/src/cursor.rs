/// The representation of a cursor.
///
/// It implements the trait [`Iterator`] to iterate over bytes from a source
/// file.   
///   
/// #### examples.
///
/// ```ignore
/// use zo_tokenizer::cursor::Cursor;
///
/// let source = b"40 + 2";
/// let cursor = Cursor::new(source);
///
/// for byte in cursor {
///   println!("{byte}");
/// }
/// ```
#[derive(Debug, Default)]
pub struct Cursor<'bytes> {
  /// The position of the cursor.
  pos: usize,
  /// The source code as bytes.
  source: &'bytes [u8],
}

impl<'bytes> Cursor<'bytes> {
  /// Creates a new cursor.
  #[inline]
  pub fn new(source: &'bytes [u8]) -> Self {
    Self { pos: 0, source }
  }

  /// Checks if we can continue to eat.
  #[inline]
  pub fn has_bytes(&self) -> bool {
    self.pos < self.source.len()
  }

  /// Gets the cursor position.
  #[inline]
  pub fn pos(&self) -> usize {
    self.pos
  }

  /// Gets the bytes from location [start, end].
  #[inline]
  pub fn bytes(&self, start: usize, end: usize) -> &'bytes [u8] {
    &self.source[start..end]
  }

  /// Gets the current byte.
  #[inline]
  pub fn byte(&self) -> u8 {
    self.source[self.pos]
  }

  /// Gets the next byte.
  #[inline]
  pub fn nbyte(&self) -> u8 {
    self.source[self.pos + 1]
  }

  /// moves cursor to the next position.
  #[inline]
  pub fn bump(&mut self) {
    self.pos += 1;
  }

  /// moves cursor to the previous position.
  #[inline]
  pub fn back(&mut self) {
    self.pos -= 1;
  }
}

impl<'bytes> std::iter::Iterator for Cursor<'bytes> {
  type Item = u8;

  /// Moves to the next byte.
  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    if self.has_bytes() {
      return Some(self.byte());
    }

    None
  }
}
