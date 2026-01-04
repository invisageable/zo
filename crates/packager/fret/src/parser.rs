//! Hand-written recursive descent parser for the fret.oz configuration format.
//!
//! This parser transforms tokens into a ProjectConfig struct using a simple,
//! direct parsing strategy with clear error messages.

use crate::lexer::Lexer;
use crate::token::{Token, TokenKind};
use crate::types::{ProjectConfig, StageError, Version};

use std::path::PathBuf;

/// Parse error with source location information.
#[derive(Debug)]
pub struct ParseError {
  pub message: String,
  pub line: usize,
  pub column: usize,
  pub snippet: String,
}

/// Parser for fret.oz configuration files.
///
/// Implements a recursive descent parser that builds ProjectConfig
/// from the token stream with minimal allocations.
pub struct Parser<'src> {
  /// The lexer providing tokens
  lexer: Lexer<'src>,
  /// The source text for error reporting
  source: &'src str,
  /// Current token
  current: Token,
  /// Previous token for error reporting
  previous: Token,
}
impl<'src> Parser<'src> {
  /// Create a new parser for the given source text.
  pub fn new(source: &'src str) -> Self {
    let mut lexer = Lexer::new(source);
    let current = lexer.next_token();
    let previous = Token::new(TokenKind::Eof, 0, 0);

    Self {
      lexer,
      source,
      current,
      previous,
    }
  }

  /// Parse the configuration file and return a ProjectConfig.
  pub fn parse(mut self) -> Result<ProjectConfig, StageError> {
    // Skip any leading comments/whitespace
    while self.current.kind != TokenKind::At && !self.is_at_end() {
      self.advance();
    }

    // Expect @pack directive
    if !self.check(TokenKind::At) {
      return Err(self.error("Expected @pack directive"));
    }

    self.consume(TokenKind::At, "Expected '@'")?;
    self.consume(TokenKind::Pack, "Expected 'pack' keyword")?;

    // Parse optional namespace segments: { ":" identifier }
    while self.check(TokenKind::Colon) {
      self.advance(); // consume ':'
      self.consume(TokenKind::Identifier, "Expected identifier after ':'")?;
    }

    self.consume(TokenKind::Equal, "Expected '=' after directive path")?;
    self.consume(TokenKind::LeftParen, "Expected '(' after '='")?;

    // Parse package fields
    let mut name = None;
    let mut version = None;
    let mut authors = Vec::new();
    let mut license = None;
    let mut entry_point = None;
    let mut source_dir = None;
    let mut binary_name = None;
    let mut optimization_level = 0;
    let mut debug_symbols = true;

    // Parse key-value pairs
    while !self.check(TokenKind::RightParen) && !self.is_at_end() {
      let key_token =
        self.consume(TokenKind::Identifier, "Expected field name")?;
      let key = key_token.lexeme(self.source);

      self.consume(TokenKind::Colon, "Expected ':' after field name")?;

      match key {
        "name" => {
          let value =
            self.consume_string("Expected string value for 'name'")?;
          name = Some(value);
        }
        "version" => {
          let value =
            self.consume_string("Expected string value for 'version'")?;
          version = Some(self.parse_version(&value)?);
        }
        "authors" => {
          authors = self.parse_string_array()?;
        }
        "license" => {
          let value =
            self.consume_string("Expected string value for 'license'")?;
          license = Some(value);
        }
        "entry_point" => {
          let value =
            self.consume_string("Expected string value for 'entry_point'")?;
          entry_point = Some(PathBuf::from(value));
        }
        "source_dir" => {
          let value =
            self.consume_string("Expected string value for 'source_dir'")?;
          source_dir = Some(PathBuf::from(value));
        }
        "binary_name" => {
          let value =
            self.consume_string("Expected string value for 'binary_name'")?;
          binary_name = Some(value);
        }
        "optimization_level" => {
          let value = self
            .consume_number("Expected number value for 'optimization_level'")?;
          optimization_level = value as u8;
        }
        "debug_symbols" => {
          let value =
            self.consume_bool("Expected boolean value for 'debug_symbols'")?;
          debug_symbols = value;
        }
        _ => {
          return Err(self.error(&format!("Unknown field '{key}'")));
        }
      }

      // Check for comma or closing paren
      if !self.check(TokenKind::RightParen) {
        self.consume(TokenKind::Comma, "Expected ',' or ')'")?;
      }
    }

    self.consume(TokenKind::RightParen, "Expected ')' to close directive")?;

    // Validate required fields
    let name =
      name.ok_or_else(|| self.error("Missing required field 'name'"))?;
    let version = version.unwrap_or(Version {
      major: 0,
      minor: 1,
      patch: 0,
    });

    // Set defaults
    let entry_point =
      entry_point.unwrap_or_else(|| PathBuf::from("src/main.zo"));
    let source_dir = source_dir.unwrap_or_else(|| PathBuf::from("src"));
    let binary_name = binary_name.unwrap_or_else(|| name.clone());

    Ok(ProjectConfig {
      name,
      version,
      entry_point,
      source_dir,
      binary_name,
      optimization_level,
      debug_symbols,
      authors,
      license,
    })
  }

  /// Parse a version string like "1.2.3".
  fn parse_version(&self, version_str: &str) -> Result<Version, StageError> {
    let parts: Vec<&str> = version_str.split('.').collect();

    if parts.len() != 3 {
      return Err(self.error("Version must be in format 'major.minor.patch'"));
    }

    let major = parts[0]
      .parse::<u16>()
      .map_err(|_| self.error("Invalid major version number"))?;
    let minor = parts[1]
      .parse::<u16>()
      .map_err(|_| self.error("Invalid minor version number"))?;
    let patch = parts[2]
      .parse::<u16>()
      .map_err(|_| self.error("Invalid patch version number"))?;

    Ok(Version {
      major,
      minor,
      patch,
    })
  }

  /// Parse an array of strings like ["foo", "bar"].
  fn parse_string_array(&mut self) -> Result<Vec<String>, StageError> {
    self.consume(TokenKind::LeftBracket, "Expected '['")?;

    let mut values = Vec::new();

    while !self.check(TokenKind::RightBracket) && !self.is_at_end() {
      let value = self.consume_string("Expected string in array")?;
      values.push(value);

      if !self.check(TokenKind::RightBracket) {
        self.consume(TokenKind::Comma, "Expected ',' or ']'")?;
      }
    }

    self.consume(TokenKind::RightBracket, "Expected ']'")?;

    Ok(values)
  }

  /// Consume a string token and return its value.
  fn consume_string(&mut self, error_msg: &str) -> Result<String, StageError> {
    let token = self.consume(TokenKind::String, error_msg)?;
    let lexeme = token.lexeme(self.source);
    let content = &lexeme[1..lexeme.len() - 1];

    Ok(self.unescape_string(content))
  }

  /// Consume a number token and return its value.
  fn consume_number(&mut self, error_msg: &str) -> Result<f64, StageError> {
    let token = self.consume(TokenKind::Number, error_msg)?;
    let lexeme = token.lexeme(self.source);

    lexeme
      .parse::<f64>()
      .map_err(|_| self.error("Invalid number"))
  }

  /// Consume a boolean value (identifier "true" or "false").
  fn consume_bool(&mut self, error_msg: &str) -> Result<bool, StageError> {
    let token = self.consume(TokenKind::Identifier, error_msg)?;
    let lexeme = token.lexeme(self.source);

    match lexeme {
      "true" => Ok(true),
      "false" => Ok(false),
      _ => Err(self.error("Expected 'true' or 'false'")),
    }
  }

  /// Unescape a string value.
  fn unescape_string(&self, s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();

    while let Some(ch) = chars.next() {
      if ch == '\\' {
        if let Some(escaped) = chars.next() {
          match escaped {
            'n' => result.push('\n'),
            'r' => result.push('\r'),
            't' => result.push('\t'),
            '\\' => result.push('\\'),
            '"' => result.push('"'),
            _ => {
              result.push('\\');
              result.push(escaped);
            }
          }
        } else {
          result.push('\\');
        }
      } else {
        result.push(ch);
      }
    }

    result
  }

  /// Check if the current token is of the given kind.
  #[inline]
  fn check(&self, kind: TokenKind) -> bool {
    self.current.kind == kind
  }

  /// Check if we're at the end of the token stream.
  #[inline]
  fn is_at_end(&self) -> bool {
    self.current.kind == TokenKind::Eof
  }

  /// Advance to the next token.
  fn advance(&mut self) {
    self.previous = self.current;
    self.current = self.lexer.next_token();
  }

  /// Consume a token of the expected kind or return an error.
  fn consume(
    &mut self,
    kind: TokenKind,
    error_msg: &str,
  ) -> Result<Token, StageError> {
    if self.check(kind) {
      let token = self.current;
      self.advance();
      Ok(token)
    } else {
      Err(self.error(error_msg))
    }
  }

  /// Create an error with source location information.
  fn error(&self, message: &str) -> StageError {
    let (line, column) = self.get_location(self.current.start);
    let error_msg = format!(
      "Parse error at line {}, column {}: {}\n{}",
      line,
      column,
      message,
      self.get_error_snippet(line, column)
    );
    StageError::ConfigParse(error_msg)
  }

  /// Get the line and column for a byte offset.
  fn get_location(&self, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut column = 1;

    for (i, ch) in self.source.chars().enumerate() {
      if i >= offset {
        break;
      }
      if ch == '\n' {
        line += 1;
        column = 1;
      } else {
        column += 1;
      }
    }

    (line, column)
  }

  /// Get a snippet of the source around the error.
  fn get_error_snippet(&self, line: usize, column: usize) -> String {
    let lines: Vec<&str> = self.source.lines().collect();

    if line > 0 && line <= lines.len() {
      let line_text = lines[line - 1];
      let pointer = " ".repeat(column - 1) + "^";
      format!("{}\n{}", line_text, pointer)
    } else {
      String::new()
    }
  }
}

/// Parse a fret.oz configuration file from the given source text.
pub fn parse_config(source: &str) -> Result<ProjectConfig, StageError> {
  let parser = Parser::new(source);
  parser.parse()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_basic_package() {
    let source = r#"
@pack = (
  name: "my-project",
  version: "0.1.0",
  authors: ["invisageable <you@example.com>"],
  license: "MIT OR Apache-2.0",
)
"#;

    let config = parse_config(source).unwrap();
    assert_eq!(config.name, "my-project");
    assert_eq!(config.version.major, 0);
    assert_eq!(config.version.minor, 1);
    assert_eq!(config.version.patch, 0);
  }

  #[test]
  fn test_parse_with_comments() {
    let source = r#"
-- This is my project configuration
@pack = (
  name: "test", -- The project name
  version: "1.0.0",
)
"#;

    let config = parse_config(source).unwrap();
    assert_eq!(config.name, "test");
    assert_eq!(config.version.major, 1);
  }

  #[test]
  fn test_parse_with_all_fields() {
    let source = r#"
@pack = (
  name: "full-project",
  version: "2.3.4",
  entry_point: "src/app.zo",
  source_dir: "source",
  binary_name: "my-app",
  optimization_level: 3,
  debug_symbols: false,
)
"#;

    let config = parse_config(source).unwrap();
    assert_eq!(config.name, "full-project");
    assert_eq!(config.version.major, 2);
    assert_eq!(config.version.minor, 3);
    assert_eq!(config.version.patch, 4);
    assert_eq!(config.entry_point, PathBuf::from("src/app.zo"));
    assert_eq!(config.source_dir, PathBuf::from("source"));
    assert_eq!(config.binary_name, "my-app");
    assert_eq!(config.optimization_level, 3);
    assert!(!config.debug_symbols);
  }

  #[test]
  fn test_error_missing_name() {
    let source = r#"
@pack = (
  version: "1.0.0",
)
"#;

    let result = parse_config(source);
    assert!(result.is_err());
  }

  #[test]
  fn test_namespaced_directive() {
    let source = r#"
@pack:zo:release = (
  name: "namespaced-project",
  version: "2.0.0",
  authors: ["Test Author"],
)
"#;

    let config = parse_config(source).unwrap();
    assert_eq!(config.name, "namespaced-project");
    assert_eq!(config.version.major, 2);
    assert_eq!(config.version.minor, 0);
    assert_eq!(config.version.patch, 0);
  }
}
