use crate::Symbol;

use rustc_hash::FxHashMap as HashMap;
use serde::Serialize;

/// A string interner that deduplicates strings and returns
/// compact `Symbol` indices.
///
/// Each interned string is its own `Box<str>`, kept alive
/// for the interner's lifetime. Boxed slices have stable
/// heap addresses — pushing more entries into `strings`
/// reshuffles `(ptr, len)` tuples in the `Vec`, but the
/// underlying buffers each `Box<str>` owns do not move.
/// That stability is what lets the `map` hold `&'static
/// str` keys derived from those buffers without dangling.
///
/// The previous shape used a single `String` for storage
/// and stored `&'static str` pointers into it. `String`
/// reallocates on `push_str` once capacity is exceeded,
/// silently invalidating every prior key in `map`. Most of
/// the time the freed bytes were still allocator-cached and
/// lookups happened to succeed; about 1 run in 30 the
/// memory had been reused, lookups returned `None`, names
/// re-interned to fresh symbols, and the analyzer reported
/// spurious "undefined variable" mid-function. That's the
/// non-determinism the synth-10K bench surfaced.
#[derive(Serialize)]
pub struct Interner {
  /// Per-symbol heap-stable string storage. Symbols are
  /// indices into this vector.
  strings: Vec<Box<str>>,
  /// Reverse map for `intern` deduplication. Keys are
  /// `&'static str` slices into the corresponding
  /// `Box<str>` in `strings`. The `'static` lifetime is a
  /// stand-in for "lives as long as the interner"; safety
  /// rests on every `Box<str>` outliving every key derived
  /// from it (see `intern` for the SAFETY note).
  #[serde(skip)]
  map: HashMap<&'static str, Symbol>,
}

impl Interner {
  const STRINGS_CAPACITY: usize = 512;
  const MAP_CAPACITY: usize = 512;

  pub fn new() -> Self {
    let mut interner = Self {
      strings: Vec::with_capacity(Self::STRINGS_CAPACITY),
      map: HashMap::with_capacity_and_hasher(
        Self::MAP_CAPACITY,
        Default::default(),
      ),
    };

    // Pre-intern common keywords and symbols.
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

  /// Interns a static string with a predefined symbol
  /// (used for keywords). Pads `strings` with empty boxes
  /// if `symbol` skips ahead so each symbol's index lines
  /// up with its slot.
  fn intern_static(&mut self, s: &str, symbol: Symbol) {
    let idx = symbol.as_u32() as usize;

    while self.strings.len() <= idx {
      self.strings.push(Box::<str>::from(""));
    }

    let boxed: Box<str> = Box::from(s);

    // SAFETY: the `Box<str>` we just allocated has a
    // stable heap address that lives as long as `self`.
    // Storing it in `self.strings` keeps it alive; the
    // `&'static str` we extract is a pointer into that
    // heap buffer, valid for `self`'s lifetime. We never
    // hand a key out across `self`'s drop boundary.
    let static_slice: &'static str =
      unsafe { std::mem::transmute::<&str, &'static str>(&*boxed) };

    self.strings[idx] = boxed;
    self.map.insert(static_slice, symbol);
  }

  /// Interns a string and returns its [`Symbol`].
  #[inline(always)]
  pub fn intern(&mut self, s: &str) -> Symbol {
    if let Some(&symbol) = self.map.get(s) {
      return symbol;
    }

    let symbol = Symbol::new(self.strings.len() as u32);
    let boxed: Box<str> = Box::from(s);

    // SAFETY: as in `intern_static` — the boxed buffer is
    // owned by `self.strings` for the interner's lifetime,
    // so any key we derive from it stays valid.
    let static_slice: &'static str =
      unsafe { std::mem::transmute::<&str, &'static str>(&*boxed) };

    self.strings.push(boxed);
    self.map.insert(static_slice, symbol);

    symbol
  }

  /// Gets the string for a [`Symbol`].
  #[inline(always)]
  pub fn get(&self, symbol: Symbol) -> &str {
    let idx = symbol.as_u32() as usize;

    self.strings.get(idx).map(|s| s.as_ref()).unwrap_or("")
  }

  /// Gets the [`Symbol`] for an interned string, if any.
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
