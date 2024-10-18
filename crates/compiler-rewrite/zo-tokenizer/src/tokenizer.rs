use super::cursor::Cursor;
use super::state::TokenizerState;
use super::state::{Expo, Num, Program, Quoted, Style, Template};

use zor_reporter::Result;
use zor_token::token::style::AtKeyword;
use zor_token::token::template::{Attr, TagKind};
use zor_token::token::{program, style, Token, TokenKind};

use swisskit::is;
use swisskit::span::Span;

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
pub struct Tokenizer {
  /// A cursor.
  cursor: Cursor,
  /// A tokenizer state.
  state: TokenizerState,
  /// A tokenizer mode.
  mode: TokenizerMode,
  /// An exponent significand.
  expo_sign: i8,
  /// An integer base.
  int_base: program::Base,
  /// A flag to check if we have reconsume the current character.
  reconsume: bool,
  /// A current character.
  char_current: char,
  /// A program keywords collection.
  keywords: std::collections::HashMap<&'static str, TokenKind>,
  /// A current tag name.
  tag_current_name: String,
  /// A current tag kind.
  tag_current_kind: TagKind,
  /// A flag to checks if the current tag is a self closing tag.
  tag_current_self_closing: bool,
  /// A current attribute.
  tag_current_attr: String,
  /// A collection of attributes.
  tag_current_attrs: Vec<Attr>,
  /// A style keywords collection.
  at_keywords: std::collections::HashMap<&'static str, AtKeyword>,
}

impl Tokenizer {
  /// Creates a new tokenizer.
  pub fn new(source: &str) -> Self {
    Self {
      cursor: Cursor::new(source),
      state: TokenizerState::Program(Program::Data),
      mode: TokenizerMode::Program,
      expo_sign: 1i8,
      int_base: program::Base::Dec,
      reconsume: false,
      char_current: '\0',
      keywords: program::keywords(),
      tag_current_name: String::with_capacity(0usize),
      tag_current_kind: TagKind::Opening,
      tag_current_self_closing: false,
      tag_current_attr: String::with_capacity(0usize),
      tag_current_attrs: Vec::with_capacity(0usize),
      at_keywords: style::keywords(),
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
}

macro_rules! get_char ( ($me:expr) => (
  $me.get_char().unwrap()
));

impl Tokenizer {
  /// Consomes the current character.
  pub fn next(&mut self) -> Result<Token> {
    let mut pos = self.cursor.pos();

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
              '$' => {
                self.cursor.next();
                self.switch(TokenizerMode::Style);

                self.state = TokenizerState::Style(Style::Data);
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
            '#' => todo!(),
            'b' => todo!(),
            'o' => todo!(),
            'x' => todo!(),
            'e' | 'E' => todo!(),
            _ => return self.scan(pos),
          },

          // program-num-bin-state.
          TokenizerState::Program(Program::Num(Num::Bin)) => match ch {
            _ => todo!(),
          },

          // program-num-oct-state.
          TokenizerState::Program(Program::Num(Num::Oct)) => match ch {
            _ => todo!(),
          },

          // program-num-hex-state.
          TokenizerState::Program(Program::Num(Num::Hex)) => match ch {
            _ => todo!(),
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
            _ => todo!(),
          },

          // program-num-expo-sign-state.
          TokenizerState::Program(Program::Num(Num::Expo(Expo::Sign))) => {
            match ch {
              _ => todo!(),
            }
          }

          // program-num-expo-digits-state.
          TokenizerState::Program(Program::Num(Num::Expo(Expo::Digits))) => {
            match ch {
              _ => todo!(),
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
            'b' => todo!(),
            'o' => todo!(),
            'x' => todo!(),
            c if is!(ident_continue c) => {
              self.cursor.next();
            }
            _ => return self.scan(pos),
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
              c if is!(quote c) => {
                self.state = TokenizerState::Style(Style::Quote);
              }
              c if is!(delim c) => {
                self.state = TokenizerState::Style(Style::Delim);
              }
              _ => todo!(),
            }
          }

          // style-quote-state.
          TokenizerState::Style(Style::Quote) => match ch {
            c if is!(quote_double c) | is!(quote_single c) => todo!(),
            _ => todo!(),
          },

          // style-state-unimplemented-yet.
          _ => panic!("State::Style = {:?}", self.state),
        },

        // template-mode.
        TokenizerMode::Template => match self.state {
          // template-data-state.
          TokenizerState::Template(Template::Data) => {
            pos = self.cursor.pos();

            match ch {
              ';' => {
                self.switch(TokenizerMode::Program);

                self.state = TokenizerState::Program(Program::Punctuation);
              }
              '<' => {
                self.cursor.next();

                self.state = TokenizerState::Template(Template::TagOpen);
              }
              _ => {
                self.cursor.next();

                self.state = TokenizerState::Template(Template::Character);

                return self.scan(pos);
              }
            }
          }

          // template-raw-text-state.
          TokenizerState::Template(Template::RawText) => match ch {
            _ => todo!(),
          },

          // template-tag-open-state.
          TokenizerState::Template(Template::TagOpen) => match ch {
            _ => todo!(),
          },

          // template-tag-open-end-state.
          TokenizerState::Template(Template::TagOpenEnd) => match ch {
            _ => todo!(),
          },

          // template-tag-name-state.
          TokenizerState::Template(Template::TagName) => match ch {
            _ => todo!(),
          },

          // template-before-attribute-name-state.
          TokenizerState::Template(Template::BeforeAttributeName) => match ch {
            _ => todo!(),
          },

          // template-attribute-name-state.
          TokenizerState::Template(Template::AttributeName) => match ch {
            _ => todo!(),
          },

          // template-after-attribute-name-state.
          TokenizerState::Template(Template::AfterAttributeName) => match ch {
            _ => todo!(),
          },

          // template-before-attribute-value-state.
          TokenizerState::Template(Template::BeforeAttributeValue) => {
            match ch {
              _ => todo!(),
            }
          }

          // template-attribute-value-quoted-no-state.
          TokenizerState::Template(Template::AttributeValue(Quoted::No)) => {
            match ch {
              _ => todo!(),
            }
          }

          // template-after-attribute-value-state.
          TokenizerState::Template(Template::AfterAttributeValue) => match ch {
            _ => todo!(),
          },

          // template-tag-self-closing-start-state.
          TokenizerState::Template(Template::TagSelfClosingStart) => match ch {
            _ => todo!(),
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
        Some(TokenKind::Program(token::Program::Int(
          source.to_string(),
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
          unimplemented!()
        }
      }
      TokenizerState::Program(Program::Char) => {
        todo!()
      }
      TokenizerState::Program(Program::Str) => {
        todo!()
      }

      TokenizerState::Template(Template::Character) => {
        Some(TokenKind::Template(token::Template::Character(
          source.chars().next().unwrap_or_default(),
        )))
      }
      TokenizerState::Template(Template::TagOpen) => todo!(),

      TokenizerState::Style(Style::Delim) => {
        Some(TokenKind::Style(token::Style::Delim(
          source.chars().next().map(style::Delim::from).unwrap(),
        )))
      }

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
pub fn tokenize(source: &str) -> Result<Vec<Token>> {
  Tokenizer::new(source).tokenize()
}

#[cfg(test)]
mod tests {
  use super::Tokenizer;

  use zor_token::token::program::{Base, Group, Kw, Punctuation};
  use zor_token::token::{Program, Template, TokenKind};

  #[test]
  fn tokenize_tokens() {
    let source = "return 1 + 2 ;";
    let mut tokenizer = Tokenizer::new(source);
    let actual = tokenizer.tokenize().unwrap();

    println!("{:?}", actual);

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

  #[test]
  fn tokenize_program_empty() {
    let source = "";
    let mut tokenizer = Tokenizer::new(source);

    assert_eq!(tokenizer.next().unwrap().kind, TokenKind::Eof);
  }

  #[test]
  fn tokenize_program_line_comments() {
    let source = "-- this is a line comments.";
    let mut tokenizer = Tokenizer::new(source);

    assert_eq!(tokenizer.next().unwrap().kind, TokenKind::Eof);
  }

  #[test]
  fn tokenize_program_integers() {
    let source = "0";
    let mut tokenizer = Tokenizer::new(source);

    assert_eq!(
      tokenizer.next().unwrap().kind,
      TokenKind::Program(Program::Int(String::from("0"), Base::Dec))
    );

    assert_eq!(tokenizer.next().unwrap().kind, TokenKind::Eof);
  }

  #[test]
  fn tokenize_program_groups() {
    let source = "( ) [ ] { }";
    let mut tokenizer = Tokenizer::new(source);

    assert_eq!(
      tokenizer.next().unwrap().kind,
      TokenKind::Program(Program::Group(Group::ParenOpen)),
    );

    assert_eq!(
      tokenizer.next().unwrap().kind,
      TokenKind::Program(Program::Group(Group::ParenClose)),
    );

    assert_eq!(
      tokenizer.next().unwrap().kind,
      TokenKind::Program(Program::Group(Group::BracketOpen)),
    );

    assert_eq!(
      tokenizer.next().unwrap().kind,
      TokenKind::Program(Program::Group(Group::BracketClose)),
    );

    assert_eq!(
      tokenizer.next().unwrap().kind,
      TokenKind::Program(Program::Group(Group::BraceOpen)),
    );

    assert_eq!(
      tokenizer.next().unwrap().kind,
      TokenKind::Program(Program::Group(Group::BraceClose)),
    );

    assert_eq!(tokenizer.next().unwrap().kind, TokenKind::Eof);
  }

  #[test]
  fn tokenize_template_characters() {
    let source = "::= hello ;";
    let mut tokenizer = Tokenizer::new(source);

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
}
