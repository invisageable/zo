use super::cursor::Cursor;
use super::token::int::Base;
use super::token::punctuation::Punctuation;
use super::token::{Token, TokenKind};
use crate::token::group::Group;
use crate::token::kw::KEYWORD;

use zo_interner::interner::Interner;
use zo_reporter::reporter::Reporter;
use zo_reporter::{error, Result};
use zo_session::session::Session;

use swisskit::is;
use swisskit::span::Span;

/// The representation of the tokeinzer.
struct Tokenizer<'bytes> {
  /// The cursor.
  cursor: Cursor<'bytes>,
  /// The current state.
  state: TokenizerState,
  /// The integer base.
  base: Base,
  /// See [`Interner`].
  interner: &'bytes mut Interner,
  /// See [`Reporter`].
  reporter: &'bytes Reporter,
}

impl<'bytes> Tokenizer<'bytes> {
  /// Creates a new tokenizer instance.
  #[inline]
  fn new(
    interner: &'bytes mut Interner,
    reporter: &'bytes Reporter,
    source: &'bytes [u8],
  ) -> Self {
    Self {
      cursor: Cursor::new(source),
      state: TokenizerState::Start,
      base: Base::Dec,
      interner,
      reporter,
    }
  }

  /// A wrapper of [`Cursor::byte`].
  #[inline]
  fn byte(&self) -> u8 {
    self.cursor.byte()
  }

  /// A wrapper of [`Cursor::bytes`].
  pub fn bytes(&self, start: usize, end: usize) -> &'bytes [u8] {
    self.cursor.bytes(start, end)
  }

  /// A wrapper of [`Cursor::bump`].
  #[inline]
  fn bump(&mut self) {
    self.cursor.bump()
  }

  /// Gets the current state.
  #[inline]
  fn state(&self) -> TokenizerState {
    self.state
  }

  /// Transform the source code into an array of tokens.
  #[inline]
  fn tokenize(self) -> Result<Vec<Token>> {
    let tokens = self.collect();

    dbg!(&tokens);

    Ok(tokens)
  }

  /// Gets the current state from byte.
  ///
  /// The state tells to the tokenizer how to deal with the current character.
  /// Each state is related to our token classification.
  ///
  /// See also [`Token`] to get more informations.
  ///
  /// #### result.
  ///
  /// The resulting should be the current state.
  #[inline]
  fn transition(&mut self, byte: u8) -> TokenizerState {
    match byte {
      b if is!(space b) => TokenizerState::Space,
      b if is!(number_start b) => TokenizerState::Zero,
      b if is!(number_continue b) => TokenizerState::Int,
      b if is!(punctuation b) => TokenizerState::Punctuation,
      b if is!(group b) => TokenizerState::Group,
      b if is!(ident_start b) => TokenizerState::Ident,
      b if is!(quote b) => TokenizerState::Quote,
      _ => TokenizerState::Unknown,
    }
  }

  /// Eats the source code byte-by-byte.
  fn step(&mut self) -> Option<Token> {
    let mut state = self.state();
    let mut cursor_pos = self.cursor.pos();

    while self.cursor.has_bytes() {
      let byte = self.byte();

      match state {
        TokenizerState::Start => {
          state = self.transition(byte);

          dbg!(state);

          if state == TokenizerState::Start {
            self.bump();
          } else {
            cursor_pos = self.cursor.pos();
          }
        }
        TokenizerState::Space => match byte {
          b if is!(space b) => self.bump(),
          _ => state = TokenizerState::Start,
        },
        TokenizerState::Comment => match byte {
          b if !is!(eol b) => self.bump(),
          _ => state = TokenizerState::Start,
        },
        TokenizerState::Zero => match byte {
          b if is!(number_start b) => self.bump(),
          b if is!(number_continue b) => {
            let span = Span::of(cursor_pos, self.cursor.pos() + 1);

            self
              .reporter
              .raise(error::lexical::invalid_number(span, byte));
          }
          _ => {
            state = TokenizerState::Int;

            break;
          }
        },
        TokenizerState::Int => match byte {
          b if is!(number_start b) | is!(number_continue b) => self.bump(),
          b if is!(dot b) => {
            state = TokenizerState::Float;

            self.bump();
          }
          _ => break,
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
          _ => break,
        },
        TokenizerState::Punctuation => match byte {
          b'+' => {
            self.bump();

            match self.byte() {
              b'=' => self.bump(),
              _ => break,
            }
          }
          b'-' => {
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
          }
          b'*' | b'/' | b'%' | b'^' | b'!' => {
            self.bump();

            match self.byte() {
              b'=' => self.bump(),
              _ => break,
            }
          }
          _ => break,
        },
        TokenizerState::Group => match byte {
          b if is!(group b) => self.bump(),
          _ => break,
        },
        TokenizerState::Ident => match byte {
          b if is!(ident_continue b) => self.bump(),
          _ => break,
        },
        TokenizerState::Unknown => {
          let span = Span::of(cursor_pos, self.cursor.pos() + 1);

          self.reporter.raise(error::lexical::unknown(span, byte));
        }
        _ => break,
      }
    }

    self.scan(state, Span::of(cursor_pos, self.cursor.pos()))
  }

  /// Detects the token from state and span.
  fn scan(&mut self, state: TokenizerState, span: Span) -> Option<Token> {
    let source = String::from_utf8_lossy(self.bytes(span.lo, span.hi));

    let maybe_kind = match state {
      TokenizerState::Int => {
        let source = source.replace('_', "");
        let symbol = self.interner.intern(&source);

        Some(TokenKind::Int(symbol, self.base))
      }
      TokenizerState::Float | TokenizerState::ENotation => {
        let source = source.replace('_', "");
        let symbol = self.interner.intern(&source);

        Some(TokenKind::Float(symbol))
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
      TokenizerState::Group => {
        Some(TokenKind::Group(source.chars().next().map(Group::from)?))
      }
      TokenizerState::Ident => KEYWORD.get::<str>(&source).map_or_else(
        || {
          let symbol = self.interner.intern(&source);

          Some(TokenKind::Ident(symbol))
        },
        |kind| Some(*kind),
      ),
      _ => None,
    };

    if let Some(kind) = maybe_kind {
      return Some(Token::new(kind, span));
    }

    None
  }
}

impl<'bytes> Iterator for Tokenizer<'bytes> {
  type Item = Token;

  /// Moves to the next token.
  fn next(&mut self) -> Option<Self::Item> {
    self.step()
  }
}

/// The tokenizer follows these commands as a finite state machine.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum TokenizerState {
  /// The unknown state.
  Unknown,
  /// The initial state.
  Start,
  /// The state for whitespace.
  Space,
  /// The state for comments.
  Comment,
  /// The state for `0`.
  Zero,
  /// The integer state.
  Int,
  /// The hexadecimal state.
  Hex,
  /// The octal state.
  Oct,
  /// The binary state.
  Bin,
  /// The float state.
  Float,
  /// The [E notation](https://en.wikipedia.org/wiki/Scientific_notation#E_notation).
  ENotation,
  /// The punctuation state.
  Punctuation,
  /// The delimiter state.
  Group,
  /// The identifier state.
  Ident,
  /// The quote state.
  Quote,
  /// The character state.
  Char,
  /// The string state.
  Str,
}

/// Transform the source code into an array of tokens.
///
/// #### examples.
///
/// ```ignore
/// use zo_tokenizer::tokenizer;
/// use zo_session::session::Session;
///
/// let mut session = Session::default();
/// let tokens = tokenizer::tokenize(&mut session, b"");
///
/// assert_eq!(tokens, vec![]);
/// ```
pub fn tokenize(session: &mut Session, source: &[u8]) -> Result<Vec<Token>> {
  Tokenizer::new(&mut session.interner, &session.reporter, source).tokenize()
}
