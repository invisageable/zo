use super::cursor::Cursor;
use super::state::TokenizerState;

use zor_reporter::Result;
use zor_token::token::Token;

/// The representation of a tokenizer mode.
#[derive(Debug)]
enum TokenizerMode {
  /// A programming mode.
  Program,
  /// A templating mode.
  Template,
  /// A styling mode.
  Style,
}

/// The representation of a Tokenizer.
#[derive(Debug)]
pub struct Tokenizer {
  /// A cursor.
  cursor: Cursor,
  /// A tokenizer's state.
  state: TokenizerState,
  /// A tokenizer's mode.
  mode: TokenizerMode,
}

impl Tokenizer {
  /// Creates a new tokenizer.
  pub fn new(source: &str) -> Self {
    Self {
      cursor: Cursor::new(source),
      state: TokenizerState::ProgramData,
      mode: TokenizerMode::Program,
    }
  }

  /// Tokenizes a source code into a stream of tokens.
  pub fn tokenize(&mut self) -> Result<Vec<Token>> {
    let mut tokens = Vec::with_capacity(0usize);

    while self.cursor.peek().is_some() {
      tokens.push(self.next()?);
    }

    Ok(tokens)
  }

  pub fn next(&mut self) -> Result<Token> {
    todo!()
  }
}

/// Transforms a source code into a stream of tokens.
pub fn tokenize(source: &str) -> Result<Vec<Token>> {
  Tokenizer::new(source).tokenize()
}
