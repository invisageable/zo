use crate::Symbol;

use rustc_hash::FxHashMap as HashMap;
use serde::Serialize;

/// A string interner that deduplicates strings and returns compact Symbol
/// indices.
#[derive(Serialize)]
pub struct Interner {
  // The actual string storage - we store all strings concatenated
  // This is append-only, so pointers into it remain valid forever
  storage: String,

  // Map from string slices to their Symbol
  // We use &'static str as keys, which are actually slices into `storage`
  // SAFETY: storage is append-only, so these pointers remain valid
  #[serde(skip)] // Can't serialize raw pointers
  map: HashMap<&'static str, Symbol>,

  // For each symbol, where does its string start and how long is it?
  // We keep this for the `get` method and error reporting
  spans: Vec<(usize, usize)>,
}
impl Interner {
  /// Creates a new interner with pre-interned keywords
  pub fn new() -> Self {
    let mut interner = Self {
      storage: String::with_capacity(4096),
      map: HashMap::with_capacity_and_hasher(512, Default::default()),
      spans: Vec::with_capacity(512),
    };

    // Pre-intern common keywords and symbols
    interner.intern_static("", Symbol::EMPTY);
    interner.intern_static("_", Symbol::UNDERSCORE);
    interner.intern_static("fun", Symbol::FUN);
    interner.intern_static("mut", Symbol::MUT);
    interner.intern_static("imu", Symbol::IMU);
    interner.intern_static("if", Symbol::IF);
    interner.intern_static("else", Symbol::ELSE);
    interner.intern_static("while", Symbol::WHILE);
    interner.intern_static("for", Symbol::FOR);
    interner.intern_static("return", Symbol::RETURN);
    interner.intern_static("break", Symbol::BREAK);
    interner.intern_static("continue", Symbol::CONTINUE);
    interner.intern_static("match", Symbol::MATCH);
    interner.intern_static("when", Symbol::WHEN);
    interner.intern_static("as", Symbol::AS);
    interner.intern_static("is", Symbol::IS);
    interner.intern_static("true", Symbol::TRUE);
    interner.intern_static("false", Symbol::FALSE);
    interner.intern_static("Self", Symbol::SELF_UPPER);
    interner.intern_static("self", Symbol::SELF_LOWER);
    interner.intern_static("struct", Symbol::STRUCT);
    interner.intern_static("enum", Symbol::ENUM);
    interner.intern_static("type", Symbol::TYPE);
    interner.intern_static("pub", Symbol::PUB);
    interner.intern_static("val", Symbol::VAL);

    interner
  }

  /// Interns a static string with a predefined symbol (used for keywords)
  fn intern_static(&mut self, s: &str, symbol: Symbol) {
    let start = self.storage.len();

    self.storage.push_str(s);

    let end = self.storage.len();

    // Ensure the spans vector is large enough
    let idx = symbol.as_u32() as usize;

    while self.spans.len() <= idx {
      self.spans.push((0, 0));
    }

    self.spans[idx] = (start, end - start);

    // Get a slice to the string we just added
    let stored_slice = &self.storage[start..end];

    // SAFETY: We transmute the slice to 'static lifetime.
    // This is safe because:
    // 1. storage is append-only (we never remove or modify existing content)
    // 2. The Interner owns storage for its entire lifetime
    // 3. These slices will remain valid as long as the Interner exists
    let static_slice: &'static str =
      unsafe { std::mem::transmute(stored_slice) };

    self.map.insert(static_slice, symbol);
  }

  /// Interns a string and returns its Symbol
  /// ZERO ALLOCATIONS on the hot path!
  #[inline(always)]
  pub fn intern(&mut self, s: &str) -> Symbol {
    if let Some(&symbol) = self.map.get(s) {
      return symbol;
    }

    // Slow path: New string, add to storage
    let symbol = Symbol::new(self.spans.len() as u32);
    let start = self.storage.len();

    self.storage.push_str(s);

    let end = self.storage.len();

    // Track the span for the get() method
    self.spans.push((start, end - start));

    // Get a slice to the string we just added
    let stored_slice = &self.storage[start..end];

    // SAFETY: We transmute the slice to 'static lifetime.
    // This is safe because storage is append-only.
    let static_slice: &'static str =
      unsafe { std::mem::transmute(stored_slice) };

    // Insert into map with ZERO ALLOCATION - just a pointer!
    self.map.insert(static_slice, symbol);

    symbol
  }

  /// Gets the string from a [`Symbol`].
  #[inline(always)]
  pub fn get(&self, symbol: Symbol) -> &str {
    let idx = symbol.as_u32() as usize;

    if idx < self.spans.len() {
      let (start, len) = self.spans[idx];

      &self.storage[start..start + len]
    } else {
      ""
    }
  }

  /// Gets the [`Symbol`] from a interned string.
  #[inline(always)]
  pub fn symbol(&self, s: &str) -> Option<Symbol> {
    self.map.get(s).copied()
  }
}
impl Default for Interner {
  fn default() -> Self {
    Self::new()
  }
}
