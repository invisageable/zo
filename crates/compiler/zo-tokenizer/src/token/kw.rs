use super::TokenKind;

use compact_str::CompactString;
use hashbrown::HashMap;

/// The keyword dictionnary.
type Keywords = HashMap<CompactString, TokenKind>;

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

lazy_static::lazy_static! {
  // A static map of available keywords.
  pub static ref KEYWORDS: Keywords = HashMap::from([
    (CompactString::const_new("abstract"), TokenKind::Kw(Kw::Abstract)),
    (CompactString::const_new("and"), TokenKind::Kw(Kw::And)),
    (CompactString::const_new("apply"), TokenKind::Kw(Kw::Apply)),
    (CompactString::const_new("as"), TokenKind::Kw(Kw::As)),
    (CompactString::const_new("async"), TokenKind::Kw(Kw::Async)),
    (CompactString::const_new("await"), TokenKind::Kw(Kw::Await)),
    (CompactString::const_new("break"), TokenKind::Kw(Kw::Break)),
    (CompactString::const_new("continue"), TokenKind::Kw(Kw::Continue)),
    (CompactString::const_new("else"), TokenKind::Kw(Kw::Else)),
    (CompactString::const_new("enum"), TokenKind::Kw(Kw::Enum)),
    (CompactString::const_new("ext"), TokenKind::Kw(Kw::Ext)),
    (CompactString::const_new("false"), TokenKind::Kw(Kw::False)),
    (CompactString::const_new("Fn"), TokenKind::Kw(Kw::FnUpper)),
    (CompactString::const_new("fn"), TokenKind::Kw(Kw::FnLower)),
    (CompactString::const_new("fun"), TokenKind::Kw(Kw::Fun)),
    (CompactString::const_new("for"), TokenKind::Kw(Kw::For)),
    (CompactString::const_new("if"), TokenKind::Kw(Kw::If)),
    (CompactString::const_new("imu"), TokenKind::Kw(Kw::Imu)),
    (CompactString::const_new("load"), TokenKind::Kw(Kw::Load)),
    (CompactString::const_new("loop"), TokenKind::Kw(Kw::Loop)),
    (CompactString::const_new("match"), TokenKind::Kw(Kw::Match)),
    (CompactString::const_new("me"), TokenKind::Kw(Kw::Me)),
    (CompactString::const_new("mut"), TokenKind::Kw(Kw::Mut)),
    (CompactString::const_new("pack"), TokenKind::Kw(Kw::Pack)),
    (CompactString::const_new("pub"), TokenKind::Kw(Kw::Pub)),
    (CompactString::const_new("return"), TokenKind::Kw(Kw::Return)),
    (CompactString::const_new("struct"), TokenKind::Kw(Kw::Struct)),
    (CompactString::const_new("true"), TokenKind::Kw(Kw::True)),
    (CompactString::const_new("type"), TokenKind::Kw(Kw::Type)),
    (CompactString::const_new("_"), TokenKind::Kw(Kw::Underscore)),
    (CompactString::const_new("val"), TokenKind::Kw(Kw::Val)),
    (CompactString::const_new("wasm"), TokenKind::Kw(Kw::Wasm)),
    (CompactString::const_new("when"), TokenKind::Kw(Kw::When)),
    (CompactString::const_new("while"), TokenKind::Kw(Kw::While)),
  ]);
}
