use super::cursor::Cursor;
use super::state::TokenizerState;
use super::state::{Expo, Num, Program, Quoted, Style, Template};

use zor_interner::interner::Interner;
use zor_reporter::Result;
use zor_session::session::Session;
use zor_token::token::template::{self, AttrKind};
use zor_token::token::template::{Attr, Tag, TagKind};
use zor_token::token::{program, style, Token, TokenKind};

use swisskit::span::Span;
use swisskit::{is, to};

// note(ivs) — this tokenizer tries to tokenize multiple language symbols such
// as programming, styling and templating symbols. to do it, it owns a mode
// property which is used to swtich to the right mode. why it is doing that?
// because it needs to avoid conflicts between similar symbols.
//
// for example in programming mode this symbol `<` it used as an operator but in
// styling or templating mode it means something else.

/// The representation of a tokenizer mode.
#[derive(Debug)]
enum TokenizerMode {
  /// A program mode.
  Program,
  /// A style mode.
  Style,
  /// A template mode.
  Template,
}

/// The representation of a Tokenizer.
pub struct Tokenizer<'source> {
  /// A cursor.
  cursor: Cursor,
  /// A tokenizer state.
  state: TokenizerState,
  /// A tokenizer mode.
  mode: TokenizerMode,
  /// A flag to check if we have reconsume the current character.
  reconsume: bool,
  /// A current character.
  char_current: char,
  /// A skip whitespace flag.
  skip_whitespace: bool,
  /// A program integer base.
  int_base: program::Base,
  /// A program exponent significand.
  expo_sign: i8,
  /// A program keywords collection.
  keywords: std::collections::HashMap<&'static str, TokenKind>,
  /// A program suffixes collection.
  suffixes: std::collections::HashSet<&'static str>,
  /// A style keywords collection.
  at_keywords: std::collections::HashMap<&'static str, TokenKind>,
  /// A template current tag.
  tag_current: Option<Tag>,
  /// A template current tag name.
  tag_current_name: String,
  /// A template last opening tag name.
  tag_last_start_name: Option<String>,
  /// A template current tag kind.
  tag_current_kind: TagKind,
  /// A flag to checks if the current tag is a self closing tag.
  tag_current_self_closing: bool,
  /// A template current attribute.
  attr_current: Attr,
  /// A template collection of attributes.
  attrs_current: Vec<Attr>,
  /// See [`Interner`].
  interner: &'source mut Interner,
}

impl<'source> Tokenizer<'source> {
  /// Creates a new tokenizer.
  pub fn new(
    source: &str,
    skip_whitespace: bool,
    interner: &'source mut Interner,
  ) -> Self {
    Self {
      cursor: Cursor::new(source),
      state: TokenizerState::Program(Program::Data),
      mode: TokenizerMode::Program,
      reconsume: false,
      char_current: '\0',
      skip_whitespace,
      int_base: program::Base::Dec,
      expo_sign: 1i8,
      keywords: program::keywords(),
      suffixes: program::suffixes(),
      at_keywords: style::keywords(),
      tag_current: None,
      tag_current_name: String::with_capacity(0usize),
      tag_last_start_name: None,
      tag_current_kind: TagKind::Opening,
      tag_current_self_closing: false,
      attr_current: Attr::new(),
      attrs_current: Vec::with_capacity(0usize),
      interner,
    }
  }

  /// Resets everything.
  fn reset(&mut self) {
    self.reset_state();
  }

  /// Resets the state to start to an initial state depending of the mode.
  fn reset_state(&mut self) {
    self.state = match self.mode {
      TokenizerMode::Program => TokenizerState::Program(Program::Data),
      TokenizerMode::Style => TokenizerState::Style(Style::Data),
      TokenizerMode::Template => TokenizerState::Template(Template::Data),
    };
  }

  /// Switches the tokenizer mode.
  fn switch(&mut self, mode: TokenizerMode) {
    self.mode = mode;
  }

  /// Skips whitespace character.
  fn consume_whitespace(&mut self) {
    self.cursor.consume_whitespace();
  }

  /// Tokenizes a source code into a stream of tokens.
  pub fn tokenize(&mut self) -> Result<Vec<Token>> {
    let mut tokens = Vec::with_capacity(0usize);

    while self.cursor.pos() < self.cursor.source().len() {
      tokens.push(self.next()?);
    }

    tokens.push(Token::eof(Span::of(self.cursor.pos(), self.cursor.pos())));

    Ok(tokens)
  }

  /// Gets the next character.
  fn get_char(&mut self) -> Option<char> {
    if self.reconsume {
      self.reconsume = false;

      Some(self.char_current)
    } else {
      self
        .cursor
        .peek()
        .and_then(|ch| self.get_preprocessed_char(ch))
    }
  }

  /// Gets the next character after preprocessing.
  fn get_preprocessed_char(&mut self, ch: char) -> Option<char> {
    self.char_current = ch;

    Some(ch)
  }

  /// Creates the current tag.
  fn create_tag(&mut self, kind: TagKind, ch: char) {
    let mut tag = Tag::new(kind);

    tag.name.push(ch);

    self.tag_current = Some(tag);
  }

  /// Appends the tag name.
  fn append_to_tag_name(&mut self, ch: char) {
    self.tag_current.as_mut().unwrap().name.push(ch);
  }

  fn have_appropriate_end_tag(&self) -> bool {
    match (self.tag_last_start_name.as_ref(), self.tag_current.as_ref()) {
      (Some(last), Some(tag)) => {
        (tag.kind == TagKind::Closing) && (tag.name == *last)
      }
      _ => false,
    }
  }

  /// Makes the tag.
  fn make_tag_current(&mut self) -> TokenKind {
    self.finish_attribute();

    let tag = self.tag_current.take().unwrap();

    match tag.kind {
      TagKind::Opening => self.tag_last_start_name = Some(tag.name.clone()),
      _ => (),
    };

    TokenKind::Template(template::Template::Tag(tag))
  }

  /// Creates the current attribute.
  fn create_attr(&mut self, ch: char) {
    self.finish_attribute();

    let attr = &mut self.attr_current;

    attr.name.push(ch);
  }

  /// Finalizes the attribute creation.
  fn finish_attribute(&mut self) {
    if self.attr_current.name.len() == 0 {
      return;
    }

    let duplicate = {
      let name = &self.attr_current.name;

      self
        .tag_current
        .as_ref()
        .unwrap()
        .attrs
        .iter()
        .any(|a| a.name == name.to_owned())
    };

    if duplicate {
      // add report — duplicate attribute.
      self.attr_current.clear();
    } else {
      let attr = std::mem::replace(&mut self.attr_current, Attr::new());

      self.tag_current.as_mut().unwrap().attrs.push(attr);
    }
  }
}

macro_rules! get_char ( ($me:expr) => (
  $me.get_char().unwrap()
));

impl<'source> Tokenizer<'source> {
  /// Consomes the current character.
  pub fn next(&mut self) -> Result<Token> {
    let mut pos = self.cursor.pos();
    let mut dynamic = false;

    while let Some(ch) = self.get_char() {
      match self.mode {
        // program-mode.
        TokenizerMode::Program => match self.state {
          // program-data-state.
          TokenizerState::Program(Program::Data) => {
            pos = self.cursor.pos();

            match ch {
              c if is!(space c) => self.consume_whitespace(),
              c if is!(number_zero c) => {
                self.state = TokenizerState::Program(Program::Num(Num::Zero));
              }
              c if is!(number_non_zero c) => {
                self.state = TokenizerState::Program(Program::Num(Num::Int));
              }
              c if is!(punctuation c) => {
                self.state = TokenizerState::Program(Program::Punctuation);
              }
              c if is!(group c) => {
                self.state = TokenizerState::Program(Program::Group);
              }
              c if is!(ident_start c) => {
                self.state = TokenizerState::Program(Program::Ident);
              }
              c if is!(quote c) => {
                self.state = TokenizerState::Program(Program::Quote);
              }
              '$' => {
                self.cursor.next();
                self.switch(TokenizerMode::Style);

                self.state = TokenizerState::Program(Program::Ident);

                return self.scan(pos);
              }
              _ => self.state = TokenizerState::Program(Program::Unknown),
            }
          }

          // program-comment-line-state.
          TokenizerState::Program(Program::CommentLine) => match ch {
            c if !is!(eol c) => {
              self.cursor.next();
            }
            _ => {
              self.cursor.next();

              self.state = TokenizerState::Program(Program::Data);
            }
          },

          // program-comment-line-doc-state.
          TokenizerState::Program(Program::CommentLineDoc) => match ch {
            c if !is!(eol c) => {
              self.cursor.next();
            }
            _ => {
              self.cursor.next();

              self.state = TokenizerState::Program(Program::Data);
            }
          },

          // program-num-zero-state.
          TokenizerState::Program(Program::Num(Num::Zero)) => match ch {
            c if is!(dot c) => {
              self.cursor.next();

              self.state = TokenizerState::Program(Program::Num(Num::Float));
            }
            c if is!(number_non_zero c) => {
              self.state = TokenizerState::Program(Program::InvalidNumber);
            }
            _ => {
              self.state = TokenizerState::Program(Program::Num(Num::Int));
            }
          },

          // program-num-int-state.
          TokenizerState::Program(Program::Num(Num::Int)) => match ch {
            c if is!(number c) || is!(underscore c) => {
              self.cursor.next();
            }
            c if is!(dot c) => {
              self.cursor.next();

              self.state = TokenizerState::Program(Program::Num(Num::Float));
            }
            'e' | 'E' => {
              self.cursor.next();

              self.state =
                TokenizerState::Program(Program::Num(Num::Expo(Expo::E)));
            }
            _ => return self.scan(pos),
          },

          // program-num-bin-state.
          TokenizerState::Program(Program::Num(Num::Bin)) => match ch {
            c if is!(number_bin c) || is!(underscore c) => {
              self.cursor.next();
            }
            _ => {
              self.state = TokenizerState::Program(Program::Num(Num::Int));

              return self.scan(pos);
            }
          },

          // program-num-oct-state.
          TokenizerState::Program(Program::Num(Num::Oct)) => match ch {
            c if is!(number_oct c) || is!(underscore c) => {
              self.cursor.next();
            }
            _ => {
              self.state = TokenizerState::Program(Program::Num(Num::Int));

              return self.scan(pos);
            }
          },

          // program-num-hex-state.
          TokenizerState::Program(Program::Num(Num::Hex)) => match ch {
            c if is!(number_hex c) || is!(underscore c) => {
              self.cursor.next();
            }
            _ => {
              self.state = TokenizerState::Program(Program::Num(Num::Hex));

              return self.scan(pos);
            }
          },

          // program-num-float-state.
          TokenizerState::Program(Program::Num(Num::Float)) => match ch {
            c if is!(number c) || is!(underscore c) => {
              self.cursor.next();
            }
            _ => return self.scan(pos),
          },

          // program-num-expo-e-state.
          TokenizerState::Program(Program::Num(Num::Expo(Expo::E))) => match ch
          {
            '+' => {
              self.cursor.next();

              self.expo_sign = 1i8;

              self.state =
                TokenizerState::Program(Program::Num(Num::Expo(Expo::Sign)));
            }
            '-' => {
              self.cursor.next();

              self.expo_sign = -1i8;

              self.state =
                TokenizerState::Program(Program::Num(Num::Expo(Expo::Sign)));
            }
            c if is!(number c) => {
              self.cursor.next();

              self.state =
                TokenizerState::Program(Program::Num(Num::Expo(Expo::Digits)));
            }
            _ => {
              // todo(ivs) — add a report error message.
              // moves the cursor to the next character.
              panic!()
            }
          },

          // program-num-expo-sign-state.
          TokenizerState::Program(Program::Num(Num::Expo(Expo::Sign))) => {
            match ch {
              c if is!(number c) => {
                self.cursor.next();

                self.state = TokenizerState::Program(Program::Num(Num::Expo(
                  Expo::Digits,
                )));
              }
              _ => {
                // todo(ivs) — add a report error message.
                // moves the cursor to the next character.
                panic!()
              }
            }
          }

          // program-num-expo-digits-state.
          TokenizerState::Program(Program::Num(Num::Expo(Expo::Digits))) => {
            match ch {
              c if is!(number c) => {
                self.cursor.next();

                self.state = TokenizerState::Program(Program::Num(Num::Expo(
                  Expo::Digits,
                )));
              }
              _ => {
                self.state = TokenizerState::Program(Program::Num(Num::Float));

                return self.scan(pos);
              }
            }
          }

          // program-punctuation-state.
          TokenizerState::Program(Program::Punctuation) => match ch {
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
                  '-' => {
                    self.state = TokenizerState::Program(Program::CommentLine)
                  }
                  '!' => {
                    self.state =
                      TokenizerState::Program(Program::CommentLineDoc)
                  }
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
                        self.switch(TokenizerMode::Template);
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

                return self.scan(pos);
              }
            }
            _ => return self.scan(pos),
          },

          // program-group-state.
          TokenizerState::Program(Program::Group) => {
            self.cursor.next();

            return self.scan(pos);
          }

          // program-ident-state.
          TokenizerState::Program(Program::Ident) => match ch {
            'b' => {
              if self.cursor.front() == '#' {
                self.int_base = program::Base::Bin;
                self.cursor.next();

                self.state = TokenizerState::Program(Program::Num(Num::Bin));
              } else {
                self.cursor.next();
              }
            }
            'o' => {
              if self.cursor.front() == '#' {
                self.int_base = program::Base::Oct;
                self.cursor.next();

                self.state = TokenizerState::Program(Program::Num(Num::Oct));
              } else {
                self.cursor.next();
              }
            }
            'x' => {
              if self.cursor.front() == '#' {
                self.int_base = program::Base::Hex;
                self.cursor.next();

                self.state = TokenizerState::Program(Program::Num(Num::Hex));
              } else {
                self.cursor.next();
              }
            }
            c if is!(ident_continue c) => {
              self.cursor.next();
            }
            _ => return self.scan(pos),
          },

          // program-quote-state.
          TokenizerState::Program(Program::Quote) => match ch {
            _ => todo!(),
          },

          // program-char-state.
          TokenizerState::Program(Program::Char) => match ch {
            _ => todo!(),
          },

          // program-str-state.
          TokenizerState::Program(Program::Str) => match ch {
            _ => todo!(),
          },

          // program-invalid-number-state.
          TokenizerState::Program(Program::InvalidNumber) => {
            panic!("invalid number (leading zero): {ch:?}")
          }

          // program-unknown-state.
          TokenizerState::Program(Program::Unknown) => {
            panic!("unknown character: {ch:?}")
          }

          // program-state-unimplemented-yet.
          _ => panic!("State::Program = {:?}", self.state),
        },

        // style-mode.
        TokenizerMode::Style => match self.state {
          // style-data-state.
          TokenizerState::Style(Style::Data) => {
            pos = self.cursor.pos();

            match ch {
              c if is!(space c) => self.consume_whitespace(),
              c if is!(quote c) => {
                self.state = TokenizerState::Style(Style::Quote);
              }
              c if is!(delim c) => {
                self.state = TokenizerState::Style(Style::Delim);
              }
              c if is!(group c) => {
                self.state = TokenizerState::Style(Style::Group);
              }
              c if is!(ident_start c) => {
                self.state = TokenizerState::Style(Style::Ident);
              }
              ';' => {
                self.switch(TokenizerMode::Program);

                self.state = TokenizerState::Program(Program::Punctuation);
              }
              _ => todo!(),
            }
          }

          // style-quote-state.
          TokenizerState::Style(Style::Quote) => match ch {
            c if is!(quote_double c) | is!(quote_single c) => todo!(),
            _ => todo!(),
          },

          // style-ident-state.
          TokenizerState::Style(Style::Ident) => match ch {
            c if is!(ident_continue c) => {
              self.cursor.next();
            }
            _ => return self.scan(pos),
          },

          // style-group-state.
          TokenizerState::Style(Style::Group) => {
            self.cursor.next();

            return self.scan(pos);
          }

          // style-state-unimplemented-yet.
          _ => panic!("State::Style = {:?}", self.state),
        },

        // template-mode.
        TokenizerMode::Template => match self.state {
          // template-data-state.
          TokenizerState::Template(Template::Data) => {
            pos = self.cursor.pos();

            match ch {
              '{' => {
                self.cursor.next();

                self.state = TokenizerState::Template(Template::Expr);
              }
              '<' => {
                self.cursor.next();

                self.state = TokenizerState::Template(Template::TagOpen);
              }
              ';' => {
                self.switch(TokenizerMode::Program);

                self.state = TokenizerState::Program(Program::Punctuation);
              }
              _ => {
                self.cursor.next();

                self.state = TokenizerState::Template(Template::Character);

                return self.scan(pos);
              }
            }
          }

          // template-raw-text-state.
          TokenizerState::Template(Template::Expr) => match ch {
            '}' => {
              self.cursor.next();

              return self.scan(pos);
            }
            _ => {
              self.cursor.next();
            }
          },

          // template-raw-text-state.
          TokenizerState::Template(Template::RawText) => match ch {
            _ => todo!(),
          },

          // template-tag-open-state.
          TokenizerState::Template(Template::TagOpen) => match ch {
            ':' => {
              self.cursor.next();

              self.state = TokenizerState::Template(Template::Directive);
            }
            '/' => {
              self.cursor.next();

              self.state = TokenizerState::Template(Template::TagOpenEnd);
            }
            _ => match to!(lower_ascii ch) {
              Some(c) => {
                self.create_tag(TagKind::Opening, c);
                self.cursor.next();

                self.state = TokenizerState::Template(Template::TagName);
              }
              None => {
                // add report.
                // state to Template::Data.
                // reconsume.
                panic!()
              }
            },
          },

          // template-directive-state.
          TokenizerState::Template(Template::Directive) => match ch {
            c if is!(ident c) => {
              self.cursor.next();
            }
            _ => return self.scan(pos),
          },

          // template-tag-open-end-state.
          TokenizerState::Template(Template::TagOpenEnd) => match ch {
            _ => match to!(lower_ascii ch) {
              Some(c) => {
                self.create_tag(TagKind::Closing, c);
                self.cursor.next();

                self.state = TokenizerState::Template(Template::TagName);
              }
              None => {
                // add report.
                // state to Template::BogusComment.
                panic!();
              }
            },
          },

          // template-tag-name-state.
          TokenizerState::Template(Template::TagName) => match ch {
            '\t' | '\n' | '\x0C' | ' ' => {
              self.state =
                TokenizerState::Template(Template::BeforeAttributeName);
            }
            '/' => {
              self.cursor.next();

              self.state =
                TokenizerState::Template(Template::TagSelfClosingStart);
            }
            '>' => {
              self.cursor.next();

              self.state = TokenizerState::Template(Template::Tag);

              return self.scan(pos);
            }
            c => match to!(lower_ascii c) {
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

          // template-before-attribute-name-state.
          TokenizerState::Template(Template::BeforeAttributeName) => match ch {
            '\t' | '\n' | '\x0C' | ' ' => {
              self.cursor.next();
            }
            '/' => {
              self.cursor.next();

              self.state =
                TokenizerState::Template(Template::TagSelfClosingStart);
            }
            '>' => {
              self.cursor.next();

              self.state = TokenizerState::Template(Template::Tag);

              return self.scan(pos);
            }
            '{' => {
              self.cursor.next();

              dynamic = true;

              self.attr_current.kind = AttrKind::Dynamic;

              self.state = TokenizerState::Template(Template::AttributeValue(
                Quoted::Brace,
              ));
            }
            c => match to!(lower_ascii c) {
              Some(c) => {
                self.create_attr(c);
                self.cursor.next();

                self.state = TokenizerState::Template(Template::AttributeName);
              }
              None => {
                self.create_attr(ch);
                self.cursor.next();

                self.state = TokenizerState::Template(Template::AttributeName);
              }
            },
          },

          // template-attribute-name-state.
          TokenizerState::Template(Template::AttributeName) => match ch {
            '\t' | '\n' | '\x0C' | ' ' => {
              self.cursor.next();

              self.state =
                TokenizerState::Template(Template::AfterAttributeName);
            }
            '/' => {
              self.cursor.next();

              self.state =
                TokenizerState::Template(Template::TagSelfClosingStart);
            }
            '=' => {
              self.cursor.next();

              self.state =
                TokenizerState::Template(Template::BeforeAttributeValue);
            }
            '>' => {
              self.cursor.next();

              self.state = TokenizerState::Template(Template::Tag);

              return self.scan(pos);
            }
            '\0' => panic!(),
            c => match to!(lower_ascii c) {
              Some(c) => {
                self.attr_current.name.push(c);
                self.cursor.next();
              }
              None => {
                self.attr_current.name.push(ch);
                self.cursor.next();
              }
            },
          },

          // template-after-attribute-name-state.
          TokenizerState::Template(Template::AfterAttributeName) => match ch {
            '\t' | '\n' | '\x0C' | ' ' => {
              self.cursor.next();
            }
            '/' => {
              self.cursor.next();

              self.state =
                TokenizerState::Template(Template::TagSelfClosingStart);
            }
            '=' => {
              self.cursor.next();

              self.state =
                TokenizerState::Template(Template::BeforeAttributeValue);
            }
            '>' => {
              self.cursor.next();

              self.state = TokenizerState::Template(Template::Tag);

              return self.scan(pos);
            }
            c => match to!(lower_ascii c) {
              Some(c) => {
                self.create_attr(c);
                self.cursor.next();

                self.state = TokenizerState::Template(Template::AttributeName);
              }
              None => {
                self.create_attr(ch);
                self.cursor.next();

                self.state = TokenizerState::Template(Template::AttributeName);
              }
            },
          },

          // template-before-attribute-value-state.
          TokenizerState::Template(Template::BeforeAttributeValue) => {
            match ch {
              '\t' | '\n' | '\x0C' | ' ' => {
                self.cursor.next();
              }
              '"' => {
                self.cursor.next();

                self.state = TokenizerState::Template(
                  Template::AttributeValue(Quoted::Double),
                );
              }
              '\'' => {
                self.cursor.next();

                self.state = TokenizerState::Template(
                  Template::AttributeValue(Quoted::Single),
                );
              }
              '{' => {
                self.cursor.next();

                self.attr_current.kind = AttrKind::Dynamic;

                self.state = TokenizerState::Template(
                  Template::AttributeValue(Quoted::Brace),
                );
              }
              _ => {
                self.attr_current.value.push(ch);
                self.cursor.next();

                self.state = TokenizerState::Template(
                  Template::AttributeValue(Quoted::No),
                );
              }
            }
          }

          // template-attribute-value-quoted-double-state.
          TokenizerState::Template(Template::AttributeValue(
            Quoted::Double,
          )) => match ch {
            '"' => {
              self.cursor.next();

              self.state =
                TokenizerState::Template(Template::AfterAttributeValue);
            }
            _ => {
              self.attr_current.value.push(ch);
              self.cursor.next();
            }
          },

          // template-attribute-value-quoted-single-state.
          TokenizerState::Template(Template::AttributeValue(
            Quoted::Single,
          )) => match ch {
            '\'' => {
              self.cursor.next();

              self.state =
                TokenizerState::Template(Template::AfterAttributeValue);
            }
            _ => {
              self.attr_current.value.push(ch);
              self.cursor.next();
            }
          },

          // template-attribute-value-quoted-brace-state.
          TokenizerState::Template(Template::AttributeValue(Quoted::Brace)) => {
            match ch {
              '}' => {
                self.cursor.next();

                dynamic = false;

                self.state =
                  TokenizerState::Template(Template::AfterAttributeValue);
              }
              _ => {
                if dynamic {
                  self.attr_current.name.push(ch);
                }

                self.attr_current.value.push(ch);
                self.cursor.next();
              }
            }
          }

          // template-attribute-value-quoted-no-state.
          TokenizerState::Template(Template::AttributeValue(Quoted::No)) => {
            match ch {
              '\t' | '\n' | '\x0C' | ' ' => {
                self.cursor.next();

                self.state =
                  TokenizerState::Template(Template::BeforeAttributeName);
              }
              '>' => {
                self.cursor.next();

                self.state = TokenizerState::Template(Template::Tag);

                return self.scan(pos);
              }
              _ => {
                self.attr_current.value.push(ch);
                self.cursor.next();
              }
            }
          }

          // template-after-attribute-value-state.
          TokenizerState::Template(Template::AfterAttributeValue) => match ch {
            '\t' | '\n' | '\x0C' | ' ' => {
              self.cursor.next();

              self.state =
                TokenizerState::Template(Template::BeforeAttributeName);
            }
            '/' => {
              self.cursor.next();

              self.state =
                TokenizerState::Template(Template::TagSelfClosingStart);
            }
            '>' => {
              self.cursor.next();

              self.state = TokenizerState::Template(Template::Tag);

              return self.scan(pos);
            }
            _ => {
              // add report error.
              // reconsume.
              // state to BeforeAttributeName.
              panic!();
            }
          },

          // template-tag-self-closing-start-state.
          TokenizerState::Template(Template::TagSelfClosingStart) => match ch {
            '>' => {
              self.cursor.next();

              self.tag_current.as_mut().unwrap().self_closing = true;

              self.state = TokenizerState::Template(Template::Tag);

              return self.scan(pos);
            }
            _ => {
              // add report error.
              // reconsume.
              // state to BeforeAttributeName.
              panic!();
            }
          },

          // template-state-unimplemented-yet.
          _ => panic!("State::Template = {:?}", self.state),
        },
      }
    }

    self.scan(pos)
  }

  /// Scans a token.
  fn scan(&mut self, pos: usize) -> Result<Token> {
    use zor_token::token;

    let source = &self.cursor.source()[pos..self.cursor.pos()];

    // println!("{}--{}", pos, self.cursor.pos());
    // println!("SOURCE: {:?}", source);
    // println!("STATE: {:?}", self.state);

    let maybe_kind = match self.state {
      TokenizerState::Program(Program::Num(Num::Int)) => {
        let mut source = source.replace('_', "");

        if source.contains("b#")
          || source.contains("o#")
          || source.contains("x#")
        {
          source = source.replace("b#", "").replace("o#", "").replace("x#", "");

          if let Ok(int) = source.parse::<u32>() {
            match self.int_base {
              token::program::Base::Bin => source = format!("{int:#b}"),
              token::program::Base::Oct => source = format!("{int:#o}"),
              token::program::Base::Hex => source = format!("{int:#x}"),
              b => panic!("invalid parse base = {b:?}"),
            }
          }
        }

        Some(TokenKind::Program(token::Program::Int(
          source,
          self.int_base,
        )))
      }
      TokenizerState::Program(Program::Num(Num::Float)) => Some(
        TokenKind::Program(token::Program::Float(source.to_string())),
      ),
      TokenizerState::Program(Program::Punctuation) => {
        Some(TokenKind::Program(token::Program::Punctuation(
          program::Punctuation::from(source),
        )))
      }
      TokenizerState::Program(Program::Group) => {
        Some(TokenKind::Program(token::Program::Group(
          source.chars().next().map(program::Group::from).unwrap(),
        )))
      }
      TokenizerState::Program(Program::Ident) => {
        if let Some(kind) = self.keywords.get(source) {
          Some(kind.to_owned())
        } else {
          let sym = self.interner.intern(source);

          Some(TokenKind::Program(token::Program::Ident(sym)))
        }
      }
      TokenizerState::Program(Program::Char) => {
        todo!()
      }
      TokenizerState::Program(Program::Str) => {
        todo!()
      }

      TokenizerState::Style(Style::Delim) => {
        Some(TokenKind::Style(token::Style::Delim(
          source.chars().next().map(style::Delim::from).unwrap(),
        )))
      }
      TokenizerState::Style(Style::Group) => {
        Some(TokenKind::Style(token::Style::Group(
          source.chars().next().map(style::Group::from).unwrap(),
        )))
      }
      TokenizerState::Style(Style::Ident) => {
        if let Some(kind) = self.at_keywords.get(source) {
          Some(kind.to_owned())
        } else {
          let sym = self.interner.intern(source);

          Some(TokenKind::Style(token::Style::Ident(sym)))
        }
      }

      TokenizerState::Template(Template::Expr) => {
        let source = source.replace("{", "").replace("}", "");

        println!("EXPR: {:?}", source);

        Some(TokenKind::Template(token::Template::Expr(source)))
      }

      TokenizerState::Template(Template::Character) => {
        Some(TokenKind::Template(token::Template::Character(
          source.chars().next().unwrap_or_default(),
        )))
      }
      TokenizerState::Template(Template::Tag) => Some(self.make_tag_current()),

      _ => None,
    };

    // println!("KIND: {:?}", maybe_kind);

    self.reset();

    if let Some(kind) = maybe_kind {
      return Ok(Token::new(kind, Span::of(pos, self.cursor.pos())));
    }

    Ok(Token::eof(Span::of(pos, self.cursor.pos())))
  }
}

/// Transforms a source code into a stream of tokens.
pub fn tokenize(
  source: &str,
  skip_whitespace: bool,
  session: &mut Session,
) -> Result<Vec<Token>> {
  Tokenizer::new(source, skip_whitespace, &mut session.interner).tokenize()
}

#[cfg(test)]
mod tests {
  use super::Tokenizer;

  use zor_interner::symbol::Symbol;
  use zor_session::session::Session;
  use zor_token::token::program::{Base, Group, Kw, Punctuation};
  use zor_token::token::{style, Program, Style, Template, TokenKind};

  #[test]
  fn tokenize_tokens() {
    let source = "return 1 + 2 ;";
    let source = "::= <a bar=\"foo\" foo={}></a>;";

    let mut session = Session::default();
    let mut tokenizer = Tokenizer::new(source, true, &mut session.interner);
    let actual = tokenizer.tokenize().unwrap();

    println!("{:#?}", actual);

    let expected = Vec::from([
      TokenKind::Program(Program::Kw(Kw::Return)),
      TokenKind::Program(Program::Int(String::from("1"), Base::Dec)),
      TokenKind::Program(Program::Punctuation(Punctuation::Plus)),
      TokenKind::Program(Program::Int(String::from("2"), Base::Dec)),
      TokenKind::Program(Program::Punctuation(Punctuation::Semi)),
      TokenKind::Eof,
    ]);

    for (idx, expected_kind) in expected.iter().enumerate() {
      let actual_kind = &actual[idx].kind;

      assert_eq!(actual_kind, expected_kind);
    }
  }

  // #[test]
  // fn tokenize_program_empty() {
  //   let source = "";
  //   let mut tokenizer = Tokenizer::new(source);

  //   assert_eq!(tokenizer.next().unwrap().kind, TokenKind::Eof);
  // }

  // #[test]
  // fn tokenize_program_line_comments() {
  //   let source = "-- this is a line comments.";
  //   let mut tokenizer = Tokenizer::new(source);

  //   assert_eq!(tokenizer.next().unwrap().kind, TokenKind::Eof);
  // }

  // #[test]
  // fn tokenize_program_integers() {
  //   let source = "0";
  //   let mut tokenizer = Tokenizer::new(source);

  //   assert_eq!(
  //     tokenizer.next().unwrap().kind,
  //     TokenKind::Program(Program::Int(String::from("0"), Base::Dec))
  //   );

  //   assert_eq!(tokenizer.next().unwrap().kind, TokenKind::Eof);
  // }

  // #[test]
  // fn tokenize_program_groups() {
  //   let source = "( ) [ ] { }";
  //   let mut tokenizer = Tokenizer::new(source);

  //   assert_eq!(
  //     tokenizer.next().unwrap().kind,
  //     TokenKind::Program(Program::Group(Group::ParenOpen)),
  //   );

  //   assert_eq!(
  //     tokenizer.next().unwrap().kind,
  //     TokenKind::Program(Program::Group(Group::ParenClose)),
  //   );

  //   assert_eq!(
  //     tokenizer.next().unwrap().kind,
  //     TokenKind::Program(Program::Group(Group::BracketOpen)),
  //   );

  //   assert_eq!(
  //     tokenizer.next().unwrap().kind,
  //     TokenKind::Program(Program::Group(Group::BracketClose)),
  //   );

  //   assert_eq!(
  //     tokenizer.next().unwrap().kind,
  //     TokenKind::Program(Program::Group(Group::BraceOpen)),
  //   );

  //   assert_eq!(
  //     tokenizer.next().unwrap().kind,
  //     TokenKind::Program(Program::Group(Group::BraceClose)),
  //   );

  //   assert_eq!(tokenizer.next().unwrap().kind, TokenKind::Eof);
  // }

  #[test]
  fn tokenize_style_declaration() {
    let source = "$ css {};";
    let mut session = Session::default();
    let mut tokenizer = Tokenizer::new(source, true, &mut session.interner);

    assert_eq!(
      tokenizer.next().unwrap().kind,
      TokenKind::Program(Program::Kw(Kw::Style))
    );

    assert_eq!(
      tokenizer.next().unwrap().kind,
      TokenKind::Style(Style::Ident(Symbol::new(0u32)))
    );

    assert_eq!(
      tokenizer.next().unwrap().kind,
      TokenKind::Style(Style::Group(style::Group::BraceOpen))
    );

    assert_eq!(
      tokenizer.next().unwrap().kind,
      TokenKind::Style(Style::Group(style::Group::BraceClose))
    );

    assert_eq!(
      tokenizer.next().unwrap().kind,
      TokenKind::Program(Program::Punctuation(Punctuation::Semi))
    );

    assert_eq!(tokenizer.next().unwrap().kind, TokenKind::Eof);
  }

  #[test]
  fn tokenize_template_declaration() {
    let source = "::= ;";
    let mut session = Session::default();
    let mut tokenizer = Tokenizer::new(source, true, &mut session.interner);

    assert_eq!(
      tokenizer.next().unwrap().kind,
      TokenKind::Program(Program::Punctuation(Punctuation::ColonColonEqual))
    );

    assert_eq!(
      tokenizer.next().unwrap().kind,
      TokenKind::Template(Template::Character(' '))
    );

    assert_eq!(
      tokenizer.next().unwrap().kind,
      TokenKind::Program(Program::Punctuation(Punctuation::Semi))
    );

    assert_eq!(tokenizer.next().unwrap().kind, TokenKind::Eof);
  }

  #[test]
  fn tokenize_template_characters() {
    let source = "::= hello ;";
    let mut session = Session::default();
    let mut tokenizer = Tokenizer::new(source, true, &mut session.interner);

    assert_eq!(
      tokenizer.next().unwrap().kind,
      TokenKind::Program(Program::Punctuation(Punctuation::ColonColonEqual))
    );

    assert_eq!(
      tokenizer.next().unwrap().kind,
      TokenKind::Template(Template::Character(' '))
    );

    assert_eq!(
      tokenizer.next().unwrap().kind,
      TokenKind::Template(Template::Character('h'))
    );

    assert_eq!(
      tokenizer.next().unwrap().kind,
      TokenKind::Template(Template::Character('e'))
    );

    assert_eq!(
      tokenizer.next().unwrap().kind,
      TokenKind::Template(Template::Character('l'))
    );

    assert_eq!(
      tokenizer.next().unwrap().kind,
      TokenKind::Template(Template::Character('l'))
    );

    assert_eq!(
      tokenizer.next().unwrap().kind,
      TokenKind::Template(Template::Character('o'))
    );

    assert_eq!(
      tokenizer.next().unwrap().kind,
      TokenKind::Template(Template::Character(' '))
    );

    assert_eq!(
      tokenizer.next().unwrap().kind,
      TokenKind::Program(Program::Punctuation(Punctuation::Semi))
    );

    assert_eq!(tokenizer.next().unwrap().kind, TokenKind::Eof);
  }

  #[test]
  fn tokenize_template_tag() {
    let mut source = "::= <a></a>";
    let mut session = Session::default();
    let mut tokenizer = Tokenizer::new(source, true, &mut session.interner);

    assert_eq!(
      tokenizer.next().unwrap().kind,
      TokenKind::Program(Program::Punctuation(Punctuation::ColonColonEqual))
    );
  }

  #[test]
  fn tokenize_template_tag_with_attribute_name() {
    let mut source = "::= <a foo></a>";
    let mut session = Session::default();
    let mut tokenizer = Tokenizer::new(source, true, &mut session.interner);

    assert_eq!(
      tokenizer.next().unwrap().kind,
      TokenKind::Program(Program::Punctuation(Punctuation::ColonColonEqual))
    );
  }

  #[test]
  fn tokenize_template_tag_with_attribute_name_and_value_double_quoted() {
    let mut source = "::= <a bar=\"foo\"></a>";
    let mut session = Session::default();
    let mut tokenizer = Tokenizer::new(source, true, &mut session.interner);

    assert_eq!(
      tokenizer.next().unwrap().kind,
      TokenKind::Program(Program::Punctuation(Punctuation::ColonColonEqual))
    );
  }

  #[test]
  fn tokenize_template_tag_with_attribute_name_and_value_single_quoted() {
    let mut source = "::= <a rab='oof'></a>";
    let mut session = Session::default();
    let mut tokenizer = Tokenizer::new(source, true, &mut session.interner);

    assert_eq!(
      tokenizer.next().unwrap().kind,
      TokenKind::Program(Program::Punctuation(Punctuation::ColonColonEqual))
    );
  }

  #[test]
  fn tokenize_template_tag_with_attribute_name_and_value_no_quoted() {
    let mut source = "::= <a oof=rab></a>";
    let mut session = Session::default();
    let mut tokenizer = Tokenizer::new(source, true, &mut session.interner);

    assert_eq!(
      tokenizer.next().unwrap().kind,
      TokenKind::Program(Program::Punctuation(Punctuation::ColonColonEqual))
    );
  }

  #[test]
  fn tokenize_template_tag_with_attribute_name_and_dynamic_value() {
    let mut source = "::= <a {ivs} svi={2 + 1}></a>";
    let mut session = Session::default();
    let mut tokenizer = Tokenizer::new(source, true, &mut session.interner);

    assert_eq!(
      tokenizer.next().unwrap().kind,
      TokenKind::Program(Program::Punctuation(Punctuation::ColonColonEqual))
    );
  }
}
