mod comment;
mod group;
mod int;
mod kw;
mod punctuation;
mod tag;

use swisskit::span::Span;

#[derive(Debug)]
pub struct Token {
  pub kind: TokenKind,
  pub span: Span,
}

#[derive(Debug)]
pub enum TokenKind {
  /// A end of file token kind.
  Eof,
}
