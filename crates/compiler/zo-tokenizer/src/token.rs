//! `zo` source code are splitted into the following kinds of tokens:
//!
//! * end of file.
//! * unknown characters.
//! * spaces.
//! * end of line.
//! * integers.
//! * floats.
//! * punctuations.
//! * groups.
//! * identifiers.
//! * keywords.
//! * characters.
//! * strings.

pub mod comment;
pub mod group;
pub mod int;
pub mod kw;
pub mod punctuation;
pub mod tag;
pub mod typ;

use group::Group;
use kw::Kw;
use punctuation::Punctuation;
use tag::Tag;

use zo_interner::interner::symbol::Symbol;

use swisskit::span::Span;

/// The representation of a token.
#[derive(Debug)]
pub struct Token {
  /// A token kind.
  pub kind: TokenKind,
  /// A span location within a source file.
  pub span: Span,
}

impl Token {
  /// An end of file token.
  pub const EOF: Self = Token::new(TokenKind::Eof, Span::ZERO);

  /// Creates a new token.
  #[inline(always)]
  pub const fn new(kind: TokenKind, span: Span) -> Self {
    Self { kind, span }
  }

  /// Checks if the kind of a token matched from a other token kind.
  #[inline]
  pub fn is(&self, kind: TokenKind) -> bool {
    self.kind.is(kind)
  }
}

impl Default for Token {
  /// Creates a default token — his default kind is EOF.
  #[inline(always)]
  fn default() -> Self {
    Self::EOF
  }
}

impl std::fmt::Display for Token {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.kind)
  }
}

/// The representation of a token's kind.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum TokenKind {
  // --- START:MODE:PROGRAM.
  ///
  /// An end of file token kind.
  #[default]
  Eof,
  /// A comment token kind.
  Comment(comment::Comment),
  /// A integer token kind.
  Int(Symbol, int::Base),
  /// A float token kind.
  Float(Symbol),
  /// A punctuation token kind.
  Punctuation(punctuation::Punctuation),
  /// A group token kind.
  Group(group::Group),
  /// A identifier token kind.
  Ident(Symbol),
  /// A keyword token kind.
  Kw(kw::Kw),
  /// A keyword token kind.
  Typ(String),
  /// A byte token kind.
  Byte(Symbol),
  /// A character token kind.
  Char(Symbol),
  /// A string token kind.
  Str(Symbol),

  // --- START:MODE:TEMPLATE.
  ///
  /// A zsx's comment token kind.
  ZsxComment(String),
  /// A zsx's raw text token kind.
  ZsxIdent(String),
  /// A zsx's raw text token kind.
  ZsxCharacter(char),
  /// A zsx's tag token kind.
  ZsxTag(Tag),
  /// A zsx's delimiter token kind.
  ZsxDelimiter(group::Group),
}

impl TokenKind {
  /// Checks the equality of a token kind.
  ///
  /// #### returns.
  ///
  /// The resulting returns `true` if the token kind is equal to the right-hand
  /// side.
  #[inline(always)]
  pub fn is(&self, kind: TokenKind) -> bool {
    *self == kind
  }

  /// Checks if the token kind is a literal.
  ///
  /// #### returns.
  ///
  /// The resulting returns `true` if the token kind is a literal one.
  #[inline(always)]
  pub fn is_lit(&self) -> bool {
    matches!(
      self,
      Self::Int(..)
        | Self::Float(..)
        | Self::Ident(..)
        | Self::Kw(Kw::False)
        | Self::Kw(Kw::True)
        | Self::Char(..)
        | Self::Str(..)
    )
  }

  /// Checks if the token kind is a unary operator.
  ///
  /// #### returns.
  ///
  /// The resulting returns `true` if the token kind is a unary operator.
  #[inline(always)]
  pub fn is_unop(&self) -> bool {
    matches!(
      self,
      Self::Punctuation(Punctuation::Exclamation)
        | Self::Punctuation(Punctuation::Minus)
    )
  }

  /// Checks if the token kind is a binary operator.
  ///
  /// #### returns.
  ///
  /// The resulting returns `true` if the token kind is a binary operator.
  #[inline(always)]
  pub fn is_binop(&self) -> bool {
    matches!(
      self,
      Self::Punctuation(Punctuation::Plus)
        | Self::Punctuation(Punctuation::Minus)
        | Self::Punctuation(Punctuation::Asterisk)
        | Self::Punctuation(Punctuation::Slash)
        | Self::Punctuation(Punctuation::Percent)
        | Self::Punctuation(Punctuation::Circumflex)
        | Self::Punctuation(Punctuation::EqualEqual)
        | Self::Punctuation(Punctuation::ExclamationEqual)
        | Self::Punctuation(Punctuation::LessThan)
        | Self::Punctuation(Punctuation::GreaterThan)
        | Self::Punctuation(Punctuation::LessThanEqual)
        | Self::Punctuation(Punctuation::GreaterThanEqual)
    )
  }

  /// Checks if the token kind is a sum.
  ///
  /// #### returns.
  ///
  /// The resulting returns `true` if the token kind is a sum.
  #[inline(always)]
  pub fn is_sum(&self) -> bool {
    matches!(
      self,
      Self::Punctuation(Punctuation::Plus)
        | Self::Punctuation(Punctuation::Minus)
    )
  }

  /// Checks if the token kind is a exponent.
  ///
  /// #### returns.
  ///
  /// The resulting returns `true` if the token kind is an exponent.
  #[inline(always)]
  pub fn is_expo(&self) -> bool {
    matches!(
      self,
      Self::Punctuation(Punctuation::Asterisk)
        | Self::Punctuation(Punctuation::Slash)
        | Self::Punctuation(Punctuation::Percent)
    )
  }

  /// Checks if the token kind is an assignment.
  ///
  /// #### returns.
  ///
  /// The resulting returns `true` if the token kind is an assignment operator.
  #[inline(always)]
  pub fn is_assignop(&self) -> bool {
    matches!(
      self,
      Self::Punctuation(Punctuation::Equal)
        | Self::Punctuation(Punctuation::PlusEqual)
        | Self::Punctuation(Punctuation::MinusEqual)
        | Self::Punctuation(Punctuation::AsteriskEqual)
        | Self::Punctuation(Punctuation::SlashEqual)
        | Self::Punctuation(Punctuation::PercentEqual)
        | Self::Punctuation(Punctuation::CircumflexEqual)
        | Self::Punctuation(Punctuation::AmspersandEqual)
        | Self::Punctuation(Punctuation::PipeEqual)
        | Self::Punctuation(Punctuation::LessThanLessThanEqual)
        | Self::Punctuation(Punctuation::GreaterThanGreaterThanEqual)
    )
  }

  /// Checks if the token kind is a conditional.
  ///
  /// #### returns.
  ///
  /// The resulting returns `true` if the token kind is a logical operator.
  #[inline(always)]
  pub fn is_cond(&self) -> bool {
    matches!(
      self,
      Self::Punctuation(Punctuation::AmpersandAmpersand)
        | Self::Punctuation(Punctuation::PipePipe)
    )
  }

  /// Checks if the token kind is a comparison.
  ///
  /// #### returns.
  ///
  /// The resulting returns `true` if the token kind is a comparaison operator.
  #[inline(always)]
  pub fn is_comp(&self) -> bool {
    matches!(
      self,
      Self::Punctuation(Punctuation::EqualEqual)
        | Self::Punctuation(Punctuation::ExclamationEqual)
        | Self::Punctuation(Punctuation::LessThan)
        | Self::Punctuation(Punctuation::GreaterThan)
        | Self::Punctuation(Punctuation::LessThanEqual)
        | Self::Punctuation(Punctuation::GreaterThanEqual)
    )
  }

  /// Checks if the token kind is a chaining.
  ///
  /// #### returns.
  ///
  /// The resulting returns `true` if the token kind is a chaining one.
  #[inline(always)]
  pub fn is_chaining(&self) -> bool {
    matches!(self, Self::Punctuation(Punctuation::Dot))
  }

  /// Checks if the token kind is a range.
  ///
  /// #### returns.
  ///
  /// The resulting returns `true` if the token kind is a range.
  #[inline(always)]
  pub fn is_range(&self) -> bool {
    matches!(self, Self::Punctuation(Punctuation::DotDot))
  }

  /// Checks if the token kind is a group open.
  ///
  /// #### returns.
  ///
  /// The resulting returns `true` if the token kind is a delimiter.
  #[inline(always)]
  pub fn is_group(&self) -> bool {
    matches!(
      self,
      Self::Group(Group::ParenOpen)
        | Self::Group(Group::BraceOpen)
        | Self::Group(Group::BracketOpen)
    )
  }

  /// Checks if the token kind is a call function.
  ///
  /// #### returns.
  ///
  /// The resulting returns `true` if the token kind is a function call.
  #[inline(always)]
  pub fn is_calling(&self) -> bool {
    matches!(self, Self::Group(Group::ParenOpen))
  }

  /// Checks if the token kind is an index.
  ///
  /// #### returns.
  ///
  /// The resulting returns `true` if the token kind is an index.
  #[inline(always)]
  pub fn is_index(&self) -> bool {
    matches!(self, Self::Group(Group::BracketOpen))
  }

  /// Checks if the token kind is a keyword.
  ///
  /// #### returns.
  ///
  /// The resulting returns `true` if the token kind is a keyword.
  #[inline(always)]
  pub fn is_kw(&self) -> bool {
    matches!(self, Self::Kw(..))
  }

  /// Checks if the token kind is an item.
  ///
  /// #### returns.
  ///
  /// The resulting returns `true` if the token kind is an item.
  #[inline(always)]
  pub fn is_item(&self) -> bool {
    matches!(
      self,
      Self::Kw(Kw::Load)
        | Self::Kw(Kw::Val)
        | Self::Kw(Kw::Type)
        | Self::Kw(Kw::Ext)
        | Self::Kw(Kw::Struct)
        | Self::Kw(Kw::Fun)
    )
  }

  /// Checks if the token kind is a local variable.
  ///
  /// #### returns.
  ///
  /// The resulting returns `true` if the token kind is a local variable.
  #[inline(always)]
  pub fn is_var_local(&self) -> bool {
    matches!(self, Self::Kw(Kw::Imu) | Self::Kw(Kw::Mut))
  }
}

impl std::fmt::Display for TokenKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      // --- MODE:PROGRAM:START. ---
      Self::Eof => write!(f, "eof"),
      Self::Comment(comment::Comment::Line) => write!(f, ""),
      Self::Comment(comment::Comment::LineDoc(comment)) => {
        write!(f, "{comment}")
      }
      Self::Int(sym, base) => {
        write!(f, "{sym}--base--{base:?}")
      }
      Self::Float(sym) => write!(f, "{sym}"),
      Self::Punctuation(punctuation) => {
        write!(f, "{punctuation}")
      }
      Self::Group(group) => write!(f, "{group}"),
      Self::Ident(sym) => write!(f, "{sym}"),
      Self::Kw(kw) => write!(f, "{kw}"),
      Self::Typ(typ) => write!(f, "{typ}"),
      Self::Byte(b) => write!(f, "{b}"),
      Self::Char(c) => write!(f, "{c}"),
      Self::Str(s) => write!(f, "{s}"),
      // --- MODE:PROGRAM:END. ---

      // --- MODE:TEMPLATE:START. ---
      Self::ZsxComment(comment) => write!(f, "{comment}"),
      Self::ZsxIdent(ident) => write!(f, "{ident}"),
      Self::ZsxCharacter(char) => write!(f, "{char}"),
      Self::ZsxTag(tag) => write!(f, "{tag}"),
      Self::ZsxDelimiter(delimiter) => write!(f, "{delimiter}"),
      // --- MODE:TEMPLATE:END. ---
    }
  }
}
