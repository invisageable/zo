#[derive(Default)]
pub struct Cursor<'bytes> {
  /// The position of the cursor.
  pos: usize,
  /// The source code as bytes.
  source: &'bytes [u8],
}

impl<'bytes> Cursor<'bytes> {
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
