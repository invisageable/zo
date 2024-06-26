//! # tokenizer.
//!
//! a simple tokenizer that used a custom finite state machine. It implements
//! the [Iterator] trait and it can only advance throw bytes one by one. For
//! the moment the tokenizer is not plugged to the parser.

use super::state::TokenizerState;
use super::token::group::Group;
use super::token::int::BaseInt;
use super::token::kw::KEYWORD;
use super::token::op::Op;
use super::token::punctuation::Punctuation;
use super::token::{Token, TokenKind};

use zo_session::session::Session;

use zo_core::interner::Interner;
use zo_core::reporter::report::lexical::Lexical;
use zo_core::reporter::report::ReportError;
use zo_core::reporter::Reporter;
use zo_core::span::Span;
use zo_core::{is, Result};

#[derive(Debug)]
pub struct Tokenizer<'source> {
  interner: &'source mut Interner,
  reporter: &'source Reporter,
  source: &'source [u8],
  index: usize,
  base_int: BaseInt,
}

impl<'source> Tokenizer<'source> {
  #[inline]
  pub fn new(
    interner: &'source mut Interner,
    reporter: &'source Reporter,
    source: &'source [u8],
  ) -> Self {
    Self {
      interner,
      reporter,
      source,
      index: 0,
      base_int: BaseInt::Dec,
    }
  }

  #[inline]
  fn byte(&self) -> u8 {
    self.source[self.index]
  }

  #[inline]
  fn bump(&mut self) {
    self.index += 1;
  }

  #[inline]
  pub fn tokenize(&mut self) -> Result<Vec<Token>> {
    Ok(self.collect())
  }

  fn state(&mut self, byte: u8) -> TokenizerState {
    match byte {
      b if is!(space b) => TokenizerState::Space,
      b if is!(number_start b) => TokenizerState::Zero,
      b if is!(number_continue b) => TokenizerState::Int,
      b if is!(ident_start b) => TokenizerState::Ident,
      b if is!(op b) => TokenizerState::Op,
      b if is!(punctuation b) => TokenizerState::Punctuation,
      b if is!(group b) => TokenizerState::Group,
      b if is!(quote b) => TokenizerState::Quote,
      _ => TokenizerState::Unknown,
    }
  }

  fn step(&mut self) -> Option<Token> {
    let mut state = TokenizerState::Start;
    let mut index_start = self.index;

    while self.index < self.source.len() {
      let byte = self.byte();

      match state {
        TokenizerState::Start => {
          state = self.state(byte);

          if state == TokenizerState::Start {
            self.bump();
          } else {
            index_start = self.index;
          }
        }
        TokenizerState::Space => match byte {
          b if is!(space b) => {
            state = TokenizerState::Start;

            self.bump();
          }
          _ => break,
        },
        TokenizerState::Comment => match byte {
          b if !is!(eol b) => self.bump(),
          _ => state = TokenizerState::Start,
        },
        TokenizerState::Zero => {
          match byte {
            b if is!(number_start b) => self.bump(),
            b if is!(number_continue b) => {
              let span = Span::of(index_start, self.index + 1);

              self.reporter.raise(ReportError::Lexical(
                Lexical::InvalidNumber(span, byte as char),
              ));
            }
            b if is!(dot b) => {
              state = TokenizerState::Float;

              self.bump();
            }
            b if b == b'x' || b == b'X' => {
              state = TokenizerState::Hex;
              self.base_int = BaseInt::Hex;

              self.bump();
            }
            b'o' => {
              state = TokenizerState::Oct;
              self.base_int = BaseInt::Oct;

              self.bump();
            }
            b'b' => {
              state = TokenizerState::Bin;
              self.base_int = BaseInt::Bin;

              self.bump();
            }
            _ => {
              state = TokenizerState::Int;

              break;
            }
          }
        }
        TokenizerState::Int => match byte {
          b if is!(number_start b) | is!(number_continue b) => self.bump(),
          b if is!(dot b) => {
            state = TokenizerState::Float;

            self.bump();
          }
          b if b == b'e' || b == b'E' => {
            state = TokenizerState::ENotation;

            self.bump();
          }
          _ => break,
        },
        TokenizerState::Hex => match byte {
          b if is!(number_hex b) || is!(underscore b) => self.bump(),
          _ => {
            state = TokenizerState::Int;

            break;
          }
        },
        TokenizerState::Oct => match byte {
          b if is!(number_oct b) || is!(underscore b) => self.bump(),
          _ => {
            state = TokenizerState::Int;

            break;
          }
        },
        TokenizerState::Bin => match byte {
          b if is!(number_bin b) || is!(underscore b) => self.bump(),
          _ => {
            state = TokenizerState::Int;

            break;
          }
        },
        TokenizerState::Float => match byte {
          b if is!(number b) || is!(underscore b) => self.bump(),
          b if b == b'e' || b == b'E' => {
            state = TokenizerState::ENotation;

            self.bump();
          }
          _ => break,
        },
        TokenizerState::ENotation => match byte {
          b if b == b'+' || b == b'-' || is!(number b) => self.bump(),
          _ => {
            // todo (ivs) — separate `int` and `float` E notation.
            state = TokenizerState::Float;

            break;
          }
        },
        TokenizerState::Ident => match byte {
          b if is!(ident_continue b) => self.bump(),
          _ => break,
        },
        TokenizerState::Op => match byte {
          b if is!(op b) => {
            if byte == b'+' {
              self.bump();

              match self.byte() {
                b'=' => self.bump(),
                _ => break,
              }
            } else if byte == b'-' {
              self.bump();

              match self.byte() {
                b'=' => self.bump(),
                b'>' => {
                  state = TokenizerState::Punctuation;

                  self.bump();
                }
                b'-' | b'!' => {
                  state = TokenizerState::Comment;
                }
                _ => break,
              }
            } else if byte == b'=' {
              self.bump();

              match self.byte() {
                b'=' => self.bump(),
                b'>' => {
                  state = TokenizerState::Punctuation;

                  self.bump();
                }
                _ => break,
              }
            } else if byte == b'*'
              || byte == b'/'
              || byte == b'%'
              || byte == b'^'
              || byte == b'!'
            {
              self.bump();

              match self.byte() {
                b'=' => self.bump(),
                _ => break,
              }
            } else if byte == b'&' {
              self.bump();

              match self.byte() {
                b'=' => self.bump(),
                b'&' => self.bump(),
                _ => break,
              }
            } else if byte == b'|' {
              self.bump();

              match self.byte() {
                b'=' => self.bump(),
                b'|' => self.bump(),
                _ => break,
              }
            } else if byte == b'<' {
              self.bump();

              match self.byte() {
                b'=' => self.bump(),
                b'<' => self.bump(),
                _ => break,
              }
            } else if byte == b'>' {
              self.bump();

              match self.byte() {
                b'=' => self.bump(),
                b'>' => self.bump(),
                _ => break,
              }
            } else if byte == b'#' {
              self.bump();

              match self.byte() {
                b'>' => self.bump(),
                _ => break,
              }
            } else {
              self.bump();

              break;
            }
          }
          _ => break,
        },
        TokenizerState::Punctuation => match byte {
          b if is!(punctuation b) => {
            if byte == b':' {
              self.bump();

              match self.byte() {
                b'=' => {
                  state = TokenizerState::Op;

                  self.bump();
                }
                b':' => self.bump(),
                _ => break,
              }
            } else {
              self.bump();

              break;
            }
          }
          _ => break,
        },
        TokenizerState::Group => match byte {
          b if is!(group b) => {
            self.bump();

            break;
          }
          _ => break,
        },
        TokenizerState::Quote => match byte {
          b if is!(quote_single b) => {
            state = TokenizerState::Char;

            self.bump();
          }
          b if is!(quote_double b) => {
            state = TokenizerState::Str;

            self.bump();
          }
          _ => break,
        },
        TokenizerState::Char => match byte {
          b if !is!(quote_single b) => self.bump(),
          _ => {
            self.bump();

            break;
          }
        },
        TokenizerState::Str => match byte {
          b if !is!(quote_double b) => self.bump(),
          _ => {
            self.bump();

            break;
          }
        },
        TokenizerState::Unknown => {
          let span = Span::of(index_start, self.index + 1);

          self
            .reporter
            .raise(ReportError::Lexical(Lexical::Unknown(span, byte as char)));
        }
      }
    }

    self.scan(state, Span::of(index_start, self.index))
  }

  fn scan(&mut self, state: TokenizerState, span: Span) -> Option<Token> {
    let source = String::from_utf8_lossy(&self.source[span.lo..span.hi]);

    let maybe_kind = match state {
      TokenizerState::Int => {
        let source = source.replace('_', "");
        let symbol = self.interner.intern(&source);

        Some(TokenKind::Int(symbol, self.base_int))
      }
      TokenizerState::Float => {
        let source = source.replace('_', "");
        let symbol = self.interner.intern(&source);

        Some(TokenKind::Float(symbol))
      }
      TokenizerState::Ident => KEYWORD.get::<str>(&source).map_or_else(
        || {
          let symbol = self.interner.intern(&source);

          Some(TokenKind::Ident(symbol))
        },
        |kind| Some(*kind),
      ),
      TokenizerState::Op => {
        if source.len() > 1 {
          Some(TokenKind::Op(Op::from(source.as_ref())))
        } else {
          Some(TokenKind::Op(source.chars().next().map(Op::from)?))
        }
      }
      TokenizerState::Group => {
        Some(TokenKind::Group(source.chars().next().map(Group::from)?))
      }
      TokenizerState::Char => {
        let symbol = self.interner.intern(&source);

        Some(TokenKind::Char(symbol))
      }
      TokenizerState::Str => {
        let symbol = self.interner.intern(&source);

        Some(TokenKind::Str(symbol))
      }
      TokenizerState::Punctuation => {
        if source.len() > 1 {
          Some(TokenKind::Punctuation(Punctuation::from(source.as_ref())))
        } else {
          Some(TokenKind::Punctuation(
            source.chars().next().map(Punctuation::from)?,
          ))
        }
      }
      _ => None,
    };

    if let Some(kind) = maybe_kind {
      return Some(Token::new(kind, span));
    }

    None
  }
}

impl<'source> Iterator for Tokenizer<'source> {
  type Item = Token;

  fn next(&mut self) -> Option<Self::Item> {
    self.step()
  }
}

/// ...
///
/// ## examples.
///
/// ```rs
/// ```
pub fn tokenize(session: &mut Session, source: &[u8]) -> Result<Vec<Token>> {
  Tokenizer::new(&mut session.interner, &session.reporter, source).tokenize()
}
