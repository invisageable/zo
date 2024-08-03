use super::TokenKind;

use hashbrown::{HashMap, HashSet};
use smol_str::SmolStr;

type Keyword = HashMap<SmolStr, TokenKind>;
type Type = HashSet<SmolStr>;

lazy_static::lazy_static! {
  // A static map of available keywords.
  pub static ref KEYWORD: Keyword = {
    let mut kw: Keyword = HashMap::with_capacity(0usize);

    kw.insert(SmolStr::new_inline("abstract"), TokenKind::Kw(Kw::Abstract));
    kw.insert(SmolStr::new_inline("apply"), TokenKind::Kw(Kw::Apply));
    kw.insert(SmolStr::new_inline("async"), TokenKind::Kw(Kw::Async));
    kw.insert(SmolStr::new_inline("await"), TokenKind::Kw(Kw::Await));
    kw.insert(SmolStr::new_inline("break"), TokenKind::Kw(Kw::Break));
    kw.insert(SmolStr::new_inline("continue"), TokenKind::Kw(Kw::Continue));
    kw.insert(SmolStr::new_inline("else"), TokenKind::Kw(Kw::Else));
    kw.insert(SmolStr::new_inline("enum"), TokenKind::Kw(Kw::Enum));
    kw.insert(SmolStr::new_inline("ext"), TokenKind::Kw(Kw::Ext));
    kw.insert(SmolStr::new_inline("false"), TokenKind::Kw(Kw::False));
    kw.insert(SmolStr::new_inline("fn"), TokenKind::Kw(Kw::Fn));
    kw.insert(SmolStr::new_inline("fun"), TokenKind::Kw(Kw::Fun));
    kw.insert(SmolStr::new_inline("for"), TokenKind::Kw(Kw::For));
    kw.insert(SmolStr::new_inline("if"), TokenKind::Kw(Kw::If));
    kw.insert(SmolStr::new_inline("imu"), TokenKind::Kw(Kw::Imu));
    kw.insert(SmolStr::new_inline("load"), TokenKind::Kw(Kw::Load));
    kw.insert(SmolStr::new_inline("loop"), TokenKind::Kw(Kw::Loop));
    kw.insert(SmolStr::new_inline("match"), TokenKind::Kw(Kw::Match));
    kw.insert(SmolStr::new_inline("me"), TokenKind::Kw(Kw::Me));
    kw.insert(SmolStr::new_inline("mut"), TokenKind::Kw(Kw::Mut));
    kw.insert(SmolStr::new_inline("pack"), TokenKind::Kw(Kw::Pack));
    kw.insert(SmolStr::new_inline("pub"), TokenKind::Kw(Kw::Pub));
    kw.insert(SmolStr::new_inline("return"), TokenKind::Kw(Kw::Return));
    kw.insert(SmolStr::new_inline("struct"), TokenKind::Kw(Kw::Struct));
    kw.insert(SmolStr::new_inline("true"), TokenKind::Kw(Kw::True));
    kw.insert(SmolStr::new_inline("type"), TokenKind::Kw(Kw::Type));
    kw.insert(SmolStr::new_inline("val"), TokenKind::Kw(Kw::Val));
    kw.insert(SmolStr::new_inline("wasm"), TokenKind::Kw(Kw::Wasm));
    kw.insert(SmolStr::new_inline("when"), TokenKind::Kw(Kw::When));
    kw.insert(SmolStr::new_inline("while"), TokenKind::Kw(Kw::While));

    kw
  };
  // reserved words for types, an error should be handled if it used as keyword.
  pub static ref TYPE: Type = {
    let mut ty: Type = HashSet::with_capacity(0usize);

    ty.insert(SmolStr::new_inline("int"));
    ty.insert(SmolStr::new_inline("float"));
    ty.insert(SmolStr::new_inline("char"));
    ty.insert(SmolStr::new_inline("str"));

    ty
  };
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kw {
  Abstract,
  Apply,
  Async,
  Await,
  Break,
  Continue,
  Else,
  Enum,
  Ext,
  False,
  Fn,
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
  Val,
  Wasm,
  When,
  While,
}

impl std::fmt::Display for Kw {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Abstract => write!(f, "abstract"),
      Self::Apply => write!(f, "apply"),
      Self::Async => write!(f, "async"),
      Self::Await => write!(f, "await"),
      Self::Break => write!(f, "break"),
      Self::Continue => write!(f, "continue"),
      Self::Else => write!(f, "else"),
      Self::Enum => write!(f, "enum"),
      Self::Ext => write!(f, "ext"),
      Self::False => write!(f, "false"),
      Self::Fn => write!(f, "fn"),
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
      Self::Val => write!(f, "val"),
      Self::Wasm => write!(f, "wasm"),
      Self::When => write!(f, "when"),
      Self::While => write!(f, "while"),
    }
  }
}
