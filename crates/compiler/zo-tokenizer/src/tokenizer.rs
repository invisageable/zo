//! # tokenizer.
//!
//! a simple tokenizer that used a custom finite state machine. It implements
//! the [Iterator] trait and it can only advance throw bytes one by one. For
//! the moment the tokenizer is not plugged to the parser.

use super::state::TokenizerState;
use super::token::int::BaseInt;
use super::token::{Token, TokenKind};

use zo_session::session::Session;

use zo_core::interner::Interner;
use zo_core::reporter::report::lexical::Lexical;
use zo_core::reporter::report::ReportError;
use zo_core::reporter::Reporter;
use zo_core::span::Span;
use zo_core::{is, Result};

struct Tokenizer<'source> {
  interner: &'source mut Interner,
  reporter: &'source Reporter,
  source: &'source [u8],
  index: usize,
  base_int: BaseInt,
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
        TokenizerState::Unknown => {
          let span = Span::of(index_start, self.index + 1);

          self
            .reporter
            .raise(ReportError::Lexical(Lexical::Unknown(span, byte as char)));
        }
        _ => todo!(),
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
