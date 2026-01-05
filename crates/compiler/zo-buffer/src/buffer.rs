/// Represents a [`Buffer`] instance.
pub struct Buffer {
  /// The inner buffer.
  inner: Vec<u8>,
}
impl Buffer {
  /// Creates a new [`Buffer`] instance.
  pub fn new() -> Self {
    Self {
      inner: Vec::with_capacity(16 * 1024), // Start with 16KB.
    }
  }

  /// Adds bytes to the buffer.
  #[inline(always)]
  pub fn bytes(&mut self, s: &[u8]) {
    self.inner.extend_from_slice(s);
  }

  /// Adds str to the buffer.
  #[inline(always)]
  pub fn str(&mut self, s: &str) {
    self.bytes(s.as_bytes());
  }

  /// Adds char to the buffer.
  #[inline(always)]
  pub fn char(&mut self, c: u8) {
    self.inner.push(c);
  }

  /// Adds u32 integer to the buffer.
  #[inline(always)]
  pub fn u32(&mut self, n: u32) {
    let mut inner = itoa::Buffer::new();
    let printed = inner.format(n);

    self.bytes(printed.as_bytes());
  }

  /// Adds u64 integer to the buffer.
  #[inline(always)]
  pub fn u64(&mut self, n: u64) {
    let mut inner = itoa::Buffer::new();
    let printed = inner.format(n);

    self.bytes(printed.as_bytes());
  }

  /// Adds f32 floating-point to the buffer.
  #[inline(always)]
  pub fn f32(&mut self, n: f32) {
    let mut inner = zmij::Buffer::new();
    let printed = inner.format(n);

    self.bytes(printed.as_bytes());
  }

  /// Adds f64 floating-point to the buffer.
  #[inline(always)]
  pub fn f64(&mut self, n: f64) {
    let mut inner = zmij::Buffer::new();
    let printed = inner.format(n);

    self.bytes(printed.as_bytes());
  }

  /// Adds newline to the buffer.
  #[inline(always)]
  pub fn newline(&mut self) {
    self.inner.push(b'\n');
  }

  /// Adds indentation to the buffer.
  #[inline(always)]
  pub fn indent(&mut self) {
    self.bytes(b"  ");
  }

  pub fn is_empty(self) -> bool {
    self.inner.is_empty()
  }

  pub fn len(self) -> usize {
    self.inner.len()
  }

  pub fn finish(self) -> Vec<u8> {
    self.inner
  }
}
impl Default for Buffer {
  fn default() -> Self {
    Self::new()
  }
}
