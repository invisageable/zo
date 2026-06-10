use serde::Serialize;
use zo_interner::Symbol;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TokenId(pub u32);

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Token {
  // Special
  Unknown,
  Eof,

  // Literals
  Int,          // Side table index
  Float,        // Side table index
  String,       // Side table index
  InterpString, // Side table index (string with {var} interpolation)
  RawString,    // Side table index
  Char,         // Side table index
  Bytes,        // Side table index
  RegexLit,     // Side table index (regex_literals: (pattern, flags))

  // Identifiers & Keywords
  Ident, // Side table index
  // Keywords...
  Fun,
  Fn, // For closures
  Mut,
  Imu,
  Own, // `own self` — consuming receiver
  If,
  Else,
  While,
  For,
  Loop,
  Nursery,
  Select,
  Spawn,
  Supervise,
  /// Synthetic marker emitted by the parser when it
  /// recognizes the contextual `thread` modifier
  /// after `spawn` (`spawn thread fn()`). The
  /// tokenizer does NOT produce this variant — it
  /// still lexes the user-written word `thread` as
  /// a plain `Ident`, so user code remains free to
  /// name variables / functions / types `thread`.
  /// This variant only exists in the parse tree,
  /// distinguishing `spawn thread fn()` (OS thread)
  /// from `spawn fn()` (green task) for downstream
  /// SIR / codegen.
  Thread,
  Await,
  Pack,
  Load,
  Type,
  Ffi,
  Abstract,
  /// Reserved keyword for the dyn-dispatch type
  /// modifier `any <Abstract>`. The parser also
  /// accepts it as a member name after `.` / `::`
  /// (keywords-as-members) so user methods like
  /// `arr.any(pred)` keep parsing.
  Any,
  Apply,
  State,
  Struct,
  Enum,
  Group,
  And,
  Return,
  Break,
  Continue,
  Match,
  When,
  As,
  Is,
  True,
  False,
  SelfUpper, // Self
  SelfLower, // self
  Raw,
  Val,
  Wasm,
  Pub,
  Test,

  // Primitive type keywords
  IntType,      // int
  S8Type,       // s8
  S16Type,      // s16
  S32Type,      // s32
  S64Type,      // s64
  UintType,     // uint
  U8Type,       // u8
  U16Type,      // u16
  U32Type,      // u32
  U64Type,      // u64
  FloatType,    // float
  F32Type,      // f32
  F64Type,      // f64
  BoolType,     // bool
  BytesType,    // bytes
  CharType,     // char
  StrType,      // str
  TemplateType, // </>
  FnType,       // Fn (uppercase, function type annotation)

  // Punctuation & Delimiters
  LParen,
  RParen, // ( )
  LBrace,
  RBrace, // { }
  LBracket,
  RBracket, // [ ]
  Comma,
  Dot,
  Semicolon,
  Colon,
  Arrow,      // ->
  FatArrow,   // =>
  PipeArrow,  // |>
  Underscore, // _
  Hash,       // #
  Dollar,     // $
  Attribute,  // %%
  Ellipsis,   // ...

  // Operators
  Eq,
  EqEq, // =, ==
  Bang,
  BangEq, // !, !=
  Plus,
  PlusPlus,
  PlusEq, // +, ++, +=
  Minus,
  UnaryMinus, // Parser-only: prefix negation.
  MinusEq,    // -, -=
  Star,
  StarEq, // *, *=
  Slash2,
  SlashEq, // /, /=
  Percent,
  PercentEq, // %, %=
  Amp,
  AmpAmp,
  AmpEq, // &, &&, &=
  Pipe,
  PipePipe,
  PipeEq, // |, ||, |=
  Caret,
  CaretEq, // ^, ^=
  LShift,
  LShiftEq, // <<, <<=
  RShift,
  RShiftEq, // >>, >>=
  Lt,
  LtEq, // <, <=
  Gt,
  GtEq, // >, >=
  DotDot,
  DotDotEq,   // .., ..=
  ColonColon, // ::
  ColonEq,    // :=
  Question,   // ?
  At,         // @

  // Template Syntax Tokens
  LAngle,
  RAngle,                // <, >
  Slash,                 // /
  TemplateAssign,        // ::=
  TemplateFatArrow, // =:>  — closure-body opener; body is a template fragment.
  TemplateFragmentStart, // <>
  TemplateFragmentEnd, // </>
  TemplateText,     // Raw text inside a tag - uses main token span

  // Style Syntax Tokens
  StyleValue, // Raw CSS value text: "cyan", "#b2f5ea", "100%"
}

impl Token {
  /// Checks if the token kind is an operand.
  #[inline(always)]
  pub fn is_operand(&self) -> bool {
    matches!(
      self,
      Self::Ident
        | Self::Int
        | Self::Float
        | Self::String
        | Self::InterpString
        | Self::RawString
        | Self::Char
        | Self::Bytes
        | Self::RegexLit
        | Self::True
        | Self::False
        | Self::SelfLower
        | Self::SelfUpper
        | Self::Dollar
    )
  }

  /// True when the next `/` is division, not the start of
  /// a regex literal. `}` is excluded: in zo a closing
  /// brace always ends a block or struct literal, so the
  /// next significant token starts a fresh expression
  /// context.
  #[inline(always)]
  pub fn is_after_operand(&self) -> bool {
    self.is_operand() || matches!(self, Self::RParen | Self::RBracket)
  }

  /// Positive predicate: `true` when this token's *successor*
  /// position can legally open a regex literal (`/pat/flags`).
  ///
  /// @note — narrow, hand-picked whitelist of syntactic
  /// expression-start positions, not the negation of
  /// `is_after_operand`. After everything outside this set
  /// the next `/` is `Slash` (or `SlashEq`), so plain math
  /// like `a / b + c / d` never speculatively enters regex
  /// mode. Matches the entire `regex_literal_basic.zo`
  /// corpus — every literal sits right after `=` or `(`.
  #[inline(always)]
  pub fn is_regex_prefix(&self) -> bool {
    matches!(
      self,
      Self::Eq
        | Self::ColonEq
        | Self::PlusEq
        | Self::MinusEq
        | Self::StarEq
        | Self::SlashEq
        | Self::PercentEq
        | Self::AmpEq
        | Self::PipeEq
        | Self::CaretEq
        | Self::LShiftEq
        | Self::RShiftEq
        | Self::LParen
        | Self::LBracket
        | Self::LBrace
        | Self::Comma
        | Self::Semicolon
        | Self::Return
        | Self::When
        | Self::If
        | Self::Else
        | Self::Match
    )
  }

  /// Positive predicate: `true` when a `<` right after this
  /// token opens a template (a tag, never a comparison).
  ///
  /// @note — deliberately narrower than `is_regex_prefix`:
  /// only *value* positions — `return`, a block tail (after
  /// `{` / `;`), a `match` arm (after `=>`). Assignment
  /// operators are excluded by language design: `::=` is the
  /// one and only template binding form, so `:= <p>` and
  /// `= <p>` stay `Lt` and fail downstream as the invalid
  /// programs they are.
  #[inline(always)]
  pub fn is_template_opener(&self) -> bool {
    matches!(
      self,
      Self::Return | Self::LBrace | Self::Semicolon | Self::FatArrow
    )
  }

  /// Checks if the token kind is a type keyword.
  #[inline(always)]
  pub fn is_ty(&self) -> bool {
    matches!(
      self,
      Self::IntType
        | Self::S8Type
        | Self::S16Type
        | Self::S32Type
        | Self::S64Type
        | Self::UintType
        | Self::U8Type
        | Self::U16Type
        | Self::U32Type
        | Self::U64Type
        | Self::FloatType
        | Self::F32Type
        | Self::F64Type
        | Self::BoolType
        | Self::CharType
        | Self::StrType
        | Self::BytesType
        | Self::TemplateType
        | Self::FnType
    )
  }

  /// Statement-introducing keywords — the narrow subset the
  /// operand buffer must NOT reorder as a value. This is NOT
  /// the full reserved-word set (it excludes the value
  /// keywords `true`/`false`/`self` and the type keywords,
  /// which ARE operands); use [`Self::is_reserved_word`] to
  /// ask "can this be an identifier?".
  #[inline(always)]
  pub fn is_keyword(&self) -> bool {
    matches!(
      self,
      Self::Fun
        | Self::Imu
        | Self::Mut
        | Self::Own
        | Self::Val
        | Self::Return
        | Self::If
        | Self::Else
        | Self::While
        | Self::For
        | Self::Test
    )
  }

  /// Checks if the token is a reserved word — any keyword the
  /// tokenizer maps to a dedicated `Token`, so it can never be
  /// an identifier (variable, parameter, field, or pattern
  /// binder). Composed from the narrower classifiers plus the
  /// remaining keywords, so every keyword is listed exactly
  /// once. Keep the remainder in lockstep with the keyword
  /// table in `zo-tokenizer`.
  #[inline(always)]
  pub fn is_reserved_word(&self) -> bool {
    self.is_keyword()
      || self.is_ty()
      || matches!(
        self,
        // declarations and modifiers.
        Self::Pub
          | Self::Ffi
          | Self::Pack
          | Self::Load
          | Self::Wasm
          | Self::Type
          | Self::Struct
          | Self::Enum
          | Self::Group
          | Self::Apply
          | Self::Abstract
          | Self::State
          | Self::Raw
          // control flow and value keywords.
          | Self::When
          | Self::Loop
          | Self::Match
          | Self::Break
          | Self::Continue
          | Self::And
          | Self::As
          | Self::Is
          | Self::Any
          | Self::True
          | Self::False
          | Self::SelfLower
          | Self::SelfUpper
          | Self::Fn
          // concurrency keywords.
          | Self::Nursery
          | Self::Supervise
          | Self::Spawn
          | Self::Await
          | Self::Select
      )
  }

  /// Returns the source text for type keyword tokens.
  ///
  /// In pattern binding positions a type keyword is used as
  /// a variable name (e.g., `Result::Pass(bytes)` where
  /// `bytes` is tokenized as `BytesType`). This method
  /// provides the text so the executor can intern it as a
  /// binding symbol.
  pub fn ty_keyword_str(&self) -> Option<&'static str> {
    match self {
      Self::IntType => Some("int"),
      Self::S8Type => Some("s8"),
      Self::S16Type => Some("s16"),
      Self::S32Type => Some("s32"),
      Self::S64Type => Some("s64"),
      Self::UintType => Some("uint"),
      Self::U8Type => Some("u8"),
      Self::U16Type => Some("u16"),
      Self::U32Type => Some("u32"),
      Self::U64Type => Some("u64"),
      Self::FloatType => Some("float"),
      Self::F32Type => Some("f32"),
      Self::F64Type => Some("f64"),
      Self::BoolType => Some("bool"),
      Self::BytesType => Some("bytes"),
      Self::CharType => Some("char"),
      Self::StrType => Some("str"),
      _ => None,
    }
  }
}

/// Compact, cache-friendly token buffer using Structure of Arrays layout
#[derive(Serialize)]
pub struct TokenBuffer {
  pub kinds: Vec<Token>,
  pub starts: Vec<u32>,
  pub lengths: Vec<u16>,

  // Literal indices run parallel to kinds/starts/lengths
  // For tokens with literals (Ident, Int, Float, String, etc), this contains
  // the index into the corresponding literal array.
  // For tokens without literals (Semicolon, Plus, etc), this is 0.
  pub literal_indices: Vec<u32>,
}

impl TokenBuffer {
  pub fn new() -> Self {
    Self {
      kinds: Vec::new(),
      starts: Vec::new(),
      lengths: Vec::new(),
      literal_indices: Vec::new(),
    }
  }

  pub fn with_capacity(cap: usize) -> Self {
    Self {
      kinds: Vec::with_capacity(cap),
      starts: Vec::with_capacity(cap),
      lengths: Vec::with_capacity(cap),
      literal_indices: Vec::with_capacity(cap),
    }
  }

  #[inline(always)]
  pub fn push(&mut self, kind: Token, start: u32, len: u16) -> TokenId {
    let idx = self.kinds.len() as u32;

    self.kinds.push(kind);
    self.starts.push(start);
    self.lengths.push(len);
    self.literal_indices.push(0);

    TokenId(idx)
  }

  #[inline(always)]
  pub fn push_with_literal(
    &mut self,
    kind: Token,
    start: u32,
    len: u16,
    literal_idx: u32,
  ) -> TokenId {
    let idx = self.kinds.len() as u32;

    self.kinds.push(kind);
    self.starts.push(start);
    self.lengths.push(len);
    self.literal_indices.push(literal_idx);

    TokenId(idx)
  }

  #[inline(always)]
  pub fn len(&self) -> usize {
    self.kinds.len()
  }

  #[inline(always)]
  pub fn is_empty(&self) -> bool {
    self.kinds.is_empty()
  }

  #[inline(always)]
  pub fn get(&self, token: TokenId) -> Option<(Token, u32, u16)> {
    let idx = token.0 as usize;

    if idx < self.kinds.len() {
      Some((self.kinds[idx], self.starts[idx], self.lengths[idx]))
    } else {
      None
    }
  }

  #[inline(always)]
  pub fn span(&self, token: TokenId) -> Option<(u32, u16)> {
    let idx = token.0 as usize;

    if idx < self.kinds.len() {
      Some((self.starts[idx], self.lengths[idx]))
    } else {
      None
    }
  }
}

impl Default for TokenBuffer {
  fn default() -> Self {
    Self::new()
  }
}

/// Segment of an interpolation string, pre-parsed by tokenizer.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum InterpSegment {
  /// Static text between {} markers.
  Literal(Symbol),
  /// Variable name inside {}.
  Variable(Symbol),
}

/// Storage for literal values extracted during tokenization
/// The tokenizer stores parsed literal values
#[derive(Serialize)]
pub struct LiteralStore {
  pub int_literals: Vec<u64>,
  pub float_literals: Vec<f64>,
  pub identifiers: Vec<Symbol>,
  pub bytes_literals: Vec<u8>,
  pub char_literals: Vec<u32>,
  pub string_literals: Vec<Symbol>,
  /// Interpolation segments (flat array, indexed by ranges).
  pub interp_segments: Vec<InterpSegment>,
  /// Per-InterpString token: (start, count) into interp_segments.
  pub interp_ranges: Vec<(u32, u16)>,
  /// Per-RegexLit token: `(pattern_symbol, flags_symbol)`.
  pub regex_literals: Vec<(Symbol, Symbol)>,
}

/// Captured lengths of every literal vector. Paired with
/// `TreeBaseline` for parse-cache rewind between cross-
/// module splices.
#[derive(Clone, Copy, Debug)]
pub struct LiteralStoreBaseline {
  pub ints: usize,
  pub floats: usize,
  pub idents: usize,
  pub bytes: usize,
  pub chars: usize,
  pub strings: usize,
  pub interp_segments: usize,
  pub interp_ranges: usize,
  pub regex: usize,
}

impl LiteralStore {
  pub fn new() -> Self {
    Self {
      identifiers: Vec::new(),
      string_literals: Vec::new(),
      int_literals: Vec::new(),
      float_literals: Vec::new(),
      char_literals: Vec::new(),
      bytes_literals: Vec::new(),
      interp_segments: Vec::new(),
      interp_ranges: Vec::new(),
      regex_literals: Vec::new(),
    }
  }

  pub fn with_capacity(cap: usize) -> Self {
    Self {
      int_literals: Vec::with_capacity(cap / 5),
      float_literals: Vec::with_capacity(cap / 20),
      identifiers: Vec::with_capacity(cap),
      bytes_literals: Vec::with_capacity(cap / 100),
      char_literals: Vec::with_capacity(cap / 20),
      string_literals: Vec::with_capacity(cap / 10),
      interp_segments: Vec::new(),
      interp_ranges: Vec::new(),
      regex_literals: Vec::new(),
    }
  }

  /// Snapshot of every literal vector length. Paired with
  /// `Tree::baseline` so the compiler driver can rewind a
  /// cached `(TokenizationResult, ParsingResult)` between
  /// analyze invocations.
  pub fn baseline(&self) -> LiteralStoreBaseline {
    LiteralStoreBaseline {
      ints: self.int_literals.len(),
      floats: self.float_literals.len(),
      idents: self.identifiers.len(),
      bytes: self.bytes_literals.len(),
      chars: self.char_literals.len(),
      strings: self.string_literals.len(),
      interp_segments: self.interp_segments.len(),
      interp_ranges: self.interp_ranges.len(),
      regex: self.regex_literals.len(),
    }
  }

  /// Rewinds every literal vector to a saved baseline.
  /// Splice replays literals at the tail (see
  /// `splice_one`), so tail-truncate is enough.
  pub fn truncate_to(&mut self, baseline: LiteralStoreBaseline) {
    self.int_literals.truncate(baseline.ints);
    self.float_literals.truncate(baseline.floats);
    self.identifiers.truncate(baseline.idents);
    self.bytes_literals.truncate(baseline.bytes);
    self.char_literals.truncate(baseline.chars);
    self.string_literals.truncate(baseline.strings);
    self.interp_segments.truncate(baseline.interp_segments);
    self.interp_ranges.truncate(baseline.interp_ranges);
    self.regex_literals.truncate(baseline.regex);
  }

  /// Push a `(pattern, flags)` pair; return its index.
  #[inline(always)]
  pub fn push_regex(&mut self, pat: Symbol, flags: Symbol) -> u32 {
    let idx = self.regex_literals.len() as u32;

    self.regex_literals.push((pat, flags));

    idx
  }

  #[inline(always)]
  pub fn push_identifier(&mut self, symbol: Symbol) -> u32 {
    let idx = self.identifiers.len() as u32;
    self.identifiers.push(symbol);
    idx
  }

  #[inline(always)]
  pub fn push_string_symbol(&mut self, symbol: Symbol) -> u32 {
    let idx = self.string_literals.len() as u32;

    self.string_literals.push(symbol);

    idx
  }

  #[inline(always)]
  pub fn push_int(&mut self, val: u64) -> u32 {
    let idx = self.int_literals.len() as u32;

    self.int_literals.push(val);

    idx
  }

  #[inline(always)]
  pub fn push_float(&mut self, val: f64) -> u32 {
    let idx = self.float_literals.len() as u32;

    self.float_literals.push(val);

    idx
  }

  #[inline(always)]
  pub fn push_char(&mut self, value: u32) -> u32 {
    let idx = self.char_literals.len() as u32;

    self.char_literals.push(value);

    idx
  }

  #[inline(always)]
  pub fn push_bytes(&mut self, value: u8) -> u32 {
    let idx = self.bytes_literals.len() as u32;

    self.bytes_literals.push(value);

    idx
  }

  /// Push interpolation segments for an InterpString token.
  /// Returns the index into interp_ranges.
  #[inline(always)]
  pub fn push_interp(&mut self, segments: &[InterpSegment]) -> u32 {
    let idx = self.interp_ranges.len() as u32;
    let start = self.interp_segments.len() as u32;

    self.interp_segments.extend_from_slice(segments);

    self.interp_ranges.push((start, segments.len() as u16));

    idx
  }

  /// Get interpolation segments for a given interp range
  /// index.
  #[inline(always)]
  pub fn interp_segs(&self, range_idx: u32) -> &[InterpSegment] {
    let (start, count) = self.interp_ranges[range_idx as usize];

    let end = start as usize + count as usize;

    &self.interp_segments[start as usize..end]
  }
}

impl Default for LiteralStore {
  fn default() -> Self {
    Self::new()
  }
}
