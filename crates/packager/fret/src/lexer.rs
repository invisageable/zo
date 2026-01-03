/// Zero-allocation lexer for the fret.oz configuration format.
///
/// This lexer implements on-demand tokenization with minimal allocations.
/// It follows the "Speed is Law" principle by operating directly on the
/// source text and producing tokens that reference byte ranges.
use crate::token::{Token, TokenKind};

/// A high-performance lexer for the fret.oz format.
///
/// The lexer operates on a byte slice and produces tokens on-demand,
/// avoiding allocations by storing only byte offsets into the source.
pub struct Lexer<'src> {
  /// The source text as a byte slice
  source: &'src [u8],
  /// Current byte position in the source
  current: usize,
  /// Start position of the current token
  start: usize,
}

impl<'src> Lexer<'src> {
  /// Create a new lexer for the given source text.
  #[inline]
  pub fn new(source: &'src str) -> Self {
    Self {
      source: source.as_bytes(),
      current: 0,
      start: 0,
    }
  }

  /// Get the next token from the source.
  pub fn next_token(&mut self) -> Token {
    self.skip_whitespace_and_comments();

    if self.is_at_end() {
      return Token::new(TokenKind::Eof, self.current, self.current);
    }

    self.start = self.current;
    let ch = self.advance();

    match ch {
      b'@' => self.make_token(TokenKind::At),
      b'(' => self.make_token(TokenKind::LeftParen),
      b')' => self.make_token(TokenKind::RightParen),
      b'[' => self.make_token(TokenKind::LeftBracket),
      b']' => self.make_token(TokenKind::RightBracket),
      b',' => self.make_token(TokenKind::Comma),
      b':' => self.make_token(TokenKind::Colon),
      b'=' => self.make_token(TokenKind::Equal),
      b'"' => self.string(),
      b'0'..=b'9' => self.number(),
      b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.identifier_or_keyword(),
      _ => self.make_token(TokenKind::Error),
    }
  }

  /// Skip whitespace and comments.
  fn skip_whitespace_and_comments(&mut self) {
    loop {
      if self.is_at_end() {
        break;
      }

      match self.peek() {
        b' ' | b'\r' | b'\t' | b'\n' => {
          self.advance();
        }
        b'-' => {
          if self.peek_next() == b'-' {
            // Skip line comment
            while self.peek() != b'\n' && !self.is_at_end() {
              self.advance();
            }
          } else {
            break;
          }
        }
        _ => break,
      }
    }
  }

  /// Parse a string literal.
  fn string(&mut self) -> Token {
    // Skip past the opening quote
    while self.peek() != b'"' && !self.is_at_end() {
      if self.peek() == b'\\' {
        // Skip escaped character
        self.advance();
        if !self.is_at_end() {
          self.advance();
        }
      } else {
        self.advance();
      }
    }

    if self.is_at_end() {
      return self.make_token(TokenKind::Error);
    }

    // Consume the closing quote
    self.advance();
    self.make_token(TokenKind::String)
  }

  /// Parse a number literal.
  fn number(&mut self) -> Token {
    while self.peek().is_ascii_digit() {
      self.advance();
    }

    // Handle decimal numbers
    if self.peek() == b'.' && self.peek_next().is_ascii_digit() {
      self.advance(); // consume '.'
      while self.peek().is_ascii_digit() {
        self.advance();
      }
    }

    self.make_token(TokenKind::Number)
  }

  /// Parse an identifier or keyword.
  fn identifier_or_keyword(&mut self) -> Token {
    while self.peek().is_ascii_alphanumeric()
      || self.peek() == b'_'
      || self.peek() == b'-'
    {
      self.advance();
    }

    // Check if this is the 'pack' keyword
    let lexeme = &self.source[self.start..self.current];
    if lexeme == b"pack" {
      self.make_token(TokenKind::Pack)
    } else {
      self.make_token(TokenKind::Identifier)
    }
  }

  /// Check if we've reached the end of the source.
  #[inline]
  fn is_at_end(&self) -> bool {
    self.current >= self.source.len()
  }

  /// Advance to the next character and return the current one.
  #[inline]
  fn advance(&mut self) -> u8 {
    let ch = self.source[self.current];
    self.current += 1;
    ch
  }

  /// Peek at the current character without advancing.
  #[inline]
  fn peek(&self) -> u8 {
    if self.is_at_end() {
      b'\0'
    } else {
      self.source[self.current]
    }
  }

  /// Peek at the next character without advancing.
  #[inline]
  fn peek_next(&self) -> u8 {
    if self.current + 1 >= self.source.len() {
      b'\0'
    } else {
      self.source[self.current + 1]
    }
  }

  /// Create a token of the given kind with the current range.
  #[inline]
  fn make_token(&self, kind: TokenKind) -> Token {
    Token::new(kind, self.start, self.current)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_basic_tokens() {
    let source = "@pack = (name: \"test\")";
    let mut lexer = Lexer::new(source);

    assert_eq!(lexer.next_token().kind, TokenKind::At);
    assert_eq!(lexer.next_token().kind, TokenKind::Pack);
    assert_eq!(lexer.next_token().kind, TokenKind::Equal);
    assert_eq!(lexer.next_token().kind, TokenKind::LeftParen);
    assert_eq!(lexer.next_token().kind, TokenKind::Identifier);
    assert_eq!(lexer.next_token().kind, TokenKind::Colon);
    assert_eq!(lexer.next_token().kind, TokenKind::String);
    assert_eq!(lexer.next_token().kind, TokenKind::RightParen);
    assert_eq!(lexer.next_token().kind, TokenKind::Eof);
  }

  #[test]
  fn test_comments() {
    let source = "-- comment\n@pack = -- another comment\n(name: \"test\")";
    let mut lexer = Lexer::new(source);

    assert_eq!(lexer.next_token().kind, TokenKind::At);
    assert_eq!(lexer.next_token().kind, TokenKind::Pack);
    assert_eq!(lexer.next_token().kind, TokenKind::Equal);
    assert_eq!(lexer.next_token().kind, TokenKind::LeftParen);
  }
}
