use super::TokenKind;

use german_str::GermanStr;
use hashbrown::HashMap;

/// The keyword dictionnary.
type Keywords = HashMap<GermanStr, TokenKind>;

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
    (GermanStr::new_inline("abstract"), TokenKind::Kw(Kw::Abstract)),
    (GermanStr::new_inline("and"), TokenKind::Kw(Kw::And)),
    (GermanStr::new_inline("apply"), TokenKind::Kw(Kw::Apply)),
    (GermanStr::new_inline("as"), TokenKind::Kw(Kw::As)),
    (GermanStr::new_inline("async"), TokenKind::Kw(Kw::Async)),
    (GermanStr::new_inline("await"), TokenKind::Kw(Kw::Await)),
    (GermanStr::new_inline("break"), TokenKind::Kw(Kw::Break)),
    (GermanStr::new_inline("continue"), TokenKind::Kw(Kw::Continue)),
    (GermanStr::new_inline("else"), TokenKind::Kw(Kw::Else)),
    (GermanStr::new_inline("enum"), TokenKind::Kw(Kw::Enum)),
    (GermanStr::new_inline("ext"), TokenKind::Kw(Kw::Ext)),
    (GermanStr::new_inline("false"), TokenKind::Kw(Kw::False)),
    (GermanStr::new_inline("Fn"), TokenKind::Kw(Kw::FnUpper)),
    (GermanStr::new_inline("fn"), TokenKind::Kw(Kw::FnLower)),
    (GermanStr::new_inline("fun"), TokenKind::Kw(Kw::Fun)),
    (GermanStr::new_inline("for"), TokenKind::Kw(Kw::For)),
    (GermanStr::new_inline("if"), TokenKind::Kw(Kw::If)),
    (GermanStr::new_inline("imu"), TokenKind::Kw(Kw::Imu)),
    (GermanStr::new_inline("load"), TokenKind::Kw(Kw::Load)),
    (GermanStr::new_inline("loop"), TokenKind::Kw(Kw::Loop)),
    (GermanStr::new_inline("match"), TokenKind::Kw(Kw::Match)),
    (GermanStr::new_inline("me"), TokenKind::Kw(Kw::Me)),
    (GermanStr::new_inline("mut"), TokenKind::Kw(Kw::Mut)),
    (GermanStr::new_inline("pack"), TokenKind::Kw(Kw::Pack)),
    (GermanStr::new_inline("pub"), TokenKind::Kw(Kw::Pub)),
    (GermanStr::new_inline("return"), TokenKind::Kw(Kw::Return)),
    (GermanStr::new_inline("struct"), TokenKind::Kw(Kw::Struct)),
    (GermanStr::new_inline("true"), TokenKind::Kw(Kw::True)),
    (GermanStr::new_inline("type"), TokenKind::Kw(Kw::Type)),
    (GermanStr::new_inline("_"), TokenKind::Kw(Kw::Underscore)),
    (GermanStr::new_inline("val"), TokenKind::Kw(Kw::Val)),
    (GermanStr::new_inline("wasm"), TokenKind::Kw(Kw::Wasm)),
    (GermanStr::new_inline("when"), TokenKind::Kw(Kw::When)),
    (GermanStr::new_inline("while"), TokenKind::Kw(Kw::While)),
  ]);
}
