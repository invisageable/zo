use zo_error::{Error, ErrorKind};
use zo_interner::Interner;
use zo_reporter::report_error;
use zo_span::Span;
use zo_token::{InterpSegment, LiteralStore, Token, TokenBuffer};

use serde::Serialize;

/// Longest keyword in the language, in bytes. Gates the keyword-match fast
/// path below — any identifier strictly longer than this can short-circuit
/// straight to `Token::Ident` without walking the match arms. Must be bumped
/// when a longer keyword is added; forgetting the bump silently turns the new
/// keyword into an identifier.
const MAX_KEYWORD_LEN: u16 = 9;

/// The complete result of tokenization
#[derive(Serialize)]
pub struct TokenizationResult {
  pub tokens: TokenBuffer,
  pub literals: LiteralStore,
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
  const STYLE: u32 = 2;
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
  fn is_style(self) -> bool {
    (self.0 & Self::MODE_MASK) == Self::STYLE
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
  interner: &'a mut Interner,
  state: ModeState,
  delimiter_stack: Vec<DelimiterInfo>,
  /// Brace-depth value at the entry of the current template
  /// scope. The TEMPLATE-vs-CODE gate in `scan_template_token`
  /// fires when `brace_depth > template_frame_base` (interp
  /// inside the template body). At top-level template
  /// (`imu view ::= <>...`) this stays at 0. When `=:>`
  /// fires *inside* an interp (`<ul>{arr.map(fn(t) =:> ...)}`),
  /// the closure body's template tokens need to be treated
  /// as markup despite `brace_depth > 0` — bumping the base
  /// to the current depth lets the gate work uniformly.
  /// Reset on `;` (end of statement) — same hook that resets
  /// mode to CODE.
  template_frame_base: u32,
  /// Set when the lexer tokenizes the `/` of a close tag
  /// (`</tag>`) in template mode. Cleared on the matching
  /// `>`. The `>` arm consults this to decide whether to
  /// re-enable template-text mode — we don't, after a close
  /// tag, because what follows is parent-context (more
  /// markup if nested, or end of frame if outermost).
  in_close_tag: bool,
  /// Element nesting depth inside the template. Incremented
  /// on the `>` that ends an open tag, decremented on the
  /// `>` that ends a close tag. The frame-end check uses
  /// `element_depth_at_frame_entry` rather than 0 to handle
  /// `=:>` opening inside an already-open element (e.g.
  /// `<ul>{arr.map(fn(t) =:> <li>...)}`).
  template_element_depth: u32,
  /// Snapshot of `template_element_depth` taken when an
  /// interp-opened template frame begins. The frame is
  /// considered done — `template_frame_base` reset, the
  /// trailing `)` of `.map(...)` re-tokenizes as code —
  /// when `template_element_depth` returns to this value
  /// after a close tag.
  template_element_depth_at_frame_entry: u32,
}

impl<'a> Tokenizer<'a> {
  const BYTES_PER_TOKEN: usize = 5;
  const TOKENS_PER_LITERAL: usize = 3;

  pub fn new(source: &'a str, interner: &'a mut Interner) -> Self {
    let bytes = source.as_bytes();
    let estimated_tokens = bytes.len() / Self::BYTES_PER_TOKEN;
    let estimated_literals = estimated_tokens / Self::TOKENS_PER_LITERAL;

    Self {
      source: bytes,
      cursor: 0,
      tokens: TokenBuffer::with_capacity(estimated_tokens),
      literals: LiteralStore::with_capacity(estimated_literals),
      interner,
      state: ModeState::new(),
      delimiter_stack: Vec::new(),
      template_frame_base: 0,
      in_close_tag: false,
      template_element_depth: 0,
      template_element_depth_at_frame_entry: 0,
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
    static ZERO_BYTE: [u8; 1] = [0];

    let pos = self.cursor + offset;
    let in_bounds = (pos < self.source.len()) as usize;

    unsafe {
      // When in_bounds = 1: read source[pos].
      // When in_bounds = 0: read ZERO_BYTE[0]. The previous
      // implementation read source[0] in the OOB case (because
      // `pos * 0 = 0`), which made `peek` return stale bytes
      // that could spoof a `0b/0o/0x` prefix and walk the
      // tokenizer past the end of the source — the source of a
      // long-standing fuzz panic on inputs like `"b:0"`.
      let base_ptr = [ZERO_BYTE.as_ptr(), self.source.as_ptr()][in_bounds];
      let offset = pos * in_bounds;

      *base_ptr.add(offset)
    }
  }

  #[inline(always)]
  fn advance(&mut self) -> u8 {
    let ch = self.current();

    self.cursor += (self.cursor < self.source.len()) as usize;

    ch
  }

  /// Close one template element. Decrements element depth.
  /// When the depth returns to the snapshot taken at the
  /// active frame's entry, the frame is done — reset
  /// `template_frame_base` so trailing code (the `)` of
  /// `.map(...)`, etc.) re-tokenizes via `scan_code_token`.
  /// Used by the `</>` fragment-end shorthand; the regular
  /// `</tag>` close inlines the same logic in the `>` arm.
  fn close_template_element(&mut self) {
    self.template_element_depth = self.template_element_depth.saturating_sub(1);

    if self.template_frame_base > 0
      && self.template_element_depth
        == self.template_element_depth_at_frame_entry
    {
      self.template_frame_base = 0;
    }
  }

  #[inline(never)]
  fn skip_whitespace(&mut self) {
    // Word-at-a-time fast path. Builds a 4-bit mask where
    // bit `i` is set iff byte `i` of the chunk is ASCII
    // whitespace; advances by 4 only when the whole word
    // is whitespace (`mask == 0xF`). The earlier OR-based
    // form folded all four flags into bit 0, so the mask
    // was 0 or 1 — never 0xF — and the SIMD path never
    // fired (the scalar fallback ran every time).
    while self.cursor + 4 <= self.source.len() {
      let chunk = unsafe {
        (self.source.as_ptr().add(self.cursor) as *const u32).read_unaligned()
      };

      let mask = ((chunk & 0xFF) as u8).is_ascii_whitespace() as u32
        | ((((chunk >> 8) & 0xFF) as u8).is_ascii_whitespace() as u32) << 1
        | ((((chunk >> 16) & 0xFF) as u8).is_ascii_whitespace() as u32) << 2
        | ((((chunk >> 24) & 0xFF) as u8).is_ascii_whitespace() as u32) << 3;

      if mask != 0xF {
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
    } else if len <= MAX_KEYWORD_LEN {
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
        3 if bytes == b"ffi" => Token::Ffi,
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
        6 if bytes == b"select" => Token::Select,
        5 if bytes == b"float" => Token::FloatType,
        5 if bytes == b"bytes" => Token::BytesType,
        6 if bytes == b"return" => Token::Return,
        6 if bytes == b"struct" => Token::Struct,
        7 if bytes == b"nursery" => Token::Nursery,
        9 if bytes == b"supervise" => Token::Supervise,
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
    // Brace-depth above the current template frame's base
    // means we're inside a `{...}` interp — tokenize as
    // code. The base is 0 for top-level templates and
    // bumped to the entry depth when `=:>` opens a new
    // template scope from inside an interp.
    if self.state.brace_depth() > self.template_frame_base {
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

        // HTML-style comment: `<!-- ... -->`. Consume through
        // the closing `-->` and emit nothing — comments are
        // stripped at compile time, matching React/Svelte/Vue
        // behavior. Template text mode resumes after the
        // comment so the next raw text run picks up correctly.
        if self.current() == b'!'
          && self.peek(1) == b'-'
          && self.peek(2) == b'-'
        {
          self.cursor += 3; // past `!--`

          while self.cursor + 2 < self.source.len() {
            if self.source[self.cursor] == b'-'
              && self.source[self.cursor + 1] == b'-'
              && self.source[self.cursor + 2] == b'>'
            {
              self.cursor += 3;
              break;
            }

            self.cursor += 1;
          }

          self.state.set_template_text(true);

          return;
        }

        if self.current() == b'>' {
          self.advance();
          self
            .tokens
            .push(Token::TemplateFragmentStart, start as u32, 2);
          self.state.set_template_text(true);
          self.template_element_depth += 1;
        } else if self.current() == b'/' && self.peek(1) == b'>' {
          self.cursor += 2;
          self
            .tokens
            .push(Token::TemplateFragmentEnd, start as u32, 3);
          self.close_template_element();
        } else {
          self.tokens.push(Token::LAngle, start as u32, 1);

          // `</tag>` close: mark so the `>` arm knows not to
          // re-enable template_text mode and to decrement
          // element depth.
          if self.current() == b'/' {
            self.in_close_tag = true;
          }
        }
      }
      b'>' => {
        self.tokens.push(Token::RAngle, start as u32, 1);

        if self.in_close_tag {
          self.in_close_tag = false;
          // Decrement before the text-mode decision so
          // `close_template_element` can detect a frame
          // exit (depth back at the entry snapshot).
          self.template_element_depth =
            self.template_element_depth.saturating_sub(1);

          let frame_done = self.template_frame_base > 0
            && self.template_element_depth
              == self.template_element_depth_at_frame_entry;

          if frame_done {
            // Closure body's outermost element just closed.
            // Surrender the frame so the trailing `)` of
            // `.map(...)` re-enters code-token scanning, and
            // suppress template-text — we're back to code
            // context, not parent-element inline content.
            self.template_frame_base = 0;
          } else {
            // Still inside an enclosing element — the
            // parent's text mode should resume so the
            // gap until the next `<` is captured as
            // `TemplateText`.
            self.state.set_template_text(true);
          }
        } else {
          self.state.set_template_text(true);
          self.template_element_depth += 1;
        }
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
          self.template_frame_base = 0;
          self.template_element_depth = 0;
          self.template_element_depth_at_frame_entry = 0;
          self.in_close_tag = false;
        }
      }
      _ => {
        self.cursor = start;
        self.scan_code_token();
      }
    }
  }

  fn scan_style_token(&mut self) {
    let start = self.cursor;
    let ch = self.advance();

    match ch {
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
        if self.state.brace_depth() > 0 {
          self.state.dec_brace_depth();
        }
        if self.state.brace_depth() == 0 {
          self.state.set_mode(ModeState::CODE);
        }
      }
      b';' => {
        self.tokens.push(Token::Semicolon, start as u32, 1);
      }
      b':' => {
        self.tokens.push(Token::Colon, start as u32, 1);
        // After colon in style context, scan the value.
        self.skip_whitespace();
        if self.cursor < self.source.len()
          && self.current() != b'}'
          && self.current() != b';'
        {
          self.scan_style_value();
        }
      }
      b',' => {
        self.tokens.push(Token::Comma, start as u32, 1);
      }
      b'.' => {
        self.tokens.push(Token::Dot, start as u32, 1);
      }
      b'#' => {
        self.tokens.push(Token::Hash, start as u32, 1);
      }
      b'-' => {
        // zo line comments (`--` and `-!`) inside style blocks.
        // CSS identifiers can also contain `-` (e.g.
        // `font-size`, `-webkit-foo`), so disambiguate on the
        // next byte: two dashes or `-!` is a comment, anything
        // else belongs to a CSS identifier.
        if self.current() == b'-' || self.current() == b'!' {
          self.skip_line_comment();
        } else {
          self.cursor = start;

          self.scan_style_ident();
        }
      }
      b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
        // Selector or property name — scan as identifier.
        // CSS allows hyphens in names (e.g. font-weight).
        self.cursor = start;
        self.scan_style_ident();
      }
      _ => {
        self.tokens.push(Token::Unknown, start as u32, 1);
      }
    }
  }

  /// Scans a CSS identifier (allows hyphens unlike code idents).
  fn scan_style_ident(&mut self) {
    let start = self.cursor;

    while self.cursor < self.source.len() {
      let ch = self.current();

      if !ch.is_ascii_alphanumeric() && ch != b'_' && ch != b'-' {
        break;
      }

      self.cursor += 1;
    }

    let len = (self.cursor - start) as u16;
    let bytes = &self.source[start..self.cursor];
    let text = std::str::from_utf8(bytes).unwrap_or("");
    let symbol = self.interner.intern(text);
    let id = self.literals.push_identifier(symbol);

    self
      .tokens
      .push_with_literal(Token::Ident, start as u32, len, id);
  }

  /// Scans an opaque CSS value (everything until `;` or `}`).
  fn scan_style_value(&mut self) {
    let start = self.cursor;

    while self.cursor < self.source.len() {
      let ch = self.current();

      if ch == b';' || ch == b'}' {
        break;
      }

      self.cursor += 1;
    }

    // Trim trailing whitespace from the value.
    let mut end = self.cursor;

    while end > start && self.source[end - 1].is_ascii_whitespace() {
      end -= 1;
    }

    if end > start {
      let len = (end - start) as u16;
      let bytes = &self.source[start..end];
      let text = std::str::from_utf8(bytes).unwrap_or("");
      let symbol = self.interner.intern(text);
      let id = self.literals.push_string_symbol(symbol);

      self
        .tokens
        .push_with_literal(Token::StyleValue, start as u32, len, id);
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
        if self.current() == b'"' {
          // Raw string literal $"..."
          self.advance();
          self.scan_raw_string(start);
        } else if self.current() == b':' {
          // Style block $: { ... }
          self.tokens.push(Token::Dollar, start as u32, 1);
          let colon_start = self.cursor;
          self.advance();
          self.tokens.push(Token::Colon, colon_start as u32, 1);
          self.state.set_mode(ModeState::STYLE);
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
        } else if self.current() == b':' && self.peek(1) == b'>' {
          // `=:>` — closure body opener that switches the
          // lexer into TEMPLATE mode. Mirrors `::=`'s mode
          // shift; distinct from the assignment binding form.
          //
          // When `=:>` fires from inside an interp (e.g.
          // `<ul>{arr.map(fn(t) =:> <li>{t}</li>)}`), the
          // closure body's `<li>...</li>` are template
          // markup but `brace_depth > 0`. Bump the frame
          // base so the markup gate fires for them, and
          // snapshot the current element depth so we can
          // detect the body's outermost close tag and
          // restore code mode for the trailing `)`.
          self.advance();
          self.advance();
          self.template_frame_base = self.state.brace_depth();
          self.template_element_depth_at_frame_entry =
            self.template_element_depth;
          self.state.set_mode(ModeState::TEMPLATE);
          self.tokens.push(Token::TemplateFatArrow, start as u32, 3);
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
          // Match found deeper — everything above it is
          // unmatched. Report each with a secondary span
          // pointing at the closing delimiter that skipped
          // over them.
          let to_remove = self.delimiter_stack.len() - index - 1;

          for _ in 0..to_remove {
            if let Some(unmatched) = self.delimiter_stack.pop() {
              report_error(Error::with_secondary(
                ErrorKind::UnmatchedOpeningDelimiter,
                Span {
                  start: unmatched.position,
                  len: 1,
                },
                Span {
                  start: position,
                  len: 1,
                },
              ));
            }
          }

          // Pop the matching one.
          self.delimiter_stack.pop();
        }
      }
      None => {
        // No matching opener found. If the stack has an
        // opener of a different kind, it's a mismatch.
        // Otherwise it's a truly unmatched closer.
        if let Some(opener) = self.delimiter_stack.last() {
          report_error(Error::with_secondary(
            ErrorKind::MismatchedDelimiter,
            Span {
              start: position,
              len: 1,
            },
            Span {
              start: opener.position,
              len: 1,
            },
          ));

          self.delimiter_stack.pop();
        } else {
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

      // Unterminated at newline — don't eat the rest
      // of the file.
      if ch == b'\n' {
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
              | b'\''
              | b'0'
              | b'{'
              | b'}'
              | b'x'
              | b'u'
              | b'e'
              | b'v'
              | b'b'
              | b'a'
              | b'f'
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
      // Extract string content (without quotes) and unescape.
      let raw_content =
        std::str::from_utf8(&self.source[start + 1..self.cursor - 1])
          .unwrap_or("");
      let unescaped = unescape_string(raw_content);
      let string_content = unescaped.as_str();

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

      // Skip the first escape char so a literal `\'` inside
      // the literal (an escaped closing quote) doesn't end
      // the scan prematurely. Then keep advancing until the
      // closing `'` — escapes like `\xNN` and `\u{HHHH}` are
      // longer than two chars, and pinning a fixed length
      // here would mis-tokenize them as `Token::Unknown`.
      // `unescape_string` + the 1-char check below validates
      // the escape's shape; the scanner's job is just to
      // find the matching quote.
      if self.cursor < self.source.len() {
        self.advance();
      }

      while self.cursor < self.source.len() && self.current() != b'\'' {
        self.advance();
      }

      if self.cursor < self.source.len() && self.current() == b'\'' {
        self.advance(); // Skip closing quote

        found_closing = true;
      }
    } else if self.cursor < self.source.len() {
      // Advance past one full UTF-8 scalar — char literals
      // are per-codepoint, not per-byte. A lead byte of a
      // multi-byte sequence needs its trailing bytes skipped
      // too, otherwise the closing `'` is missed and the
      // tokenizer reports a spurious "Unterminated character
      // literal" on every non-ASCII char.
      let cp_len = utf8_cp_len(self.current());

      self.cursor += cp_len.min(self.source.len() - self.cursor);

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
      // Delegate escape parsing to `parse_one_escape` so the
      // char-literal escape set stays in lockstep with the
      // string-literal one. `parse_one_escape` returns the
      // decoded char + the unconsumed tail; any tail bytes
      // mean the escape consumed less than the full content,
      // which we surface as `InvalidEscapeSequence`. No
      // `String` allocation per char literal.
      let content = &self.source[start + 1..self.cursor - 1];
      let ch = if content.len() >= 2 && content[0] == b'\\' {
        let raw = std::str::from_utf8(content).unwrap_or("");

        match parse_one_escape(&raw[1..]) {
          Some((c, rest)) if rest.is_empty() => c,
          _ => {
            report_error(Error::new(
              ErrorKind::InvalidEscapeSequence,
              Span {
                start: (start + 1) as u32,
                len: content.len() as u16,
              },
            ));

            '\0'
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

      // Unterminated at newline — don't eat the rest
      // of the file.
      if ch == b'\n' {
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
      // Extract the first byte between backticks.
      let byte_val = self.source[start + 1];
      let id = self.literals.push_bytes(byte_val);

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
      } else if self.state.is_style() {
        self.scan_style_token();
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
      source_len: self.source.len(),
    }
  }
}

/// Returns the UTF-8 byte length of the codepoint whose
/// lead byte is `b`. ASCII (< 0x80) and stray continuation
/// bytes (0x80..0xC0) both fall back to 1 so the tokenizer
/// still advances instead of looping — malformed UTF-8 is
/// reported upstream as `UnexpectedCharacter` at its own
/// site.
#[inline(always)]
fn utf8_cp_len(b: u8) -> usize {
  if b < 0xC0 {
    1
  } else if b < 0xE0 {
    2
  } else if b < 0xF0 {
    3
  } else {
    4
  }
}

/// Parse one escape sequence from `after`, the bytes that
/// follow a leading `\`. Returns `(decoded_char, rest)` on
/// success or `None` for unknown / malformed input. Used
/// directly by `scan_char` (one escape per char literal,
/// no `String` allocation) and indirectly by
/// `unescape_string` (looped to walk a whole string body).
fn parse_one_escape(after: &str) -> Option<(char, &str)> {
  let mut chars = after.chars();

  let c = match chars.next()? {
    'n' => '\n',
    'r' => '\r',
    't' => '\t',
    '\\' => '\\',
    '"' => '"',
    '\'' => '\'',
    '0' => '\0',
    '{' => '{',
    '}' => '}',
    'e' => '\x1B',
    'v' => '\x0B',
    'b' => '\x08',
    'a' => '\x07',
    'f' => '\x0C',
    'x' => {
      let h = chars.next()?.to_digit(16)?;
      let l = chars.next()?.to_digit(16)?;

      (h * 16 + l) as u8 as char
    }
    'u' => {
      if chars.next()? != '{' {
        return None;
      }

      let mut codepoint: u32 = 0;
      let mut digits = 0usize;
      let mut closed = false;

      for c in chars.by_ref() {
        if c == '}' {
          closed = true;

          break;
        }

        let d = c.to_digit(16)?;

        if digits >= 6 {
          return None;
        }

        codepoint = (codepoint << 4) | d;
        digits += 1;
      }

      if !closed || digits == 0 {
        return None;
      }

      char::from_u32(codepoint)?
    }
    _ => return None,
  };

  Some((c, chars.as_str()))
}

/// Process escape sequences in a string literal.
/// Converts `\"` → `"`, `\\` → `\`, `\n` → newline, the
/// C-style control escapes `\e \v \b \a \f`, and `\xNN`
/// (two hex digits → one raw byte). Returns the original
/// string unchanged if no backslashes appear. The set
/// recognized here MUST stay in sync with `scan_string`'s
/// validator — anything the validator accepts but this
/// rejects ships as a literal `\<char>` to user code.
fn unescape_string(s: &str) -> String {
  if !s.contains('\\') {
    return s.to_string();
  }

  let mut out = String::with_capacity(s.len());
  let mut chars = s.chars();

  while let Some(ch) = chars.next() {
    if ch != '\\' {
      out.push(ch);

      continue;
    }

    match chars.next() {
      Some('n') => out.push('\n'),
      Some('r') => out.push('\r'),
      Some('t') => out.push('\t'),
      Some('\\') => out.push('\\'),
      Some('"') => out.push('"'),
      Some('\'') => out.push('\''),
      Some('0') => out.push('\0'),
      Some('{') => out.push('{'),
      Some('}') => out.push('}'),
      Some('e') => out.push('\x1B'),
      Some('v') => out.push('\x0B'),
      Some('b') => out.push('\x08'),
      Some('a') => out.push('\x07'),
      Some('f') => out.push('\x0C'),
      Some('x') => {
        // `\xNN` — two hex digits → one raw byte. Falls
        // back to the literal `\x` if the digits are
        // malformed (the validator only checks the `x`,
        // not the digit pair).
        let hi = chars.next();
        let lo = chars.next();

        match (
          hi.and_then(|c| c.to_digit(16)),
          lo.and_then(|c| c.to_digit(16)),
        ) {
          (Some(h), Some(l)) => out.push((h * 16 + l) as u8 as char),
          _ => {
            out.push('\\');
            out.push('x');

            if let Some(h) = hi {
              out.push(h);
            }

            if let Some(l) = lo {
              out.push(l);
            }
          }
        }
      }
      Some('u') => {
        // `\u{HHHH...}` — 1–6 hex digits → one Unicode
        // scalar value, encoded as UTF-8. Bracket form
        // matches Rust. Malformed input (missing `{`,
        // missing `}`, no digits, > 6 digits, > 0x10FFFF,
        // or surrogate range) leaks back as the literal
        // `\u…` so the user sees their own bytes rather
        // than dropped characters. `tail` snapshots the
        // unconsumed source so the rollback re-feeds the
        // exact bytes the inner loop ate.
        let tail = chars.as_str();

        match chars.next() {
          Some('{') => {
            let mut codepoint: u32 = 0;
            let mut digits = 0usize;
            let mut closed = false;

            for c in chars.by_ref() {
              if c == '}' {
                closed = true;

                break;
              }

              match c.to_digit(16) {
                Some(d) if digits < 6 => {
                  codepoint = (codepoint << 4) | d;
                  digits += 1;
                }
                _ => break,
              }
            }

            match (closed, digits, char::from_u32(codepoint)) {
              (true, 1..=6, Some(c)) => out.push(c),
              _ => {
                out.push('\\');
                out.push('u');
                chars = tail.chars();
              }
            }
          }
          _ => {
            out.push('\\');
            out.push('u');
            chars = tail.chars();
          }
        }
      }
      Some(other) => {
        out.push('\\');
        out.push(other);
      }
      None => out.push('\\'),
    }
  }

  out
}

#[cfg(test)]
mod escape_tests {
  use super::unescape_string;

  #[test]
  fn no_escapes_returns_input_verbatim() {
    assert_eq!(unescape_string("hello"), "hello");
    assert_eq!(unescape_string(""), "");
  }

  #[test]
  fn standard_escapes() {
    assert_eq!(unescape_string("\\n"), "\n");
    assert_eq!(unescape_string("\\r"), "\r");
    assert_eq!(unescape_string("\\t"), "\t");
    assert_eq!(unescape_string("\\\\"), "\\");
    assert_eq!(unescape_string("\\\""), "\"");
    assert_eq!(unescape_string("\\0"), "\0");
    assert_eq!(unescape_string("\\{"), "{");
    assert_eq!(unescape_string("\\}"), "}");
  }

  #[test]
  fn c_style_control_escapes() {
    assert_eq!(unescape_string("\\e"), "\x1B");
    assert_eq!(unescape_string("\\v"), "\x0B");
    assert_eq!(unescape_string("\\b"), "\x08");
    assert_eq!(unescape_string("\\a"), "\x07");
    assert_eq!(unescape_string("\\f"), "\x0C");
  }

  #[test]
  fn hex_byte_escape() {
    assert_eq!(unescape_string("\\x41"), "A"); // 0x41 = 'A'
    assert_eq!(unescape_string("\\x00"), "\0");
    assert_eq!(unescape_string("\\xff"), "\u{00FF}");
    // Mixed-case hex digits both accepted.
    assert_eq!(unescape_string("\\xAb"), "\u{00AB}");
  }

  #[test]
  fn malformed_hex_byte_falls_back_to_literal() {
    // Validator only checks the leading `x`; the digit pair
    // is the processor's problem. Bad pair → keep the
    // backslash + chars verbatim, don't drop bytes.
    assert_eq!(unescape_string("\\xZZ"), "\\xZZ");
    assert_eq!(unescape_string("\\xA"), "\\xA");
  }

  #[test]
  fn ansi_color_sequence_round_trip() {
    // The motivating case: `\e[32m` is one byte (ESC) then
    // four ASCII chars. Total 5 chars in the unescaped
    // string.
    let out = unescape_string("\\e[32mHello\\e[0m");

    assert_eq!(out, "\x1B[32mHello\x1B[0m");
    assert_eq!(out.len(), 14);
  }

  #[test]
  fn unknown_escape_passes_through_with_backslash() {
    // The validator rejects unknown escapes, but if one
    // slips through, the processor preserves the bytes.
    assert_eq!(unescape_string("\\q"), "\\q");
  }

  #[test]
  fn unicode_codepoint_escape_basic_latin() {
    assert_eq!(unescape_string("\\u{41}"), "A");
    assert_eq!(unescape_string("\\u{0041}"), "A");
  }

  #[test]
  fn unicode_codepoint_escape_emoji() {
    // U+1F600 = 😀 — exercises the > BMP path (4-byte
    // UTF-8 encoding, 5 hex digits).
    assert_eq!(unescape_string("\\u{1F600}"), "😀");
  }

  #[test]
  fn unicode_codepoint_escape_six_digits_max() {
    // U+10FFFF — Unicode's upper bound, fits in 6 hex
    // digits. `char::from_u32` accepts it.
    assert_eq!(unescape_string("\\u{10FFFF}"), "\u{10FFFF}");
  }

  #[test]
  fn unicode_codepoint_escape_inside_text() {
    let out = unescape_string("hi \\u{2603} there");

    assert_eq!(out, "hi \u{2603} there");
  }

  #[test]
  fn unicode_codepoint_escape_malformed_falls_back() {
    // Missing brace, missing close, surrogate range,
    // out-of-range, or no digits → keep the literal `\u`
    // and the rest of the source (don't drop bytes).
    assert_eq!(unescape_string("\\uxyz"), "\\uxyz");
    assert_eq!(unescape_string("\\u{}"), "\\u{}");
    assert_eq!(unescape_string("\\u{D800}"), "\\u{D800}");
    assert_eq!(unescape_string("\\u{110000}"), "\\u{110000}");
  }

  #[test]
  fn single_quote_escape() {
    // Char literals delegate to `unescape_string`, so
    // `'\''` needs the single-quote arm too.
    assert_eq!(unescape_string("\\'"), "'");
  }
}
