// todo(ivs) — the `tokenizer` is still in work in progress. I'm revriting it to
// support a kind a templating langage.

use super::cursor::Cursor;
use super::token::int::Base;
use super::token::punctuation::Punctuation;
use super::token::{Token, TokenKind};
use crate::token::group::Group;
use crate::token::kw::KEYWORDS;

use zo_interner::interner::Interner;
use zo_reporter::reporter::Reporter;
use zo_reporter::{error, Result};
use zo_session::session::Session;

use swisskit::is;
use swisskit::span::Span;

/// The representation of the tokeinzer.
struct Tokenizer<'bytes> {
  /// A tokenizer mode.
  mode: TokenizerMode,
  /// A tokenizer mode.
  state: TokenizerState,
  /// A cursor.
  cursor: Cursor<'bytes>,
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
      mode: TokenizerMode::Program,
      state: TokenizerState::Idle,
      cursor: Cursor::new(source),
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

  /// Transform the source code into an array of tokens.
  #[inline]
  fn tokenize(self) -> Result<Vec<Token>> {
    let mut tokens = self.collect::<Vec<Token>>();

    tokens.push(Token::EOF);

    Ok(tokens)
  }

  fn switch(&mut self) {
    self.mode = match self.mode {
      TokenizerMode::Program => TokenizerMode::Template,
      TokenizerMode::Template => TokenizerMode::Program,
    };
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
    match self.mode {
      TokenizerMode::Program => match byte {
        b if is!(space b) => TokenizerState::Space,
        b if is!(number_zero b) => TokenizerState::Zero,
        b if is!(number_non_zero b) => TokenizerState::Int,
        b if is!(punctuation b) => TokenizerState::Punctuation,
        b if is!(group b) => TokenizerState::Group,
        b if is!(ident_start b) => TokenizerState::Ident,
        b if is!(quote b) => TokenizerState::Quote,
        _ => TokenizerState::Unknown,
      },
      TokenizerMode::Template => match byte {
        b if b == b'<' || b == b'>' => TokenizerState::Template,
        _ => TokenizerState::PlainText,
      },
    }
  }

  /// Eats the source code byte-by-byte.
  fn scan(&mut self) -> Option<Token> {
    let mut base = Base::Dec;
    let mut cursor_pos = self.cursor.pos();

    while self.cursor.has_bytes() {
      let byte = self.byte();

      match self.state {
        // mode — programming.
        TokenizerState::Idle => {
          self.state = self.transition(byte);

          if self.state == TokenizerState::Idle {
            self.bump();
          } else {
            cursor_pos = self.cursor.pos();
          }
        }
        TokenizerState::Space => match byte {
          b if is!(space b) => self.bump(),
          _ => self.state = TokenizerState::Idle,
        },
        TokenizerState::Comment => match byte {
          b if !is!(eol b) => self.bump(),
          _ => self.state = TokenizerState::Idle,
        },
        TokenizerState::Zero => match byte {
          b if is!(number_zero b) => self.bump(),
          b if is!(number_non_zero b) => {
            let span =
              Span::of(cursor_pos as u32, (self.cursor.pos() + 1) as u32);

            self
              .reporter
              .raise(error::lexical::invalid_number(span, byte));
          }
          b if is!(dot b) => {
            self.state = TokenizerState::Float;

            self.bump();
          }
          _ => {
            self.state = TokenizerState::Int;

            break;
          }
        },
        TokenizerState::Int => match byte {
          b if is!(number_zero b) | is!(number_non_zero b) => self.bump(),
          b if is!(dot b) => {
            self.state = TokenizerState::Float;

            self.bump();
          }
          b'b' => {
            self.state = TokenizerState::Bin;
            base = Base::Bin;

            self.bump();
          }
          b'o' => {
            self.state = TokenizerState::Oct;
            base = Base::Oct;

            self.bump();
          }
          b'x' => {
            self.state = TokenizerState::Hex;
            base = Base::Hex;

            self.bump();
          }
          _ => break,
        },
        TokenizerState::Bin => match byte {
          b if is!(number_bin b) || is!(underscore b) => self.bump(),
          _ => {
            self.state = TokenizerState::Int;

            break;
          }
        },
        TokenizerState::Oct => match byte {
          b if is!(number_oct b) || is!(underscore b) => self.bump(),
          _ => {
            self.state = TokenizerState::Int;

            break;
          }
        },
        TokenizerState::Hex => match byte {
          b if is!(number_hex b) || is!(underscore b) => self.bump(),
          _ => {
            self.state = TokenizerState::Int;

            break;
          }
        },
        TokenizerState::Float => match byte {
          b if is!(number b) || is!(underscore b) => self.bump(),
          b if b == b'e' || b == b'E' => {
            self.state = TokenizerState::ENotation;

            self.bump();
          }
          _ => break,
        },
        TokenizerState::ENotation => match byte {
          b if b == b'+' || b == b'-' || is!(number b) => self.bump(),
          _ => break,
        },
        TokenizerState::Punctuation => match byte {
          b if is!(punctuation b) => {
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
                  self.state = TokenizerState::Punctuation;

                  self.bump();
                }
                b'-' | b'!' => {
                  self.state = TokenizerState::Comment;
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
            } else if byte == b':' {
              self.bump();

              match self.byte() {
                b'=' => self.bump(),
                b':' => {
                  self.bump();

                  match self.byte() {
                    b'=' => {
                      // self.state = TokenizerState::Template;

                      self.switch();
                      self.bump();

                      break;
                    }
                    _ => break,
                  }
                }
                _ => break,
              }
            } else if byte == b'=' {
              self.bump();

              match self.byte() {
                b'=' => self.bump(),
                b'>' => {
                  self.state = TokenizerState::Punctuation;

                  self.bump();
                }
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
        TokenizerState::Group => match byte {
          b if is!(group b) => {
            self.bump();

            break;
          }
          _ => break,
        },
        TokenizerState::Ident => match byte {
          b if is!(ident_continue b) => self.bump(),
          _ => break,
        },
        TokenizerState::Quote => match byte {
          b if is!(quote_single b) => {
            self.state = TokenizerState::Char;

            self.bump();
          }
          b if is!(quote_double b) => {
            self.state = TokenizerState::Str;

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
          let span =
            Span::of(cursor_pos as u32, (self.cursor.pos() + 1) as u32);

          self.reporter.raise(error::lexical::unknown(span, byte));
        }
        // mode — template.
        TokenizerState::Template => match byte {
          b'<' => self.state = TokenizerState::TagOpen,
          _ => {
            self.state = TokenizerState::PlainText;
          }
        },
        TokenizerState::TagOpen => match byte {
          b if b.is_ascii_alphabetic() => {
            // create tag opening.
            self.state = TokenizerState::TagName;
          }
          _ => {
            self.state = TokenizerState::Unknown;
            self.bump();
          }
        },
        TokenizerState::TagName => match byte {
          b'>' => self.state = TokenizerState::Template,
          _ => {
            self.bump();
          }
        },
        TokenizerState::PlainText => match byte {
          b if b != b';' => {
            println!("char = {:?}", self.byte() as char);
            self.bump();
          }
          _ => {
            self.state = TokenizerState::PlainText;

            self.switch();

            break;
          }
        },
      }
    }

    self.scanning(Span::of(cursor_pos as u32, self.cursor.pos() as u32), base)
  }

  /// Detects the token from state and span.
  fn scanning(&mut self, span: Span, base: Base) -> Option<Token> {
    let source =
      String::from_utf8_lossy(self.bytes(span.lo as usize, span.hi as usize));

    let maybe_kind = match self.state {
      // mode — program.
      TokenizerState::Int => {
        let source = source.replace('_', "");
        let symbol = self.interner.intern(&source);

        Some(TokenKind::Int(symbol, base))
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
      TokenizerState::Ident => KEYWORDS.get::<str>(&source).map_or_else(
        || {
          let symbol = self.interner.intern(&source);

          Some(TokenKind::Ident(symbol))
        },
        |kind| Some(kind.to_owned()),
      ),
      TokenizerState::Char => {
        let symbol = self.interner.intern(&source);

        Some(TokenKind::Char(symbol))
      }
      TokenizerState::Str => {
        let symbol = self.interner.intern(&source);

        Some(TokenKind::Str(symbol))
      }
      // mode — template.
      TokenizerState::PlainText => {
        let symbol = self.interner.intern(&source);

        Some(TokenKind::PlainText(symbol))
      }
      _ => None,
    };

    self.state = TokenizerState::Idle;

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
    self.scan()
  }
}

/// The representation of a tokenizer mode.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum TokenizerMode {
  /// A mode for program.
  Program,
  /// A mode for template.
  Template,
}

/// The tokenizer follows these commands like a finite state machine.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum TokenizerState {
  /// A initial state.
  Idle,
  /// A state for whitespace.
  Space,
  /// A state for comments.
  Comment,
  /// A state for `0`.
  Zero,
  /// A integer state.
  Int,
  /// A hexadecimal state.
  Hex,
  /// A octal state.
  Oct,
  /// A binary state.
  Bin,
  /// A float state.
  Float,
  /// A [E notation](https://en.wikipedia.org/wiki/Scientific_notation#E_notation).
  ENotation,
  /// A punctuation state.
  Punctuation,
  /// A delimiter state.
  Group,
  /// A identifier state.
  Ident,
  /// A quote state.
  Quote,
  /// A character state.
  Char,
  /// A string state.
  Str,
  /// A unknown state.
  Unknown,
  /// A template state.
  Template,
  /// A plain text state.
  PlainText,
  /// A tag open state.
  TagOpen,
  /// A tag name state.
  TagName,
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
