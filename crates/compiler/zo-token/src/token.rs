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
  Int,       // Side table index
  Float,     // Side table index
  String,    // Side table index
  RawString, // Side table index
  Char,      // Side table index
  Bytes,     // Side table index

  // Identifiers & Keywords
  Ident, // Side table index
  // Keywords...
  Fun,
  Fn, // For closures
  Mut,
  Imu,
  If,
  Else,
  While,
  For,
  Loop,
  Nursery,
  Spawn,
  Await,
  Pack,
  Load,
  Type,
  Ext,
  Abstract,
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
  PlusEq, // +, +=
  Minus,
  MinusEq, // -, -=
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
  TemplateFragmentStart, // <>
  TemplateFragmentEnd,   // </>
  TemplateText,          // Raw text inside a tag - uses main token span
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
        | Self::RawString
        | Self::Char
        | Self::Bytes
        | Self::True
        | Self::False
        | Self::SelfLower
        | Self::SelfUpper
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
    )
  }

  /// Checks if the token kind is a keyword.
  #[inline(always)]
  pub fn is_keyword(&self) -> bool {
    matches!(
      self,
      Self::Fun
        | Self::Imu
        | Self::Mut
        | Self::Val
        | Self::Return
        | Self::If
        | Self::Else
        | Self::While
        | Self::For
    )
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

/// Storage for literal values extracted during tokenization
/// The tokenizer stores parsed literal values
#[derive(Serialize)]
pub struct LiteralStore {
  pub int_literals: Vec<u64>,
  pub float_literals: Vec<f64>,
  pub identifiers: Vec<Symbol>,
  pub bytes_literals: Vec<(u32, u16)>,
  pub char_literals: Vec<(u32, u16)>,
  pub string_literals: Vec<Symbol>,
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
    }
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
  pub fn push_char_span(&mut self, start: u32, len: u16) -> u32 {
    let idx = self.char_literals.len() as u32;

    self.char_literals.push((start, len));

    idx
  }

  #[inline(always)]
  pub fn push_bytes_span(&mut self, start: u32, len: u16) -> u32 {
    let idx = self.bytes_literals.len() as u32;

    self.bytes_literals.push((start, len));

    idx
  }
}
impl Default for LiteralStore {
  fn default() -> Self {
    Self::new()
  }
}
