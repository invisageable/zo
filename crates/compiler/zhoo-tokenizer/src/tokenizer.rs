use super::state::TokenizerState;
use super::token::group::Group;
use super::token::kw::KEYWORD;
use super::token::op::Op;
use super::token::punctuation::Punctuation;
use super::token::{Token, TokenKind};

use zhoo_session::session::Session;

use zo_core::interner::Interner;
use zo_core::is;
use zo_core::reporter::report::lexical::Lexical;
use zo_core::reporter::report::ReportError;
use zo_core::reporter::Reporter;
use zo_core::span::Span;
use zo_core::Result;

#[derive(Debug)]
struct Tokenizer<'source> {
  interner: &'source mut Interner,
  reporter: &'source Reporter,
  source: &'source [u8],
  index: usize,
}

impl<'source> Tokenizer<'source> {
  #[inline]
  fn new(
    interner: &'source mut Interner,
    reporter: &'source Reporter,
    source: &'source [u8],
  ) -> Self {
    Self {
      interner,
      reporter,
      source,
      index: 0usize,
    }
  }

  fn byte(&self) -> u8 {
    self.source[self.index]
  }

  fn bump(&mut self) {
    self.index += 1;
  }

  fn tokenize(&mut self) -> Result<Vec<Token>> {
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
      b if is!(eof b) => TokenizerState::End,
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
        TokenizerState::Space => {
          println!("SPACE: {}", byte as char);
        }
        TokenizerState::Comment => {
          println!("COMMENT: {}", byte as char);
        }
        TokenizerState::Zero => match byte {
          b if is!(number_start b) => {
            println!("ZERO: {}", byte as char);

            state = TokenizerState::Zero;

            self.bump();
          }
          b if b == b'x' || b == b'X' => {
            println!("IS:HEX: {}", byte as char);
            state = TokenizerState::Hex;

            self.bump();
          }
          _ => break,
        },
        TokenizerState::Hex => match byte {
          b if is!(number_hex b) => {
            println!("HEX: {}", byte as char);
            self.bump();
          }
          _ => break,
        },
        TokenizerState::Int => {
          println!("INT: {}", byte as char);
          self.bump();
        }
        TokenizerState::Float => {
          println!("FLOAT: {}", byte as char);
        }
        TokenizerState::Ident => {
          println!("IDENT: {}", byte as char);
          self.bump();
        }
        TokenizerState::Op => {
          println!("OP: {}", byte as char);
        }
        TokenizerState::Punctuation => {
          println!("PUNCTUATION: {}", byte as char);
        }
        TokenizerState::Group => {
          println!("GROUP: {}", byte as char);
        }
        TokenizerState::Unknown => {
          println!("UNKNOWN: {}", byte as char);

          let span = Span::of(index_start, self.index + 1);

          self
            .reporter
            .raise(ReportError::Lexical(Lexical::Unknown(span, byte as char)));
        }
        TokenizerState::End => {
          println!("EOF: {}", byte as char);

          break;
        }
      }
    }

    let span = Span::of(index_start, self.index);

    self.scan(state, span)
  }

  fn scan(&mut self, state: TokenizerState, span: Span) -> Option<Token> {
    let source = String::from_utf8_lossy(&self.source[span.lo..span.hi]);

    let maybe_kind = match state {
      TokenizerState::Int => {
        let source = source.replace('_', "");
        let symbol = self.interner.intern(&source);

        Some(TokenKind::Int(symbol))
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

/// ## arguments.
///
/// - `session` — A mutable reference to the [`Session`].
/// - `source` — A sequence of bytes to be tokenized.
///
/// ## returns.
///
/// A [`Result`] containing a vector of tokens or an error.
///
/// ```
/// ```
pub fn tokenize(session: &mut Session, source: &[u8]) -> Result<Vec<Token>> {
  println!("tokenize.");
  Tokenizer::new(&mut session.interner, &session.reporter, source).tokenize()
}
