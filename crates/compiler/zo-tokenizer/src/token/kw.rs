use super::TokenKind;

use hashbrown::{HashMap, HashSet};
use smol_str::SmolStr;

/// The keyword dictionnary.
type Keyword = HashMap<SmolStr, TokenKind>;
/// The type dictionnary.
type Type = HashSet<SmolStr>;

lazy_static::lazy_static! {
  // A static map of available keywords.
  pub static ref KEYWORD: Keyword = HashMap::from([
    (SmolStr::new_inline("abstract"), TokenKind::Kw(Kw::Abstract)),
    (SmolStr::new_inline("and"), TokenKind::Kw(Kw::And)),
    (SmolStr::new_inline("apply"), TokenKind::Kw(Kw::Apply)),
    (SmolStr::new_inline("as"), TokenKind::Kw(Kw::As)),
    (SmolStr::new_inline("async"), TokenKind::Kw(Kw::Async)),
    (SmolStr::new_inline("await"), TokenKind::Kw(Kw::Await)),
    (SmolStr::new_inline("break"), TokenKind::Kw(Kw::Break)),
    (SmolStr::new_inline("continue"), TokenKind::Kw(Kw::Continue)),
    (SmolStr::new_inline("else"), TokenKind::Kw(Kw::Else)),
    (SmolStr::new_inline("enum"), TokenKind::Kw(Kw::Enum)),
    (SmolStr::new_inline("ext"), TokenKind::Kw(Kw::Ext)),
    (SmolStr::new_inline("false"), TokenKind::Kw(Kw::False)),
    (SmolStr::new_inline("Fn"), TokenKind::Kw(Kw::FnUpper)),
    (SmolStr::new_inline("fn"), TokenKind::Kw(Kw::FnLower)),
    (SmolStr::new_inline("fun"), TokenKind::Kw(Kw::Fun)),
    (SmolStr::new_inline("for"), TokenKind::Kw(Kw::For)),
    (SmolStr::new_inline("if"), TokenKind::Kw(Kw::If)),
    (SmolStr::new_inline("imu"), TokenKind::Kw(Kw::Imu)),
    (SmolStr::new_inline("load"), TokenKind::Kw(Kw::Load)),
    (SmolStr::new_inline("loop"), TokenKind::Kw(Kw::Loop)),
    (SmolStr::new_inline("match"), TokenKind::Kw(Kw::Match)),
    (SmolStr::new_inline("me"), TokenKind::Kw(Kw::Me)),
    (SmolStr::new_inline("mut"), TokenKind::Kw(Kw::Mut)),
    (SmolStr::new_inline("pack"), TokenKind::Kw(Kw::Pack)),
    (SmolStr::new_inline("pub"), TokenKind::Kw(Kw::Pub)),
    (SmolStr::new_inline("return"), TokenKind::Kw(Kw::Return)),
    (SmolStr::new_inline("struct"), TokenKind::Kw(Kw::Struct)),
    (SmolStr::new_inline("true"), TokenKind::Kw(Kw::True)),
    (SmolStr::new_inline("type"), TokenKind::Kw(Kw::Type)),
    (SmolStr::new_inline("_"), TokenKind::Kw(Kw::Underscore)),
    (SmolStr::new_inline("val"), TokenKind::Kw(Kw::Val)),
    (SmolStr::new_inline("wasm"), TokenKind::Kw(Kw::Wasm)),
    (SmolStr::new_inline("when"), TokenKind::Kw(Kw::When)),
    (SmolStr::new_inline("while"), TokenKind::Kw(Kw::While)),
  ]);

  // reserved words for types, an error should be handled if it used as keyword.
  pub static ref TYPE: Type = HashSet::from([
    (SmolStr::new_inline("int")),
    (SmolStr::new_inline("s8")),
    (SmolStr::new_inline("s16")),
    (SmolStr::new_inline("s32")),
    (SmolStr::new_inline("s64")),
    (SmolStr::new_inline("s128")),
    (SmolStr::new_inline("u8")),
    (SmolStr::new_inline("u16")),
    (SmolStr::new_inline("u32")),
    (SmolStr::new_inline("u64")),
    (SmolStr::new_inline("u128")),
    (SmolStr::new_inline("float")),
    (SmolStr::new_inline("f32")),
    (SmolStr::new_inline("f64")),
    (SmolStr::new_inline("char")),
    (SmolStr::new_inline("str")),
  ]);
}

/// The representation of a keyword.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kw {
  Abstract,
  And,
  Apply,
  As,
  Async,
  Await,
  Break,
  Continue,
  Else,
  Enum,
  Ext,
  False,
  FnUpper,
  FnLower,
  Fun,
  For,
  If,
  Imu,
  Load,
  Loop,
  Match,
  Me,
  Mut,
  Pack,
  Pub,
  Return,
  Struct,
  True,
  Type,
  Underscore,
  Val,
  Wasm,
  When,
  While,
}

impl std::fmt::Display for Kw {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Abstract => write!(f, "abstract"),
      Self::And => write!(f, "and"),
      Self::Apply => write!(f, "apply"),
      Self::As => write!(f, "as"),
      Self::Async => write!(f, "async"),
      Self::Await => write!(f, "await"),
      Self::Break => write!(f, "break"),
      Self::Continue => write!(f, "continue"),
      Self::Else => write!(f, "else"),
      Self::Enum => write!(f, "enum"),
      Self::Ext => write!(f, "ext"),
      Self::False => write!(f, "false"),
      Self::FnUpper => write!(f, "Fn"),
      Self::FnLower => write!(f, "fn"),
      Self::Fun => write!(f, "fun"),
      Self::For => write!(f, "for"),
      Self::If => write!(f, "if"),
      Self::Imu => write!(f, "imu"),
      Self::Load => write!(f, "load"),
      Self::Loop => write!(f, "loop"),
      Self::Match => write!(f, "match"),
      Self::Me => write!(f, "me"),
      Self::Mut => write!(f, "mut"),
      Self::Pack => write!(f, "pack"),
      Self::Pub => write!(f, "pub"),
      Self::Return => write!(f, "return"),
      Self::Struct => write!(f, "struct"),
      Self::True => write!(f, "true"),
      Self::Type => write!(f, "type"),
      Self::Underscore => write!(f, "_"),
      Self::Val => write!(f, "val"),
      Self::Wasm => write!(f, "wasm"),
      Self::When => write!(f, "when"),
      Self::While => write!(f, "while"),
    }
  }
}
