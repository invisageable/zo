use super::TokenKind;

use hashbrown::{HashMap, HashSet};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

type Keyword = HashMap<SmolStr, TokenKind>;
type Type = HashSet<SmolStr>;

lazy_static::lazy_static! {
  pub static ref KEYWORD: Keyword = {
    let mut kw: Keyword = HashMap::new();

    kw.insert(SmolStr::new_inline("abstract"), TokenKind::Kw(Kw::Abstract));
    kw.insert(SmolStr::new_inline("apply"), TokenKind::Kw(Kw::Apply));
    kw.insert(SmolStr::new_inline("async"), TokenKind::Kw(Kw::Async));
    kw.insert(SmolStr::new_inline("await"), TokenKind::Kw(Kw::Await));
    kw.insert(SmolStr::new_inline("break"), TokenKind::Kw(Kw::Break));
    kw.insert(SmolStr::new_inline("continue"), TokenKind::Kw(Kw::Continue));
    kw.insert(SmolStr::new_inline("else"), TokenKind::Kw(Kw::Else));
    kw.insert(SmolStr::new_inline("enum"), TokenKind::Kw(Kw::Enum));
    kw.insert(SmolStr::new_inline("ext"), TokenKind::Kw(Kw::Ext));
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
    kw.insert(SmolStr::new_inline("return"), TokenKind::Kw(Kw::Return));
    kw.insert(SmolStr::new_inline("struct"), TokenKind::Kw(Kw::Struct));
    kw.insert(SmolStr::new_inline("type"), TokenKind::Kw(Kw::Type));
    kw.insert(SmolStr::new_inline("val"), TokenKind::Kw(Kw::Val));
    kw.insert(SmolStr::new_inline("wasm"), TokenKind::Kw(Kw::Wasm));
    kw.insert(SmolStr::new_inline("when"), TokenKind::Kw(Kw::When));
    kw.insert(SmolStr::new_inline("while"), TokenKind::Kw(Kw::While));

    kw
  };
  pub static ref TYPE: Type = {
    let mut kwf: Type = HashSet::new();

    kwf.insert(SmolStr::new_inline("int"));
    kwf.insert(SmolStr::new_inline("float"));
    kwf.insert(SmolStr::new_inline("char"));
    kwf.insert(SmolStr::new_inline("str"));

    kwf
  };
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
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
      Self::Type => write!(f, "type"),
      Self::Val => write!(f, "val"),
      Self::Wasm => write!(f, "wasm"),
      Self::When => write!(f, "when"),
      Self::While => write!(f, "while"),
    }
  }
}
