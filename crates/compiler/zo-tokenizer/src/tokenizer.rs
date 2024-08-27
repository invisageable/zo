use super::cursor::Cursor;
use super::state::{Expo, In, Num, Quoted, TokenizerState};
use super::token::comment::Comment;
use super::token::group::Group;
use super::token::int::Base;
use super::token::kw::KEYWORDS;
use super::token::punctuation::Punctuation;
use super::token::tag::{Attr, Custom, Name, Tag, TagKind};
use super::token::typ::TYPES;
use super::token::{Token, TokenKind};

use zo_interner::interner::Interner;
use zo_reporter::reporter::Reporter;
use zo_reporter::{error, Result};
use zo_session::session::Session;

use swisskit::span::Span;
use swisskit::{is, to};

use thin_vec::ThinVec;

use std::borrow::Borrow;

/// The representation of a Tokenizer.
struct Tokenizer<'source> {
  /// A tokenizer's mode.
  mode: std::cell::Cell<TokenizerMode>,
  /// A tokenizer's state.
  state: std::cell::Cell<TokenizerState>,
  /// A current character.
  char_current: std::cell::Cell<char>,
  /// A flag to check if we can reconsume the current character.
  reconsume: std::cell::Cell<bool>,
  /// An integer base.
  int_base: std::cell::Cell<Base>,
  /// An exponent significand.
  expo_sign: std::cell::Cell<i8>,
  /// A current tag kind.
  tag_current_kind: std::cell::Cell<TagKind>,
  /// A current name tag.
  tag_current_name: std::cell::RefCell<String>,
  /// A flag to checks if the current tag is a self closing tag.
  tag_current_self_closing: std::cell::Cell<bool>,
  /// A flag to checks if the current tag is a fragment tag.
  tag_current_frag: std::cell::Cell<bool>,
  /// A current name tag.
  tag_current_attrs: std::cell::RefCell<ThinVec<Attr>>,
  /// A current attribute name.
  attr_current_name: std::cell::RefCell<String>,
  /// A current attribute value.
  attr_current_value: std::cell::RefCell<String>,
  /// A current comment.
  comment_current: std::cell::RefCell<String>,
  /// A last start tag name, for use in checking "appropriate end tag".
  last_start_tag_name: std::cell::RefCell<Option<Name>>,
  /// A temporary buffer to track progress.
  temp_buf: std::cell::RefCell<String>,
  /// A track current line.
  line_current: std::cell::Cell<u64>,
  /// A cursor.  
  cursor: Cursor<'source>,
  /// See [`Interner`].
  interner: &'source mut Interner,
  /// See [`Reporter`].
  reporter: &'source Reporter,
  styling: std::cell::Cell<bool>,
  cascading: std::cell::Cell<bool>,
}

impl<'source> Tokenizer<'source> {
  /// Creates a new tokenizer.
  #[inline(always)]
  fn new(
    interner: &'source mut Interner,
    reporter: &'source Reporter,
    source: &'source str,
  ) -> Self {
    Self {
      mode: std::cell::Cell::new(TokenizerMode::Program),
      state: std::cell::Cell::new(TokenizerState::Program),
      char_current: std::cell::Cell::new('\0'),
      reconsume: std::cell::Cell::new(false),
      int_base: std::cell::Cell::new(Base::Dec),
      expo_sign: std::cell::Cell::new(1i8),
      tag_current_kind: std::cell::Cell::new(TagKind::Opening),
      tag_current_name: std::cell::RefCell::new(String::new()),
      tag_current_self_closing: std::cell::Cell::new(false),
      tag_current_frag: std::cell::Cell::new(false),
      tag_current_attrs: std::cell::RefCell::new(ThinVec::with_capacity(
        0usize,
      )),
      attr_current_name: std::cell::RefCell::new(String::new()),
      attr_current_value: std::cell::RefCell::new(String::new()),
      comment_current: std::cell::RefCell::new(String::new()),
      last_start_tag_name: std::cell::RefCell::new(Some(Name::Custom(
        Custom::Name(String::with_capacity(0usize)),
      ))),
      temp_buf: std::cell::RefCell::new(String::with_capacity(0usize)),
      line_current: std::cell::Cell::new(1u64),
      cursor: Cursor::new(source),
      interner,
      reporter,
      styling: std::cell::Cell::new(false),
      cascading: std::cell::Cell::new(false),
    }
  }

  /// Tokenizes a source code into a stream of tokens.
  fn tokenize(self) -> Result<Vec<Token>> {
    let len = self.cursor.source().len();
    let mut tokens = self.collect::<Vec<Token>>();

    tokens.push(Token::new(TokenKind::Eof, Span::of(len, len + 1)));

    // println!("{tokens:?}");

    Ok(tokens)
  }

  /// Toggles the tokenizer mode.
  #[inline]
  fn switch(&mut self) {
    self.mode.set(match self.mode.get() {
      TokenizerMode::Program => TokenizerMode::Template,
      TokenizerMode::Template => TokenizerMode::Program,
    });
  }

  /// Resets everything.
  #[inline]
  fn reset(&self) {
    self.reset_tag();
    self.reset_state();
  }

  // Resets the state to start to an initial state depending of the mode.
  #[inline]
  fn reset_state(&self) {
    self.state.set(match self.mode.get() {
      TokenizerMode::Program => TokenizerState::Program,
      TokenizerMode::Template => TokenizerState::ZsxData,
    });
  }

  /// Gets the next character.
  #[inline]
  fn get_char(&mut self) -> Option<char> {
    if self.reconsume.get() {
      self.reconsume.set(false);

      Some(self.char_current.get())
    } else {
      self
        .cursor
        .peek()
        .and_then(|ch| self.get_preprocessed_char(ch))
    }
  }

  /// Gets the next character after preprocessing.
  #[inline(always)]
  fn get_preprocessed_char(&self, ch: char) -> Option<char> {
    if ch == '\n' {
      self.line_current.set(self.line_current.get() + 1u64);
    }

    self.char_current.set(ch);
    Some(ch)
  }

  /// Creates a tag.
  fn create_tag(&mut self, kind: TagKind, ch: char) {
    self.reset_tag();
    self.tag_current_name.borrow_mut().push(ch);
    self.tag_current_kind.set(kind);
  }

  /// Resets a tag.
  #[inline]
  fn reset_tag(&self) {
    self.tag_current_name.borrow_mut().clear();
    self.tag_current_self_closing.set(false);
    *self.tag_current_attrs.borrow_mut() = ThinVec::with_capacity(0usize);
  }

  /// Fills a tag name.
  #[inline]
  fn append_to_tag_name(&mut self, ch: char) {
    self.tag_current_name.borrow_mut().push(ch);
  }

  /// Creates an attribute.
  #[inline]
  fn create_attribute(&mut self, ch: char) {
    self.finish_attribute();

    self.attr_current_name.borrow_mut().push(ch);
  }

  /// Finishes to create the current attribute.
  fn finish_attribute(&mut self) {
    if self.attr_current_name.borrow().len() == 0usize {
      return;
    }

    let dup = {
      let attr_name = self.attr_current_name.borrow();

      self
        .tag_current_attrs
        .borrow()
        .iter()
        .any(|attr| match attr {
          Attr::Static(name, _) | Attr::Dynamic(name, _) => {
            attr_name.as_bytes() == name.as_bytes()
          }
        })
    };

    if dup {
      eprintln!("Parse error: duplicate attribute");
      self.attr_current_name.borrow_mut().clear();
      self.attr_current_value.borrow_mut().clear();
    } else {
      let name = self.attr_current_name.borrow();

      self.attr_current_name.borrow_mut().clear();

      self.tag_current_attrs.borrow_mut().push(Attr::Static(
        name.to_owned(),
        Some(std::mem::take(&mut self.attr_current_value.borrow_mut())),
      ));
    }
  }
}

// macro_rules! unwrap_or_else {
//   ($opt:expr, $else_block:block) => {{
//     let Some(x) = $opt else { $else_block };
//     x
//   }};
// }

// macro_rules! unwrap_or_return {
//   ($opt:expr, $retval:expr) => {
//     unwrap_or_else!($opt, { return $retval })
//   };
// }

macro_rules! get_char ( ($me:expr) => (
  $me.get_char()?
));

impl<'source> Tokenizer<'source> {
  /// Consomes the current character.
  pub fn step(&mut self) -> Option<Token> {
    let mut pos = self.cursor.pos();

    while let Some(ch) = self.get_char() {
      // println!("STEP => State = {:?} | Char = {ch:?}", self.state.get());

      match self.mode.get() {
        //# --- MODE:PROGRAM:START. ---
        TokenizerMode::Program => match self.state.get() {
          //# program-state.
          TokenizerState::Program => {
            pos = self.cursor.pos();

            match ch {
              c if is!(space c) => {
                self.cursor.next();
              }
              c if is!(number_zero c) => {
                self.state.set(TokenizerState::Num(Num::Zero))
              }
              c if is!(number_non_zero c) => {
                self.state.set(TokenizerState::Num(Num::Int))
              }

              c if is!(group c) => self.state.set(TokenizerState::Group),
              c if is!(punctuation c) => {
                self.state.set(TokenizerState::Punctuation)
              }
              c if is!(ident_start c) => self.state.set(TokenizerState::Ident),
              c if is!(quote c) => self.state.set(TokenizerState::Quote),
              '$' => {
                self.cursor.next();
                self.state.set(TokenizerState::ZssStart);
              }
              _ => self.state.set(TokenizerState::Unknown),
            }
          }

          //# style-start-state.
          TokenizerState::ZssStart => match ch {
            ':' => {
              self.styling.set(true);
              self.state.set(TokenizerState::Zss);
            }
            _ => {
              panic!("wrong following symbol")
            }
          },

          //# style-state.
          TokenizerState::Zss => match ch {
            ';' if !self.cascading.get() => {
              self.styling.set(false);
              self.cursor.next();
              self.state.set(TokenizerState::ZssEnd)
            }
            '{' => {
              self.cascading.set(true);
              self.cursor.next();
            }
            '}' => {
              self.cascading.set(false);
              self.cursor.next();
            }
            _ => {
              self.cursor.next();
            }
          },

          //# style-end-state.
          TokenizerState::ZssEnd => match ch {
            c if c == '\n' => {
              self.cursor.next();
              self.state.set(TokenizerState::Zss);

              return self.scan(pos);
            }
            c => panic!("should have a newline: {c:?}"),
          },

          //# comment-line-state.
          TokenizerState::CommentLine => match ch {
            c if !is!(eol c) => {
              self.cursor.next();
            }
            _ => {
              self.cursor.next();
              self.state.set(TokenizerState::Program);
              // self.state.set(TokenizerState::CommentLine);

              // return self.scan(pos);
            }
          },

          //# comment-line-doc-state.
          TokenizerState::CommentLineDoc => match ch {
            c if !is!(eol c) => {
              self.cursor.next();
            }
            _ => {
              self.cursor.next();
              self.state.set(TokenizerState::Program);
              // self.state.set(TokenizerState::CommentLineDoc);

              // return self.scan(pos);
            }
          },

          //# num-zero-state.
          TokenizerState::Num(Num::Zero) => match ch {
            c if is!(dot c) => {
              self.cursor.next();
              self.state.set(TokenizerState::Num(Num::DecPoint));
            }
            _ => self.state.set(TokenizerState::Num(Num::Int)),
          },

          //# num-int-state.
          TokenizerState::Num(Num::Int) => match ch {
            c if is!(number c) || is!(underscore c) => {
              self.cursor.next();
            }
            'b' => {
              self.int_base.set(Base::Bin);
              self.cursor.next();
              self.state.set(TokenizerState::Num(Num::Bin));
            }
            'o' => {
              self.int_base.set(Base::Oct);
              self.cursor.next();
              self.state.set(TokenizerState::Num(Num::Oct));
            }
            'x' => {
              self.int_base.set(Base::Hex);
              self.cursor.next();
              self.state.set(TokenizerState::Num(Num::Hex));
            }
            '#' => {
              self.cursor.next();
            }
            c if is!(dot c) => {
              self.cursor.next();
              self.state.set(TokenizerState::Num(Num::DecPoint));
            }
            c if c == 'e' || c == 'E' => {
              self.cursor.next();
              self.state.set(TokenizerState::Num(Num::Expo(Expo::E)));
            }
            _ => return self.scan(pos),
          },

          //# num-bin-state.
          TokenizerState::Num(Num::Bin) => match ch {
            c if is!(number_bin c) || is!(underscore c) => {
              self.cursor.next();
            }
            _ => {
              self.state.set(TokenizerState::Num(Num::Int));

              return self.scan(pos);
            }
          },

          //# num-oct-state.
          TokenizerState::Num(Num::Oct) => match ch {
            c if is!(number_oct c) || is!(underscore c) => {
              self.cursor.next();
            }
            _ => {
              self.state.set(TokenizerState::Num(Num::Int));

              return self.scan(pos);
            }
          },

          //# num-hex-state.
          TokenizerState::Num(Num::Hex) => match ch {
            c if is!(number_hex c) || is!(underscore c) => {
              self.cursor.next();
            }
            _ => {
              self.state.set(TokenizerState::Num(Num::Int));

              return self.scan(pos);
            }
          },

          //# num-dec-point-state.
          TokenizerState::Num(Num::DecPoint) => match ch {
            c if is!(number c) => {
              self.cursor.next();
              self.state.set(TokenizerState::Num(Num::Float));
            }
            _ => {
              // todo(ivs) — add a report error message.
              // moves the cursor to the next character.
              // sets the state to Unknown.
            }
          },

          //# num-float-state.
          TokenizerState::Num(Num::Float) => match ch {
            c if is!(number c) || is!(underscore c) => {
              self.cursor.next();
            }
            c if c == 'e' || c == 'E' => {
              self.cursor.next();
              self.state.set(TokenizerState::Num(Num::Expo(Expo::E)));
            }
            _ => return self.scan(pos),
          },

          //# num-expo-e-state.
          TokenizerState::Num(Num::Expo(Expo::E)) => match ch {
            '+' => {
              self.expo_sign.set(1i8);
              self.cursor.next();
              self.state.set(TokenizerState::Num(Num::Expo(Expo::Sign)));
            }
            '-' => {
              self.expo_sign.set(-1i8);
              self.cursor.next();
              self.state.set(TokenizerState::Num(Num::Expo(Expo::Sign)));
            }
            c if is!(number c) => {
              self.cursor.next();
              self.state.set(TokenizerState::Num(Num::Expo(Expo::Digits)));
            }
            _ => {
              // todo(ivs) — add a report error message.
              // moves the cursor to the next character.
              // sets the state to Unknown.
            }
          },

          //# num-expo-sign-state.
          TokenizerState::Num(Num::Expo(Expo::Sign)) => match ch {
            c if is!(number c) => {
              self.cursor.next();
              self.state.set(TokenizerState::Num(Num::Expo(Expo::Digits)));
            }
            _ => {
              // todo(ivs) — add a report error message.
              // moves the cursor to the next character.
              // sets the state to Unknown.
            }
          },

          //# num-expo-digits-state.
          TokenizerState::Num(Num::Expo(Expo::Digits)) => match ch {
            c if is!(number c) => {
              self.cursor.next();
              self.state.set(TokenizerState::Num(Num::Expo(Expo::Digits)));
            }
            _ => {
              self.state.set(TokenizerState::Num(Num::Float));

              return self.scan(pos);
            }
          },

          //# punctuation-state.
          TokenizerState::Punctuation => match ch {
            c if is!(punctuation c) => {
              if ch == '*'
                || ch == '/'
                || ch == '%'
                || ch == '^'
                || ch == '!'
                || ch == '+'
              {
                self.cursor.next();

                match get_char!(self) {
                  '=' => {
                    self.cursor.next();
                  }
                  _ => return self.scan(pos),
                }
              } else if ch == '-' {
                self.cursor.next();

                match get_char!(self) {
                  '=' | '>' => {
                    self.cursor.next();
                  }
                  '-' => self.state.set(TokenizerState::CommentLine),
                  '!' => self.state.set(TokenizerState::CommentLineDoc),
                  _ => return self.scan(pos),
                }
              } else if ch == ':' {
                self.cursor.next();

                match get_char!(self) {
                  '=' => {
                    self.cursor.next();
                  }
                  ':' => {
                    self.cursor.next();

                    match get_char!(self) {
                      '=' => {
                        self.switch();
                        self.cursor.next();

                        return self.scan(pos);
                      }
                      _ => return self.scan(pos),
                    }
                  }
                  _ => return self.scan(pos),
                }
              } else if ch == '=' {
                self.cursor.next();

                match get_char!(self) {
                  '=' | '>' => {
                    self.cursor.next();
                  }
                  _ => return self.scan(pos),
                }
              } else if ch == '&' {
                self.cursor.next();

                match get_char!(self) {
                  '=' | '&' => {
                    self.cursor.next();
                  }
                  _ => return self.scan(pos),
                }
              } else if ch == '|' {
                self.cursor.next();

                match get_char!(self) {
                  '=' | '|' => {
                    self.cursor.next();
                  }
                  _ => return self.scan(pos),
                }
              } else if ch == '<' {
                self.cursor.next();

                match get_char!(self) {
                  '=' | '<' => {
                    self.cursor.next();
                  }
                  _ => return self.scan(pos),
                }
              } else if ch == '>' {
                self.cursor.next();

                match get_char!(self) {
                  '=' | '>' => {
                    self.cursor.next();
                  }
                  _ => return self.scan(pos),
                }
              } else if ch == '.' {
                self.cursor.next();

                match get_char!(self) {
                  '.' => {
                    self.cursor.next();
                  }
                  _ => return self.scan(pos),
                }
              } else if ch == '#' {
                self.cursor.next();

                match get_char!(self) {
                  '>' => {
                    self.cursor.next();
                  }
                  _ => return self.scan(pos),
                }
              } else {
                self.cursor.next();
                self.state.set(TokenizerState::Punctuation);

                return self.scan(pos);
              }
            }
            _ => {
              self.state.set(TokenizerState::Punctuation);

              return self.scan(pos);
            }
          },

          //# group-state.
          TokenizerState::Group => match ch {
            _ => {
              self.cursor.next();

              return self.scan(pos);
            }
          },

          //# ident-state.
          TokenizerState::Ident => match ch {
            'b' => {
              if self.cursor.back() == '#' {
                self.int_base.set(Base::Bin);
                self.cursor.next();
                self.state.set(TokenizerState::Num(Num::Int));
              } else {
                self.cursor.next();
              }
            }
            'o' => {
              if self.cursor.back() == '#' {
                self.int_base.set(Base::Oct);
                self.cursor.next();
                self.state.set(TokenizerState::Num(Num::Int));
              } else {
                self.cursor.next();
              }
            }
            'x' => {
              if self.cursor.back() == '#' {
                self.int_base.set(Base::Hex);
                self.cursor.next();
                self.state.set(TokenizerState::Num(Num::Int));
              } else {
                self.cursor.next();
              }
            }
            c if is!(ident_continue c) => {
              self.cursor.next();
            }
            _ => return self.scan(pos),
          },

          //# quote-state.
          TokenizerState::Quote => match ch {
            c if is!(quote_tick c) => {
              self.cursor.next();
              self.state.set(TokenizerState::Byte);
            }
            c if is!(quote_single c) => {
              self.cursor.next();
              self.state.set(TokenizerState::Char);
            }
            c if is!(quote_double c) => {
              self.cursor.next();
              self.state.set(TokenizerState::Str);
            }
            _ => {}
          },

          //# byte-state.
          // todo(ivs) — unstable, needs work.
          TokenizerState::Byte => match ch {
            '`' => {
              self.cursor.next();
              self.state.set(TokenizerState::Byte);

              return self.scan(pos);
            }
            '\\' => self.state.set(TokenizerState::Escape(In::Byte)),
            _ => {
              self.cursor.next();
            }
          },

          //# escaped-byte-state.
          // todo(ivs) — unstable, needs work.
          TokenizerState::Escape(In::Byte) => match ch {
            '\'' | '"' | '\\' => {
              self.cursor.next();
            }
            'u' => self.state.set(TokenizerState::Unicode(In::Byte)),
            _ => {
              self.cursor.next();
              self.state.set(TokenizerState::Byte);
            }
          },

          //# unicode-in-byte-state.
          // todo(ivs) — unstable, needs work.
          TokenizerState::Unicode(In::Byte) => match ch {
            '{' => {
              self.cursor.next();
            }
            '}' => {
              self.cursor.next();
              self.state.set(TokenizerState::Byte);

              return self.scan(pos);
            }
            _ => {
              self.cursor.next();
            }
          },

          //# char-state.
          // todo(ivs) — unstable, needs work.
          TokenizerState::Char => match ch {
            '\'' => {
              self.cursor.next();
              self.state.set(TokenizerState::Char);

              return self.scan(pos);
            }
            '\\' => self.state.set(TokenizerState::Escape(In::Char)),
            _ => {
              self.cursor.next();
            }
          },

          //# escape-in-char-state.
          // todo(ivs) — unstable, needs work.
          TokenizerState::Escape(In::Char) => match ch {
            '\'' | '"' | '\\' => {
              self.cursor.next();
            }
            'u' => self.state.set(TokenizerState::Unicode(In::Char)),
            _ => {
              self.cursor.next();
              self.state.set(TokenizerState::Char);
            }
          },

          //# unicode-in-char-state.
          // todo(ivs) — unstable, needs work.
          TokenizerState::Unicode(In::Char) => match ch {
            '{' => {
              self.cursor.next();
            }
            '}' => {
              self.cursor.next();
              self.state.set(TokenizerState::Char);

              return self.scan(pos);
            }
            _ => {
              self.cursor.next();
            }
          },

          //# str-state.
          // todo(ivs) — unstable, needs work.
          TokenizerState::Str => match ch {
            '\"' => {
              self.cursor.next();
              self.state.set(TokenizerState::Str);

              return self.scan(pos);
            }
            '\\' => self.state.set(TokenizerState::Escape(In::Str)),
            _ => {
              self.cursor.next();
            }
          },

          //# escape-in-str-state.
          // todo(ivs) — unstable, needs work.
          TokenizerState::Escape(In::Str) => match ch {
            '\'' | '"' | '\\' => {
              self.cursor.next();
            }
            'u' => self.state.set(TokenizerState::Unicode(In::Str)),
            _ => {
              self.cursor.next();
              self.state.set(TokenizerState::Str);
            }
          },

          //# unicode-in-str-state.
          // todo(ivs) — unstable, needs work.
          TokenizerState::Unicode(In::Str) => match ch {
            '{' => {
              self.cursor.next();
            }
            '}' => {
              self.cursor.next();
              self.state.set(TokenizerState::Str);

              return self.scan(pos);
            }
            _ => {
              self.cursor.next();
            }
          },

          //# unknwon-state.
          TokenizerState::Unknown => {
            let span = Span::of(pos, self.cursor.pos());

            self.reporter.raise(error::lexical::unknown(span, ch as u8));
          }

          s => panic!("State = {s:?}"),
        },
        //# --- MODE:PROGRAM:END. ---

        //# --- MODE:TEMPLATE:START. ---
        TokenizerMode::Template => match self.state.get() {
          //# zsx-data-state.
          TokenizerState::ZsxData => {
            pos = self.cursor.pos();

            match ch {
              c if is!(space c) => {
                self.cursor.next();
              }
              ';' => {
                self.switch();
                self.state.set(TokenizerState::Punctuation);
              }
              '<' => {
                self.cursor.next();
                self.state.set(TokenizerState::ZsxTagOpen);
              }
              _ => {
                self.cursor.next();
                self.state.set(TokenizerState::ZsxCharacter);

                return self.scan(pos);
              }
            }
          }

          //# zsx-tag-open-state.
          TokenizerState::ZsxTagOpen => match ch {
            '/' => {
              self.cursor.next();
              self.state.set(TokenizerState::ZsxTagOpenEnd);
            }
            '>' => {
              if self.cursor.back() == '<' {
                self.tag_current_frag.set(true);
                self.create_tag(TagKind::Opening, '_');
                self.cursor.next();
                self.state.set(TokenizerState::ZsxTag);

                return self.scan(pos);
              } else {
                self.tag_current_frag.set(false);
                self.cursor.next();
              }
            }
            _ => match to!(lower_ascii ch) {
              Some(ch) => {
                self.create_tag(TagKind::Opening, ch);
                self.cursor.next();
                self.state.set(TokenizerState::ZsxTagName)
              }
              None => {
                if ch == ':' {
                  self.create_tag(TagKind::Opening, ch);
                  self.cursor.next();
                  self.state.set(TokenizerState::ZsxTagName)
                } else {
                  self.state.set(TokenizerState::ZsxData);
                  panic!();
                }
              }
            },
          },

          //# zsx-tag-open-end-state.
          TokenizerState::ZsxTagOpenEnd => match ch {
            '>' => {
              if self.cursor.back() == '/' {
                self.tag_current_frag.set(true);
                self.create_tag(TagKind::Closing, '_');
                self.cursor.next();
                self.state.set(TokenizerState::ZsxTag);

                return self.scan(pos);
              } else {
                self.tag_current_frag.set(false);
                self.state.set(TokenizerState::ZsxData);
                panic!();
              }
            }
            _ => match to!(lower_ascii ch) {
              Some(c) => {
                self.create_tag(TagKind::Closing, c);
                self.cursor.next();
                self.state.set(TokenizerState::ZsxTagName);
              }
              None => {
                // self.state.set(TokenizerState::BogusComment);
                panic!();
              }
            },
          },

          //# zsx-tag-name-state.
          TokenizerState::ZsxTagName => match ch {
            '\t' | '\n' | '\x0C' | ' ' => {
              self.state.set(TokenizerState::ZsxBeforeAttributeName)
            }
            '/' => self.state.set(TokenizerState::ZsxTagSelfClosingStart),
            '>' => {
              self.cursor.next();
              self.state.set(TokenizerState::ZsxTag);

              return self.scan(pos);
            }
            _ => match to!(lower_ascii ch) {
              Some(c) => {
                self.append_to_tag_name(c);
                self.cursor.next();
              }
              None => {
                self.append_to_tag_name(ch);
                self.cursor.next();
              }
            },
          },

          //# zsx-before-attribute-name-state.
          TokenizerState::ZsxBeforeAttributeName => match ch {
            '\t' | '\n' | '\x0C' | ' ' => {
              self.cursor.next();
            }
            '>' => {
              self.cursor.next();
              self.state.set(TokenizerState::ZsxTag);

              return self.scan(pos);
            }
            '/' => {
              self.cursor.next();
              self.state.set(TokenizerState::ZsxTagSelfClosingStart)
            }
            _ => match to!(lower_ascii ch) {
              Some(c) => {
                self.create_attribute(c);
                self.cursor.next();
                self.state.set(TokenizerState::ZsxAttributeName)
              }
              None => {
                self.create_attribute(ch);
                self.cursor.next();
                self.state.set(TokenizerState::ZsxAttributeName)
              }
            },
          },

          //# zsx-attribute-name-state.
          TokenizerState::ZsxAttributeName => match ch {
            '\t' | '\n' | '\x0C' | ' ' => {
              self.cursor.next();
            }
            '/' => {
              self.cursor.next();
              self.state.set(TokenizerState::ZsxTagSelfClosingStart)
            }
            '=' => {
              self.cursor.next();
              self.state.set(TokenizerState::ZsxBeforeAttributeValue);
            }
            '>' => {
              self.cursor.next();
              self.state.set(TokenizerState::ZsxTag);

              return self.scan(pos);
            }
            c => match to!(lower_ascii c) {
              Some(c) => {
                self.attr_current_name.borrow_mut().push(c);
                self.cursor.next();
              }
              None => {
                self.attr_current_name.borrow_mut().push(c);
                self.cursor.next();
              }
            },
          },

          //# zsx-after-attribute-name-state.
          TokenizerState::ZsxAfterAttributeName => match ch {
            '\t' | '\n' | '\x0C' | ' ' => {
              self.cursor.next();
            }
            '/' => {
              self.cursor.next();
              self.state.set(TokenizerState::ZsxTagSelfClosingStart)
            }
            '=' => {
              self.cursor.next();
              self.state.set(TokenizerState::ZsxBeforeAttributeValue);
            }
            '>' => {
              self.cursor.next();
              self.state.set(TokenizerState::ZsxTag);

              return self.scan(pos);
            }
            c => match to!(lower_ascii c) {
              Some(c) => {
                self.create_attribute(c);
                self.cursor.next();
                self.state.set(TokenizerState::ZsxAttributeName);
              }
              None => {
                self.create_attribute(c);
                self.cursor.next();
                self.state.set(TokenizerState::ZsxAttributeName)
              }
            },
          },

          //# zsx-before-attribute-value-state.
          TokenizerState::ZsxBeforeAttributeValue => match ch {
            '{' => {
              self.cursor.next();

              self
                .state
                .set(TokenizerState::ZsxAttributeValue(Quoted::Brace));
            }
            _ => {
              self.cursor.next();

              self
                .state
                .set(TokenizerState::ZsxAttributeValue(Quoted::No));
            }
          },

          //# zsx-self-closing-start-tag-state
          TokenizerState::ZsxTagSelfClosingStart => match ch {
            '>' => {
              self.tag_current_self_closing.set(true);
              self.cursor.next();
              self.state.set(TokenizerState::ZsxTag);

              return self.scan(pos);
            }
            _ => {
              panic!("error; go to before attr name");
            }
          },

          s => panic!("State = {s:?}"),
        },
      }
    }

    None
  }

  /// Scans a token.
  fn scan(&mut self, pos: usize) -> Option<Token> {
    let scanned = &self.cursor.source()[pos..self.cursor.pos()];

    // println!(
    //   "\nSCAN => State = {:?} | Scanned = {:?} | Span = {:?}\n",
    //   self.state.get(),
    //   scanned,
    //   (pos, self.cursor.pos()),
    // );

    let maybe_kind = match self.state.get() {
      TokenizerState::CommentLine => Some(TokenKind::Comment(Comment::Line)),

      TokenizerState::CommentLineDoc => {
        Some(TokenKind::Comment(Comment::LineDoc(scanned.into())))
      }

      TokenizerState::Num(Num::Int) => {
        let mut scanned = scanned.replace('_', "");

        if scanned.contains("b#")
          || scanned.contains("o#")
          || scanned.contains("x#")
        {
          scanned = scanned
            .replace("b#", "")
            .replace("o#", "")
            .replace("x#", "");

          if let Ok(int) = scanned.parse::<u32>() {
            match self.int_base.get() {
              Base::Bin => scanned = format!("{int:#b}"),
              Base::Oct => scanned = format!("{int:#o}"),
              Base::Hex => scanned = format!("{int:#x}"),
              b => panic!("invalid parse base = {b:?}"),
            }
          }
        }

        let sym = self.interner.intern(&scanned);

        Some(TokenKind::Int(sym, self.int_base.get()))
      }

      TokenizerState::Num(Num::Float) => {
        let sym = self.interner.intern(&scanned.replace('_', ""));

        Some(TokenKind::Float(sym))
      }

      TokenizerState::Punctuation => {
        Some(TokenKind::Punctuation(Punctuation::from(scanned)))
      }

      TokenizerState::Group => {
        Some(TokenKind::Group(scanned.chars().next().map(Group::from)?))
      }

      TokenizerState::Ident => {
        if let Some(kind) = KEYWORDS.get::<str>(&scanned) {
          Some(kind.to_owned())
        } else {
          let sym = self.interner.intern(&scanned);

          Some(TokenKind::Ident(sym))
        }
      }

      TokenizerState::Byte => {
        let sym = self.interner.intern(&scanned);

        Some(TokenKind::Byte(sym))
      }

      TokenizerState::Char => {
        let sym = self.interner.intern(&scanned);

        Some(TokenKind::Char(sym))
      }

      TokenizerState::Str => {
        let sym = self.interner.intern(&scanned.replace("\"", ""));

        Some(TokenKind::Str(sym))
      }

      TokenizerState::Zss => Some(TokenKind::Zss(scanned.into())),

      TokenizerState::ZsxComment => {
        let comment = std::mem::take(&mut *self.comment_current.borrow_mut());

        Some(TokenKind::ZsxComment(comment))
      }

      TokenizerState::ZsxCharacter => Some(TokenKind::ZsxCharacter(
        scanned.chars().next().unwrap_or_default(),
      )),

      TokenizerState::ZsxTag => {
        // removes extra character in a string tag.
        let scanned =
          scanned.replace("<", "").replace("/", "").replace(">", "");

        // formats the tag name.
        let scanned = scanned.split_whitespace().next().unwrap_or("");

        let kind = self.tag_current_kind.borrow().get();
        let name = Name::from_name(scanned);
        let self_closing = self.tag_current_self_closing.get();
        let frag = self.tag_current_frag.get();
        let attrs = std::mem::take(&mut *self.tag_current_attrs.borrow_mut());

        self.tag_current_frag.set(false);

        Some(TokenKind::ZsxTag(Tag::new(
          kind,
          name,
          self_closing,
          frag,
          attrs,
        )))
      }
      _ => None,
    };

    self.reset();

    if let Some(kind) = maybe_kind {
      return Some(Token::new(kind, Span::of(pos, self.cursor.pos())));
    }

    None
  }
}

impl<'source> Iterator for Tokenizer<'source> {
  type Item = Token;

  /// Moves to the next token.
  fn next(&mut self) -> Option<Self::Item> {
    self.step()
  }
}

/// The representation of a tokenizer mode.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum TokenizerMode {
  /// A program mode.
  #[default]
  Program,
  /// A template mode.
  Template,
}

/// Transforms a source code into a stream of tokens.
pub fn tokenize(session: &mut Session, source: &str) -> Result<Vec<Token>> {
  Tokenizer::new(&mut session.interner, &session.reporter, source).tokenize()
}
