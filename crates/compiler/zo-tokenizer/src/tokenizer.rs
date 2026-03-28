use zo_error::{Error, ErrorKind};
use zo_interner::Interner;
use zo_reporter::report_error;
use zo_span::Span;
use zo_token::{InterpSegment, LiteralStore, Token, TokenBuffer};

use serde::Serialize;

/// The complete result of tokenization
#[derive(Serialize)]
pub struct TokenizationResult {
  pub tokens: TokenBuffer,
  pub literals: LiteralStore,
  pub interner: Interner,
  /// Store source length for validation.
  pub source_len: usize,
}

/// Delimiter kinds for matching
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DelimiterKind {
  /// The parenthesis delimiters — `(`, `)`.
  Paren,
  /// The curly braces delimiters — `{`, `}`.
  Brace,
  /// The brackets delimiters — `[`, `]`.
  Bracket,
}

/// Delimiter info for error reporting
#[derive(Debug, Clone, Copy)]
struct DelimiterInfo {
  kind: DelimiterKind,
  position: u32,
}

/// Packed mode state - entire state in single u32
#[derive(Debug, Clone, Copy)]
struct ModeState(u32);

impl ModeState {
  const CODE: u32 = 0;
  const TEMPLATE: u32 = 1;
  const MODE_MASK: u32 = 0x3;
  const DEPTH_SHIFT: u32 = 2;
  const DEPTH_MASK: u32 = 0x3FFF;
  const DEPTH_FIELD: u32 = 0xFFFC;
  const TEXT_BIT: u32 = 16;

  #[inline(always)]
  const fn new() -> Self {
    Self(0)
  }

  #[inline(always)]
  fn mode(self) -> u32 {
    self.0 & Self::MODE_MASK
  }

  #[inline(always)]
  fn is_template(self) -> bool {
    (self.0 & Self::MODE_MASK) == Self::TEMPLATE
  }

  #[inline(always)]
  fn brace_depth(self) -> u32 {
    (self.0 >> Self::DEPTH_SHIFT) & Self::DEPTH_MASK
  }

  #[inline(always)]
  fn template_text_mode(self) -> bool {
    (self.0 >> Self::TEXT_BIT) & 0x1 == 1
  }

  #[inline(always)]
  fn set_mode(&mut self, mode: u32) {
    self.0 = (self.0 & !Self::MODE_MASK) | (mode & Self::MODE_MASK);
  }

  #[inline(always)]
  fn set_template_text(&mut self, enabled: bool) {
    if enabled {
      self.0 |= 1 << Self::TEXT_BIT;
    } else {
      self.0 &= !(1 << Self::TEXT_BIT);
    }
  }

  #[inline(always)]
  fn inc_brace_depth(&mut self) {
    let depth = self.brace_depth();

    if depth < Self::DEPTH_MASK {
      self.0 =
        (self.0 & !Self::DEPTH_FIELD) | ((depth + 1) << Self::DEPTH_SHIFT);
    }
  }

  #[inline(always)]
  fn dec_brace_depth(&mut self) {
    let depth = self.brace_depth();

    if depth > 0 {
      self.0 =
        (self.0 & !Self::DEPTH_FIELD) | ((depth - 1) << Self::DEPTH_SHIFT);
    }
  }
}

pub struct Tokenizer<'a> {
  source: &'a [u8],
  cursor: usize,
  tokens: TokenBuffer,
  literals: LiteralStore,
  interner: Interner,
  state: ModeState,
  delimiter_stack: Vec<DelimiterInfo>,
}

impl<'a> Tokenizer<'a> {
  const BYTES_PER_TOKEN: usize = 5;
  const TOKENS_PER_LITERAL: usize = 3;

  pub fn new(source: &'a str) -> Self {
    let bytes = source.as_bytes();
    let estimated_tokens = bytes.len() / Self::BYTES_PER_TOKEN;
    let estimated_literals = estimated_tokens / Self::TOKENS_PER_LITERAL;

    Self {
      source: bytes,
      cursor: 0,
      tokens: TokenBuffer::with_capacity(estimated_tokens),
      literals: LiteralStore::with_capacity(estimated_literals),
      interner: Interner::new(),
      state: ModeState::new(),
      delimiter_stack: Vec::new(),
    }
  }

  #[inline(always)]
  fn current(&self) -> u8 {
    static ZERO_BYTE: [u8; 1] = [0];

    let in_bounds = (self.cursor < self.source.len()) as usize;

    unsafe {
      // when in_bounds=1: read from source[cursor].
      // when in_bounds=0: read from ZERO_BYTE[0].
      let base_ptr = [ZERO_BYTE.as_ptr(), self.source.as_ptr()][in_bounds];
      let offset = self.cursor * in_bounds;

      *base_ptr.add(offset)
    }
  }

  #[inline(always)]
  fn peek(&self, offset: usize) -> u8 {
    let pos = self.cursor + offset;
    let in_bounds = (pos < self.source.len()) as usize;

    unsafe { *self.source.as_ptr().add(pos * in_bounds) }
  }

  #[inline(always)]
  fn advance(&mut self) -> u8 {
    let ch = self.current();

    self.cursor += (self.cursor < self.source.len()) as usize;

    ch
  }

  #[inline(never)]
  fn skip_whitespace(&mut self) {
    // Unrolled loop for better performance
    while self.cursor + 4 <= self.source.len() {
      // Use read_unaligned for safe access regardless of alignment
      let chunk = unsafe {
        (self.source.as_ptr().add(self.cursor) as *const u32).read_unaligned()
      };

      // Check if any byte in the chunk is NOT whitespace
      let is_ws = ((chunk & 0xFF) as u8).is_ascii_whitespace() as u32
        | (((chunk >> 8) & 0xFF) as u8).is_ascii_whitespace() as u32
        | (((chunk >> 16) & 0xFF) as u8).is_ascii_whitespace() as u32
        | (((chunk >> 24) & 0xFF) as u8).is_ascii_whitespace() as u32;

      if is_ws != 0xF {
        break;
      }

      self.cursor += 4;
    }

    // Handle remainder
    while self.cursor < self.source.len()
      && self.current().is_ascii_whitespace()
    {
      self.cursor += 1;
    }
  }

  #[inline(always)]
  fn skip_line_comment(&mut self) {
    self.advance();

    while self.cursor < self.source.len() {
      let ch = self.current();

      if ch == b'\n' {
        break;
      }

      self.cursor += 1;
    }
  }

  #[inline(always)]
  fn skip_block_comment(&mut self, start: usize) {
    self.advance();

    let mut depth = 1; // track nesting depth.

    while self.cursor < self.source.len() && depth > 0 {
      if self.current() == b'-' && self.peek(1) == b'*' {
        // found nested comment start.
        depth += 1;
        self.cursor += 2;
      } else if self.current() == b'*' && self.peek(1) == b'-' {
        // found comment end.
        depth -= 1;
        self.cursor += 2;
      } else {
        self.cursor += 1;
      }
    }

    if depth > 0 {
      // Report unterminated block comment error
      report_error(Error::new(
        ErrorKind::UnterminatedBlockComment,
        Span {
          start: start as u32,
          len: (self.cursor - start) as u16,
        },
      ));
    }
  }

  #[inline(always)]
  fn scan_identifier(&mut self, start: usize) {
    // Fast path for common identifier lengths
    if self.cursor + 8 <= self.source.len() {
      // Check 8 bytes at once
      let mut end = self.cursor;

      while end + 8 <= self.source.len() {
        // Use read_unaligned for safe access
        let chunk = unsafe {
          (self.source.as_ptr().add(end) as *const u64).read_unaligned()
        };

        // Check if all bytes are valid identifier chars
        let mut valid = true;

        for i in 0..8 {
          let b = ((chunk >> (i * 8)) & 0xFF) as u8;

          if !b.is_ascii_alphanumeric() && b != b'_' {
            end += i;
            valid = false;

            break;
          }
        }

        if !valid {
          break;
        }

        end += 8;
      }

      self.cursor = end;
    }

    // Handle remainder
    while self.cursor < self.source.len() {
      let ch = self.current();

      if !ch.is_ascii_alphanumeric() && ch != b'_' {
        break;
      }

      self.cursor += 1;
    }

    let len = (self.cursor - start) as u16;
    let bytes = &self.source[start..self.cursor];

    // In template mode inside tag markup (between < and >),
    // all identifiers are plain Ident — never keywords.
    // This prevents `type`, `for`, `loop` etc. from losing
    // their string value when used as HTML attribute names.
    // Exclude interpolation blocks (brace_depth > 0) where
    // keywords like `if`/`else` must remain keywords.
    let in_tag_markup = self.state.is_template()
      && !self.state.template_text_mode()
      && self.state.brace_depth() == 0;

    // Perfect hash for keywords using first 2 bytes
    let kind = if in_tag_markup {
      Token::Ident
    } else if len <= 8 {
      match len {
        2 => {
          // Use read_unaligned for safe access
          let key = unsafe { (bytes.as_ptr() as *const u16).read_unaligned() };

          match key {
            0x6E66 => Token::Fn,     // "fn"
            0x6E46 => Token::FnType, // "Fn"
            0x6669 => Token::If,     // "if"
            0x7361 => Token::As,     // "as"
            0x7369 => Token::Is,     // "is"
            0x3873 => Token::S8Type, // "s8"
            0x3875 => Token::U8Type, // "u8"
            _ => Token::Ident,
          }
        }
        3 if bytes == b"fun" => Token::Fun,
        3 if bytes == b"imu" => Token::Imu,
        3 if bytes == b"mut" => Token::Mut,
        3 if bytes == b"pub" => Token::Pub,
        3 if bytes == b"for" => Token::For,
        3 if bytes == b"ext" => Token::Ext,
        3 if bytes == b"val" => Token::Val,
        3 if bytes == b"raw" => Token::Raw,
        3 if bytes == b"and" => Token::And,
        3 if bytes == b"int" => Token::IntType,
        3 if bytes == b"str" => Token::StrType,
        3 if bytes == b"s16" => Token::S16Type,
        3 if bytes == b"s32" => Token::S32Type,
        3 if bytes == b"s64" => Token::S64Type,
        3 if bytes == b"u16" => Token::U16Type,
        3 if bytes == b"u32" => Token::U32Type,
        3 if bytes == b"u64" => Token::U64Type,
        3 if bytes == b"f32" => Token::F32Type,
        3 if bytes == b"f64" => Token::F64Type,
        4 if bytes == b"else" => Token::Else,
        4 if bytes == b"true" => Token::True,
        4 if bytes == b"enum" => Token::Enum,
        4 if bytes == b"type" => Token::Type,
        4 if bytes == b"when" => Token::When,
        4 if bytes == b"pack" => Token::Pack,
        4 if bytes == b"load" => Token::Load,
        4 if bytes == b"wasm" => Token::Wasm,
        4 if bytes == b"loop" => Token::Loop,
        4 if bytes == b"self" => Token::SelfLower,
        4 if bytes == b"Self" => Token::SelfUpper,
        4 if bytes == b"bool" => Token::BoolType,
        4 if bytes == b"char" => Token::CharType,
        4 if bytes == b"uint" => Token::UintType,
        5 if bytes == b"while" => Token::While,
        5 if bytes == b"break" => Token::Break,
        5 if bytes == b"false" => Token::False,
        5 if bytes == b"match" => Token::Match,
        5 if bytes == b"apply" => Token::Apply,
        5 if bytes == b"state" => Token::State,
        5 if bytes == b"group" => Token::Group,
        5 if bytes == b"spawn" => Token::Spawn,
        5 if bytes == b"await" => Token::Await,
        5 if bytes == b"float" => Token::FloatType,
        5 if bytes == b"bytes" => Token::BytesType,
        6 if bytes == b"return" => Token::Return,
        6 if bytes == b"struct" => Token::Struct,
        7 if bytes == b"nursery" => Token::Nursery,
        8 if bytes == b"continue" => Token::Continue,
        8 if bytes == b"abstract" => Token::Abstract,
        _ => Token::Ident,
      }
    } else {
      Token::Ident
    };

    if kind == Token::Ident {
      // Intern the identifier text (like Carbon does)
      let text = std::str::from_utf8(bytes).unwrap_or("");
      let symbol = self.interner.intern(text);
      let id = self.literals.push_identifier(symbol);

      self.tokens.push_with_literal(kind, start as u32, len, id);
    } else {
      self.tokens.push(kind, start as u32, len);
    }
  }

  #[inline(always)]
  fn scan_template_text(&mut self) {
    let start = self.cursor;

    // Fast bulk scan for template text
    while self.cursor < self.source.len() {
      let ch = self.current();

      if ch == b'<' || ch == b'{' || ch == b';' || ch == b'}' {
        break;
      }

      self.cursor += 1;
    }

    if self.cursor > start {
      let len = (self.cursor - start) as u16;

      // Intern template text like we do for identifiers
      let bytes = &self.source[start..self.cursor];
      let text = std::str::from_utf8(bytes).unwrap_or("");
      let symbol = self.interner.intern(text);
      let id = self.literals.push_identifier(symbol);

      self
        .tokens
        .push_with_literal(Token::TemplateText, start as u32, len, id);
    }
  }

  fn scan_template_token(&mut self) {
    // When we're in brace depth > 0, we're inside an interpolation expression
    // and should tokenize as regular code
    if self.state.brace_depth() > 0 {
      self.scan_code_token();

      return;
    }

    if self.state.template_text_mode() {
      let ch = self.current();

      if ch != b'<' && ch != b'{' && ch != b';' && ch != b'}' {
        self.scan_template_text();

        return;
      }
    }

    let start = self.cursor;
    let ch = self.advance();

    match ch {
      b'<' => {
        self.state.set_template_text(false);

        if self.current() == b'>' {
          self.advance();
          self
            .tokens
            .push(Token::TemplateFragmentStart, start as u32, 2);
          self.state.set_template_text(true);
        } else if self.current() == b'/' && self.peek(1) == b'>' {
          self.cursor += 2;
          self
            .tokens
            .push(Token::TemplateFragmentEnd, start as u32, 3);
        } else {
          self.tokens.push(Token::LAngle, start as u32, 1);
        }
      }
      b'>' => {
        self.tokens.push(Token::RAngle, start as u32, 1);
        self.state.set_template_text(true);
      }
      b'{' => {
        self.delimiter_stack.push(DelimiterInfo {
          kind: DelimiterKind::Brace,
          position: start as u32,
        });
        self.tokens.push(Token::LBrace, start as u32, 1);
        self.state.inc_brace_depth();
      }
      b'}' => {
        self.check_closing_delimiter(DelimiterKind::Brace, start as u32);
        self.tokens.push(Token::RBrace, start as u32, 1);
        self.state.dec_brace_depth();
      }
      b';' => {
        self.tokens.push(Token::Semicolon, start as u32, 1);
        if self.state.brace_depth() == 0 {
          self.state.set_mode(ModeState::CODE);
        }
      }
      _ => {
        self.cursor = start;
        self.scan_code_token();
      }
    }
  }

  fn scan_code_token(&mut self) {
    let start = self.cursor;
    let ch = self.advance();

    match ch {
      b'0'..=b'9' => {
        self.cursor = start;

        self.scan_number();
      }
      // Check for base literals (b#, o#, x#) before regular identifiers
      b'b' | b'o' | b'x' if self.peek(0) == b'#' => {
        self.cursor = start;

        self.scan_number();
      }
      b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
        self.cursor = start;

        self.scan_identifier(start);
      }
      b'"' => {
        self.cursor = start;
        self.scan_string();
      }
      b'\'' => {
        self.cursor = start;
        self.scan_char();
      }
      b'`' => {
        self.cursor = start;
        self.scan_bytes();
      }
      b'$' => {
        // Check for raw string literal $"..."
        if self.current() == b'"' {
          self.advance(); // Skip the '"'
          self.scan_raw_string(start);
        } else {
          self.tokens.push(Token::Dollar, start as u32, 1);
        }
      }
      b':' => {
        if self.current() == b':' {
          self.advance();
          if self.current() == b'=' {
            self.advance();
            self.state.set_mode(ModeState::TEMPLATE);
            self.tokens.push(Token::TemplateAssign, start as u32, 3);
          } else {
            self.tokens.push(Token::ColonColon, start as u32, 2);
          }
        } else if self.current() == b'=' {
          self.advance();
          self.tokens.push(Token::ColonEq, start as u32, 2);
        } else {
          self.tokens.push(Token::Colon, start as u32, 1);
        }
      }
      b'=' => {
        if self.current() == b'=' {
          self.advance();
          self.tokens.push(Token::EqEq, start as u32, 2);
        } else if self.current() == b'>' {
          self.advance();
          self.tokens.push(Token::FatArrow, start as u32, 2);
        } else {
          self.tokens.push(Token::Eq, start as u32, 1);
        }
      }
      b'+' => {
        if self.current() == b'+' {
          self.advance();
          self.tokens.push(Token::PlusPlus, start as u32, 2);
        } else if self.current() == b'=' {
          self.advance();
          self.tokens.push(Token::PlusEq, start as u32, 2);
        } else {
          self.tokens.push(Token::Plus, start as u32, 1);
        }
      }
      b'-' => {
        if self.current() == b'=' {
          self.advance();
          self.tokens.push(Token::MinusEq, start as u32, 2);
        } else if self.current() == b'>' {
          self.advance();
          self.tokens.push(Token::Arrow, start as u32, 2);
        } else if self.current() == b'-' {
          self.skip_line_comment();
        } else if self.current() == b'!' {
          // Doc line comment: -! ...
          self.skip_line_comment();
        } else if self.current() == b'*' {
          self.skip_block_comment(start);
        } else {
          self.tokens.push(Token::Minus, start as u32, 1);
        }
      }
      b'*' => {
        if self.current() == b'=' {
          self.advance();
          self.tokens.push(Token::StarEq, start as u32, 2);
        } else {
          self.tokens.push(Token::Star, start as u32, 1);
        }
      }
      b'/' => {
        if self.current() == b'=' {
          self.advance();
          self.tokens.push(Token::SlashEq, start as u32, 2);
        } else {
          // In template mode, we need Slash2 for tags like </p> or />
          // In code mode, we need Slash for division operator
          let token_kind = if self.state.mode() == ModeState::TEMPLATE {
            Token::Slash2
          } else {
            Token::Slash
          };
          self.tokens.push(token_kind, start as u32, 1);
        }
      }
      b'%' => {
        if self.current() == b'=' {
          self.advance();
          self.tokens.push(Token::PercentEq, start as u32, 2);
        } else if self.current() == b'%' {
          self.advance();
          self.tokens.push(Token::Attribute, start as u32, 2);
        } else {
          self.tokens.push(Token::Percent, start as u32, 1);
        }
      }
      b'<' => {
        // Check for </> as TemplateType when not in template mode
        if !self.state.is_template()
          && self.current() == b'/'
          && self.peek(1) == b'>'
        {
          // Found </> - emit as TemplateType token
          self.cursor += 2; // Advance past />
          self.tokens.push(Token::TemplateType, start as u32, 3);
        } else if self.current() == b'=' {
          self.advance();
          self.tokens.push(Token::LtEq, start as u32, 2);
        } else if self.current() == b'<' {
          self.advance();
          if self.current() == b'=' {
            self.advance();
            self.tokens.push(Token::LShiftEq, start as u32, 3);
          } else {
            self.tokens.push(Token::LShift, start as u32, 2);
          }
        } else {
          self.tokens.push(Token::Lt, start as u32, 1);
        }
      }
      b'>' => {
        if self.current() == b'=' {
          self.advance();
          self.tokens.push(Token::GtEq, start as u32, 2);
        } else if self.current() == b'>' {
          self.advance();
          if self.current() == b'=' {
            self.advance();
            self.tokens.push(Token::RShiftEq, start as u32, 3);
          } else {
            self.tokens.push(Token::RShift, start as u32, 2);
          }
        } else {
          self.tokens.push(Token::Gt, start as u32, 1);
        }
      }
      b'!' => {
        if self.current() == b'=' {
          self.advance();
          self.tokens.push(Token::BangEq, start as u32, 2);
        } else {
          self.tokens.push(Token::Bang, start as u32, 1);
        }
      }
      b'&' => {
        if self.current() == b'&' {
          self.advance();
          self.tokens.push(Token::AmpAmp, start as u32, 2);
        } else if self.current() == b'=' {
          self.advance();
          self.tokens.push(Token::AmpEq, start as u32, 2);
        } else {
          self.tokens.push(Token::Amp, start as u32, 1);
        }
      }
      b'|' => {
        if self.current() == b'|' {
          self.advance();
          self.tokens.push(Token::PipePipe, start as u32, 2);
        } else if self.current() == b'>' {
          self.advance();
          self.tokens.push(Token::PipeArrow, start as u32, 2);
        } else if self.current() == b'=' {
          self.advance();
          self.tokens.push(Token::PipeEq, start as u32, 2);
        } else {
          self.tokens.push(Token::Pipe, start as u32, 1);
        }
      }
      b'^' => {
        if self.current() == b'=' {
          self.advance();
          self.tokens.push(Token::CaretEq, start as u32, 2);
        } else {
          self.tokens.push(Token::Caret, start as u32, 1);
        }
      }
      b'.' => {
        if self.current() == b'.' {
          self.advance();
          if self.current() == b'.' {
            self.advance();
            self.tokens.push(Token::Ellipsis, start as u32, 3);
          } else if self.current() == b'=' {
            self.advance();
            self.tokens.push(Token::DotDotEq, start as u32, 3);
          } else {
            self.tokens.push(Token::DotDot, start as u32, 2);
          }
        } else {
          self.tokens.push(Token::Dot, start as u32, 1);
        }
      }
      b'(' => {
        self.delimiter_stack.push(DelimiterInfo {
          kind: DelimiterKind::Paren,
          position: start as u32,
        });
        self.tokens.push(Token::LParen, start as u32, 1);
      }
      b')' => {
        self.check_closing_delimiter(DelimiterKind::Paren, start as u32);
        self.tokens.push(Token::RParen, start as u32, 1);
      }
      b'{' => {
        self.delimiter_stack.push(DelimiterInfo {
          kind: DelimiterKind::Brace,
          position: start as u32,
        });
        self.tokens.push(Token::LBrace, start as u32, 1);
        if self.state.mode() == ModeState::TEMPLATE {
          self.state.inc_brace_depth();
        }
      }
      b'}' => {
        self.check_closing_delimiter(DelimiterKind::Brace, start as u32);
        self.tokens.push(Token::RBrace, start as u32, 1);
        if self.state.mode() == ModeState::TEMPLATE {
          self.state.dec_brace_depth();
          // Don't automatically go into template text mode when closing
          // interpolation We might still be inside a tag like <div
          // class={...} />
        }
      }
      b'[' => {
        self.delimiter_stack.push(DelimiterInfo {
          kind: DelimiterKind::Bracket,
          position: start as u32,
        });
        self.tokens.push(Token::LBracket, start as u32, 1);
      }
      b']' => {
        self.check_closing_delimiter(DelimiterKind::Bracket, start as u32);
        self.tokens.push(Token::RBracket, start as u32, 1);
      }
      b',' => {
        self.tokens.push(Token::Comma, start as u32, 1);
      }
      b';' => {
        self.tokens.push(Token::Semicolon, start as u32, 1);
      }
      b'@' => {
        self.tokens.push(Token::At, start as u32, 1);
      }
      b'?' => {
        self.tokens.push(Token::Question, start as u32, 1);
      }
      b'#' => {
        self.tokens.push(Token::Hash, start as u32, 1);
      }
      _ => {
        if ch != b'$' {
          self.tokens.push(Token::Unknown, start as u32, 1);
        }
      }
    }
  }

  fn check_closing_delimiter(
    &mut self,
    expected_kind: DelimiterKind,
    position: u32,
  ) {
    // Check if there's a matching delimiter in the stack
    let matching_index = self
      .delimiter_stack
      .iter()
      .rposition(|d| d.kind == expected_kind);

    match matching_index {
      Some(index) => {
        // Found a matching opener
        if index == self.delimiter_stack.len() - 1 {
          // It's the top of the stack - perfect match
          self.delimiter_stack.pop();
        } else {
          // It's not at the top - we have mismatched delimiters
          // Report mismatch error
          report_error(Error::new(
            ErrorKind::MismatchedDelimiter,
            Span {
              start: position,
              len: 1,
            },
          ));
          // Pop everything above the matching opener (they're mismatched)
          let to_remove = self.delimiter_stack.len() - index - 1;
          for _ in 0..to_remove {
            if let Some(unmatched) = self.delimiter_stack.pop() {
              report_error(Error::new(
                ErrorKind::UnmatchedOpeningDelimiter,
                Span {
                  start: unmatched.position,
                  len: 1,
                },
              ));
            }
          }
          // Now pop the matching one
          self.delimiter_stack.pop();
        }
      }
      None => {
        // No matching opener - report unmatched closing delimiter
        report_error(Error::new(
          ErrorKind::UnmatchedClosingDelimiter,
          Span {
            start: position,
            len: 1,
          },
        ));
      }
    }
  }

  fn scan_number(&mut self) {
    let start = self.cursor;
    let first = self.current();

    // Check for display-base literals (b#, o#, x#).
    // These store the VALUE as decimal, with a display hint.
    // b#30 = decimal 30, displayed as binary "11110".
    // o#75 = decimal 75, displayed as octal "113".
    // x#76 = decimal 76, displayed as hex "4c".
    if (first == b'b' || first == b'o' || first == b'x') && self.peek(1) == b'#'
    {
      self.cursor += 2; // Skip base prefix

      // Scan decimal digits (the value is always decimal).
      while self.cursor < self.source.len() {
        let ch = self.current();

        if ch.is_ascii_digit() || ch == b'_' {
          self.cursor += 1;
        } else {
          break;
        }
      }

      let len = (self.cursor - start) as u16;

      let text = if start + 2 <= self.cursor {
        unsafe {
          std::str::from_utf8_unchecked(&self.source[start + 2..self.cursor])
        }
      } else {
        ""
      };

      let clean = text.replace('_', "");
      let value = clean.parse::<u64>().unwrap_or(0);
      let id = self.literals.push_int(value);

      self
        .tokens
        .push_with_literal(Token::Int, start as u32, len, id);

      return;
    }

    // Check for 0b, 0o, 0x prefixes
    if first == b'0' {
      let second = self.peek(1);
      if second == b'b' || second == b'o' || second == b'x' {
        self.cursor += 2; // Skip prefix

        let base = match second {
          b'b' => 2,
          b'o' => 8,
          b'x' => 16,
          _ => unreachable!(),
        };

        // Scan digits with underscores
        while self.cursor < self.source.len() {
          let ch = self.current();

          let valid = match base {
            2 => ch == b'0' || ch == b'1' || ch == b'_',
            8 => (b'0'..=b'7').contains(&ch) || ch == b'_',
            16 => ch.is_ascii_hexdigit() || ch == b'_',
            _ => false,
          };

          if valid {
            self.cursor += 1;
          } else {
            break;
          }
        }

        let len = (self.cursor - start) as u16;

        let text = if start + 2 <= self.cursor {
          unsafe {
            std::str::from_utf8_unchecked(&self.source[start + 2..self.cursor])
          }
        } else {
          ""
        };

        let clean_text = text.replace('_', "");
        let value = u64::from_str_radix(&clean_text, base as u32).unwrap_or(0);
        let id = self.literals.push_int(value);

        self
          .tokens
          .push_with_literal(Token::Int, start as u32, len, id);

        return;
      }
    }

    // Regular decimal number or float
    let mut has_dot = false;
    let mut has_exp = false;

    // Scan integer part
    while self.cursor < self.source.len() {
      let ch = self.current();

      if ch.is_ascii_digit() || ch == b'_' {
        self.cursor += 1;
      } else if ch == b'.'
        && !has_dot
        && !has_exp
        && self.peek(1).is_ascii_digit()
      {
        has_dot = true;
        self.cursor += 1;
      } else if (ch == b'e' || ch == b'E') && !has_exp {
        has_exp = true;
        self.cursor += 1;

        // Handle optional +/- after exponent
        let next = self.current();

        if next == b'+' || next == b'-' {
          self.cursor += 1;
        }

        // Scan exponent digits
        while self.cursor < self.source.len()
          && (self.current().is_ascii_digit() || self.current() == b'_')
        {
          self.cursor += 1;
        }

        break;
      } else {
        break;
      }
    }

    let len = (self.cursor - start) as u16;

    let text = unsafe {
      std::str::from_utf8_unchecked(&self.source[start..self.cursor])
    };

    if has_dot || has_exp {
      let value: f64 = text.replace('_', "").parse().unwrap_or(0.0);
      let id = self.literals.push_float(value);

      self
        .tokens
        .push_with_literal(Token::Float, start as u32, len, id);
    } else {
      let value: u64 = text.replace('_', "").parse().unwrap_or(0);
      let id = self.literals.push_int(value);

      self
        .tokens
        .push_with_literal(Token::Int, start as u32, len, id);
    }
  }

  #[inline(always)]
  fn scan_raw_string(&mut self, start: usize) {
    // Raw string literal $"..." - no escape sequences are processed
    let mut found_closing = false;

    while self.cursor < self.source.len() {
      if self.current() == b'"' {
        self.advance();

        found_closing = true;

        break;
      }

      self.cursor += 1;
    }

    if !found_closing {
      report_error(Error::new(
        ErrorKind::UnterminatedString,
        Span {
          start: start as u32,
          len: (self.cursor - start) as u16,
        },
      ));

      let len = (self.cursor - start) as u16;

      self.tokens.push(Token::Unknown, start as u32, len);
    } else {
      let len = (self.cursor - start) as u16;
      // Skip $" at start (2 bytes) and " at end (1 byte) when extracting
      // content
      let string_content =
        std::str::from_utf8(&self.source[start + 2..self.cursor - 1])
          .unwrap_or("");

      // Intern the string
      let symbol = self.interner.intern(string_content);
      // Store the Symbol in literals
      let id = self.literals.push_string_symbol(symbol);

      self
        .tokens
        .push_with_literal(Token::RawString, start as u32, len, id);
    }
  }

  #[inline(always)]
  fn scan_string(&mut self) {
    let start = self.cursor;
    self.advance(); // Skip opening "

    let mut found_closing = false;
    let mut has_interpolation = false;

    while self.cursor < self.source.len() {
      let ch = self.current();

      if ch == b'"' {
        self.advance();
        found_closing = true;
        break;
      }
      if ch == b'\\' {
        let esc_start = self.cursor;

        self.advance(); // skip backslash

        if self.cursor < self.source.len() {
          let esc = self.current();

          if !matches!(
            esc,
            b'n'
              | b'r'
              | b't'
              | b'\\'
              | b'"'
              | b'0'
              | b'{'
              | b'}'
              | b'x'
              | b'u'
              | b'U'
          ) {
            report_error(Error::new(
              ErrorKind::InvalidEscapeSequence,
              Span {
                start: esc_start as u32,
                len: 2,
              },
            ));
          }

          self.advance(); // skip escaped char
        }
      } else {
        if ch == b'{' {
          has_interpolation = true;
        }

        self.advance();
      }
    }

    if !found_closing {
      // Report unterminated string error
      report_error(Error::new(
        ErrorKind::UnterminatedString,
        Span {
          start: start as u32,
          len: (self.cursor - start) as u16,
        },
      ));

      // Still create a token so parsing can continue
      let len = (self.cursor - start) as u16;

      self.tokens.push(Token::Unknown, start as u32, len);
    } else {
      let len = (self.cursor - start) as u16;
      // Extract string content (without quotes)
      let string_content =
        std::str::from_utf8(&self.source[start + 1..self.cursor - 1])
          .unwrap_or("");

      if has_interpolation {
        // Parse segments and store in side table.
        let interp_id = self.parse_interp_segments(string_content);
        // Also intern the full string for the tree node.
        let symbol = self.interner.intern(string_content);
        let str_id = self.literals.push_string_symbol(symbol);
        // Encode both indices: str_id in literal_indices,
        // interp_id stored via push_with_literal on a
        // parallel field. We pack interp_id into the high
        // 16 bits of the literal index.
        let packed = str_id | ((interp_id as u32) << 16);

        self.tokens.push_with_literal(
          Token::InterpString,
          start as u32,
          len,
          packed,
        );
      } else {
        // Intern the string
        let symbol = self.interner.intern(string_content);
        // Store the Symbol in literals
        let id = self.literals.push_string_symbol(symbol);

        self
          .tokens
          .push_with_literal(Token::String, start as u32, len, id);
      }
    }
  }

  /// Parses interpolation segments from string content.
  /// Returns the interp_ranges index.
  fn parse_interp_segments(&mut self, content: &str) -> u32 {
    let bytes = content.as_bytes();
    let mut segments: Vec<InterpSegment> = Vec::new();
    let mut lit_start = 0;
    let mut i = 0;

    while i < bytes.len() {
      // Escaped brace: \{ → literal {
      if bytes[i] == b'\\' && i + 1 < bytes.len() && bytes[i + 1] == b'{' {
        let mut lit = content[lit_start..i].to_owned();

        lit.push('{');

        if !lit.is_empty() {
          let sym = self.interner.intern(&lit);

          segments.push(InterpSegment::Literal(sym));
        }

        i += 2;
        lit_start = i;

        continue;
      }

      if bytes[i] == b'{' {
        // Flush preceding literal.
        if i > lit_start {
          let lit = &content[lit_start..i];
          let sym = self.interner.intern(lit);

          segments.push(InterpSegment::Literal(sym));
        }

        // Scan variable name until '}'.
        let var_start = i + 1;

        i += 1;

        while i < bytes.len() && bytes[i] != b'}' {
          i += 1;
        }

        if i < bytes.len() {
          let var_name = &content[var_start..i];
          let sym = self.interner.intern(var_name);

          segments.push(InterpSegment::Variable(sym));

          i += 1; // skip }
        }

        lit_start = i;
      } else {
        i += 1;
      }
    }

    // Flush trailing literal.
    if lit_start < bytes.len() {
      let lit = &content[lit_start..];
      let sym = self.interner.intern(lit);

      segments.push(InterpSegment::Literal(sym));
    }

    self.literals.push_interp(&segments)
  }

  fn scan_char(&mut self) {
    let start = self.cursor;

    self.advance(); // Skip opening '

    let mut found_closing = false;
    let mut is_empty = false;

    // Check for empty character literal first
    if self.cursor < self.source.len() && self.current() == b'\'' {
      self.advance(); // Skip closing quote

      found_closing = true;
      is_empty = true;
    } else if self.cursor < self.source.len() && self.current() == b'\\' {
      self.advance(); // Skip backslash

      if self.cursor < self.source.len() {
        self.advance(); // Skip escaped char
      }

      if self.cursor < self.source.len() && self.current() == b'\'' {
        self.advance(); // Skip closing quote

        found_closing = true;
      }
    } else if self.cursor < self.source.len() {
      self.advance(); // Skip the character

      if self.cursor < self.source.len() && self.current() == b'\'' {
        self.advance(); // Skip closing quote

        found_closing = true;
      }
    }

    if is_empty {
      // Report empty character literal error
      report_error(Error::new(
        ErrorKind::EmptyCharLiteral,
        Span {
          start: start as u32,
          len: 2, // '' is 2 characters
        },
      ));

      let len = (self.cursor - start) as u16;

      self.tokens.push(Token::Unknown, start as u32, len);
    } else if !found_closing {
      // Report unterminated char literal
      report_error(Error::new(
        ErrorKind::UnterminatedChar,
        Span {
          start: start as u32,
          len: (self.cursor - start) as u16,
        },
      ));

      let len = (self.cursor - start) as u16;

      self.tokens.push(Token::Unknown, start as u32, len);
    } else {
      let len = (self.cursor - start) as u16;

      // Parse char value from source between quotes.
      let content = &self.source[start + 1..self.cursor - 1];
      let ch = if content.len() >= 2 && content[0] == b'\\' {
        match content[1] {
          b'n' => '\n',
          b'r' => '\r',
          b't' => '\t',
          b'\\' => '\\',
          b'\'' => '\'',
          b'0' => '\0',
          _ => {
            report_error(Error::new(
              ErrorKind::InvalidEscapeSequence,
              Span {
                start: (start + 1) as u32,
                len: 2,
              },
            ));
            content[1] as char
          }
        }
      } else if let Ok(s) = std::str::from_utf8(content) {
        s.chars().next().unwrap_or('\0')
      } else {
        content[0] as char
      };

      let id = self.literals.push_char(ch as u32);

      self
        .tokens
        .push_with_literal(Token::Char, start as u32, len, id);
    }
  }

  fn scan_bytes(&mut self) {
    let start = self.cursor;

    self.advance(); // Skip opening `

    let mut found_closing = false;

    while self.cursor < self.source.len() {
      let ch = self.current();

      if ch == b'`' {
        self.advance();

        found_closing = true;

        break;
      }

      if ch == b'\\' {
        self.advance();

        if self.cursor < self.source.len() {
          self.advance();
        }
      } else {
        self.advance();
      }
    }

    if !found_closing {
      report_error(Error::new(
        ErrorKind::UnterminatedBytes,
        Span {
          start: start as u32,
          len: (self.cursor - start) as u16,
        },
      ));

      let len = (self.cursor - start) as u16;

      self.tokens.push(Token::Unknown, start as u32, len);
    } else {
      let len = (self.cursor - start) as u16;
      let id = self.literals.push_bytes_span((start + 1) as u32, len - 2);

      self
        .tokens
        .push_with_literal(Token::Bytes, start as u32, len, id);
    }
  }

  pub fn tokenize(mut self) -> TokenizationResult {
    while self.cursor < self.source.len() {
      self.skip_whitespace();

      if self.cursor >= self.source.len() {
        break;
      }

      if self.state.is_template() {
        self.scan_template_token();
      } else {
        self.scan_code_token();
      }
    }

    // EOF token should be at source length, not cursor position
    // (cursor can be beyond source length after multi-char tokens)
    self.tokens.push(Token::Eof, self.source.len() as u32, 0);

    // Check for unclosed delimiters
    while let Some(delimiter) = self.delimiter_stack.pop() {
      report_error(Error::new(
        ErrorKind::UnmatchedOpeningDelimiter,
        Span {
          start: delimiter.position,
          len: 1,
        },
      ));
    }

    TokenizationResult {
      tokens: self.tokens,
      literals: self.literals,
      interner: self.interner,
      source_len: self.source.len(),
    }
  }
}
