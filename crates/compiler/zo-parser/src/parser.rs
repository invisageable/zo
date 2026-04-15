use zo_error::{Error, ErrorKind};
use zo_reporter::report_error;
use zo_span::Span;
use zo_token::{LiteralStore, Token, TokenBuffer};
use zo_tokenizer::TokenizationResult;
use zo_tree::{NodeValue, Tree};

use serde::Serialize;

/// The result of parsing
#[derive(Serialize)]
pub struct ParsingResult {
  pub tree: Tree,
}

/// Parser state for tracking context
#[derive(Debug, Clone, Copy, PartialEq)]
enum ParserState {
  TopLevel,          // At module/file level
  FunctionSignature, // Inside function signature
  ParameterList,     // Inside parameter list
  Block,             // Inside a block {}
  Expression,        // Inside an expression
  TypeAnnotation,    // After : expecting type
  TemplateMode,      // Inside template content
  ModulePath,        // Inside load/pack path (direct emit)
  StyleBlock,        // Inside $: { ... } style block
}

/// Introducer info for tracking nested structures
#[derive(Debug)]
struct Introducer {
  /// The state we were in when this introducer was encountered
  state: ParserState,
  /// Token kind of the introducer (Fun, LParen, LBrace, etc.)
  token: Token,
  /// Index in the parse tree where this introducer was emitted
  node_index: u32,
  /// Start position for children of this introducer
  children_start: u32,
}

/// State machine-based parser
pub struct Parser<'a> {
  /// The source code as text.
  source: &'a str,
  /// The [`TokenBuffer`].
  tokens: &'a TokenBuffer,
  /// The [`LiteralStore`].
  literals: &'a LiteralStore,
  /// The linear postorder parse [`Tree`].
  tree: Tree,
  /// The position.
  pos: usize,
  /// The parser states.
  state: ParserState,
  /// The [`Introducer`] stack.
  introducer_stack: Vec<Introducer>,
  /// The expression buffer for reordering — (token, span, value).
  expr_buffer: Vec<(Token, Span, Option<NodeValue>)>,
  /// The operators stack — (token, precedence, associativity).
  operator_stack: Vec<(Token, u8, u8)>,
  /// Saved operator stacks for array indexing — the outer
  /// operators are parked here while the index expression
  /// is parsed, then restored on `]`.
  saved_op_stacks: Vec<Vec<(Token, u8, u8)>>,
  /// Saved expr buffers for array indexing.
  saved_expr_bufs: Vec<Vec<(Token, Span, Option<NodeValue>)>>,
  /// Spans for unary prefix operators (not in expr_buffer).
  unary_spans: Vec<(Token, Span)>,
  /// Saved unary spans for nested calls/indices.
  saved_unary_spans: Vec<Vec<(Token, Span)>>,
  /// True after emitting a value-producing token (RParen,
  /// RBracket, Ident, literal) when expr_buffer is empty.
  /// Prevents `*` / `&` from being misidentified as unary.
  last_was_value: bool,
  /// Nesting depth inside type annotation parens: Fn(...) or (ty, ty).
  type_paren_depth: u32,
}
impl<'a> Parser<'a> {
  const INTRODUCER_STACK_CAP: usize = 32;
  const EXPR_BUFFER_CAP: usize = 64;
  const OPERATOR_STACK_CAP: usize = 16;

  /// Creates a new [`Parser`] instance.
  pub fn new(tokenization: &'a TokenizationResult, source: &'a str) -> Self {
    Self {
      source,
      tokens: &tokenization.tokens,
      literals: &tokenization.literals,
      tree: Tree::new(),
      pos: 0,
      state: ParserState::TopLevel,
      introducer_stack: Vec::with_capacity(Self::INTRODUCER_STACK_CAP),
      expr_buffer: Vec::with_capacity(Self::EXPR_BUFFER_CAP),
      operator_stack: Vec::with_capacity(Self::OPERATOR_STACK_CAP),
      saved_op_stacks: Vec::new(),
      saved_expr_bufs: Vec::new(),
      unary_spans: Vec::new(),
      saved_unary_spans: Vec::new(),
      last_was_value: false,
      type_paren_depth: 0,
    }
  }

  #[inline(always)]
  fn current_span(&self) -> Span {
    if self.pos < self.tokens.kinds.len() {
      Span {
        start: self.tokens.starts[self.pos],
        len: self.tokens.lengths[self.pos],
      }
    } else {
      Span::ZERO
    }
  }

  #[inline(always)]
  fn peek(&self) -> Option<Token> {
    if self.pos + 1 < self.tokens.kinds.len() {
      Some(self.tokens.kinds[self.pos + 1])
    } else {
      None
    }
  }

  /// Reports a parse error at the current token.
  fn error(&self, kind: ErrorKind) {
    report_error(Error::new(kind, self.current_span()));
  }

  /// Reports a parse error at a specific position.
  fn error_at(&self, kind: ErrorKind, pos: usize) {
    let span = if pos < self.tokens.kinds.len() {
      Span {
        start: self.tokens.starts[pos],
        len: self.tokens.lengths[pos],
      }
    } else {
      self.current_span()
    };

    report_error(Error::new(kind, span));
  }

  /// Parses an array of tokens.
  pub fn parse(mut self) -> ParsingResult {
    while self.pos < self.tokens.kinds.len() {
      let kind = self.tokens.kinds[self.pos];

      if kind == Token::Eof {
        break;
      }

      self.process_token(kind);

      self.pos += 1;
    }

    self.flush_expr();

    while !self.introducer_stack.is_empty() {
      self.close_introducer();
    }

    // Debug output
    // self.debug_print_tree();

    ParsingResult { tree: self.tree }
  }

  fn process_token(&mut self, kind: Token) {
    // In style blocks, tokens emit directly — no expression
    // reordering, no type annotation, no keyword dispatch.
    if self.state == ParserState::StyleBlock {
      match kind {
        Token::LBrace => {
          let node_index = self.emit_node(Token::LBrace);

          self.introducer_stack.push(Introducer {
            state: ParserState::StyleBlock,
            token: Token::LBrace,
            node_index,
            children_start: self.tree.nodes.len() as u32,
          });
        }
        Token::RBrace => self.handle_rbrace_closer(),
        Token::StyleValue => {
          let span = self.current_span();
          let value = self.extract_value(Token::StyleValue);

          self.emit_node_internal(Token::StyleValue, span, value);
        }
        Token::Ident => {
          self.emit_node(Token::Ident);
        }
        _ => {
          self.emit_node(kind);
        }
      }

      return;
    }

    match kind {
      // Module statements
      Token::Load => self.handle_load_statement(),
      Token::Pack => self.handle_pack_statement(),

      // Introducers - these start new contexts
      Token::Fun | Token::Ext | Token::Fn => self.handle_fun_introducer(kind),
      Token::Enum => self.handle_enum_keyword(),
      Token::Struct => self.handle_struct_keyword(),
      Token::Apply => self.handle_apply_keyword(),
      Token::Type => self.handle_type_alias_keyword(),
      Token::Group => self.handle_group_keyword(),
      Token::And => self.handle_and_keyword(),

      Token::LParen => self.handle_lparen_introducer(),
      Token::LBrace => self.handle_lbrace_introducer(),
      Token::LBracket => self.handle_lbracket(),

      // Closers - these end contexts
      Token::RParen => self.handle_rparen_closer(),
      Token::RBrace => self.handle_rbrace_closer(),
      Token::RBracket => self.handle_rbracket_closer(),

      // Special tokens that affect state
      Token::Colon => self.handle_colon(),
      Token::ColonEq => self.handle_colon_eq(),
      Token::TemplateAssign => self.handle_template_assign(),
      Token::Arrow => self.handle_arrow(),
      Token::FatArrow => self.handle_fat_arrow(),
      Token::Comma => self.handle_comma(),
      Token::Semicolon => self.handle_semicolon(),
      Token::Eq => self.handle_assignment(),

      // Compound assignment operators
      Token::PlusEq
      | Token::MinusEq
      | Token::StarEq
      | Token::SlashEq
      | Token::PercentEq
      | Token::AmpEq
      | Token::PipeEq
      | Token::CaretEq
      | Token::LShiftEq
      | Token::RShiftEq => self.handle_compound_assignment(kind),

      // Types
      _ if kind.is_ty() => self.handle_type(kind),

      // Unary operators (check context to distinguish from binary)
      Token::Bang => self.handle_unary_operator(kind),
      Token::Minus if self.is_unary_context() => {
        self.handle_unary_operator(Token::UnaryMinus)
      }
      Token::Star | Token::Amp if self.is_unary_context() => {
        self.handle_unary_operator(kind)
      }

      // Generic type parameters: <$T, $A> after fun/struct/
      // enum/apply/type. Must be checked before the operator
      // path, otherwise `<` would be parsed as less-than.
      Token::Lt
        if self.peek() == Some(Token::Dollar)
          || (self.peek() == Some(Token::Ident)
            && (self.state == ParserState::FunctionSignature
              || self.introducer_stack.last().is_some_and(|i| {
                matches!(
                  i.token,
                  Token::Struct
                    | Token::Enum
                    | Token::Apply
                    | Token::Type
                    | Token::Group
                )
              }))) =>
      {
        let in_generic_ctx = self.state == ParserState::FunctionSignature
          || self.introducer_stack.last().is_some_and(|i| {
            matches!(
              i.token,
              Token::Struct
                | Token::Enum
                | Token::Apply
                | Token::Type
                | Token::Group
            )
          });

        if in_generic_ctx {
          self.flush_expr();
          self.parse_type_params();
        } else {
          self.handle_operator(kind);
        }
      }

      // Fn type annotation in type context: buffer as part
      // of the type expression so the executor can see it.
      Token::FnType if self.state == ParserState::TypeAnnotation => {
        self
          .expr_buffer
          .push((Token::FnType, self.current_span(), None));
      }

      // Binary operators
      _ if self.is_operator(kind) => self.handle_operator(kind),

      // Style block: $: { ... }
      Token::Dollar if self.peek() == Some(Token::Colon) => {
        self.handle_style_block();
      }

      // Operands (identifiers, literals)
      _ if kind.is_operand() => self.handle_operand(kind),

      // Keywords that introduce statements.
      // In parameter lists, `mut` is a modifier (e.g.,
      // `mut self`) — emit it directly so the executor
      // can read it.
      Token::Mut if self.state == ParserState::ParameterList => {
        let span = self.current_span();

        self.emit_node_internal(Token::Mut, span, None);
      }
      Token::Imu | Token::Mut | Token::Val => self.handle_binding_keyword(kind),

      // Control flow keywords
      Token::If => self.handle_if_keyword(),
      Token::Else => self.handle_else_keyword(),
      Token::When => self.handle_when_keyword(),
      Token::While => self.handle_while_keyword(),
      Token::For => self.handle_for_keyword(),
      Token::Match => self.handle_match_keyword(),
      Token::Return => self.handle_return_keyword(),

      // Directives
      Token::Hash => self.handle_directive(),

      // Modifier syntax: ident@ident (e.g., check@lt)
      Token::At => self.handle_at(),

      // Style value tokens
      Token::StyleValue => {
        let span = self.current_span();
        let value = self.extract_value(Token::StyleValue);

        self.emit_node_internal(Token::StyleValue, span, value);
      }

      // Template tokens
      Token::TemplateFragmentStart => self.handle_template_fragment_start(),
      Token::TemplateFragmentEnd => self.handle_template_fragment_end(),
      Token::LAngle => self.handle_langle(),
      Token::RAngle => self.handle_rangle(),
      Token::Gt if self.state == ParserState::TemplateMode => {
        self.handle_rangle()
      }
      Token::Slash | Token::Slash2 => self.handle_slash(),
      Token::TemplateText => self.handle_template_text(),

      // Ternary delimiters: flush the expression so
      // operators in the condition/arms are correctly
      // placed before ? and : in the tree.
      Token::Question
        if self
          .introducer_stack
          .last()
          .is_some_and(|i| i.token == Token::When) =>
      {
        self.flush_expr();
        self.emit_node(kind);
      }

      // Type cast: expr as Type.
      // Flush the expression (LHS), emit As, then emit the
      // target type directly so it appears right after As in
      // the tree (the executor reads tree[idx+1] for the type).
      Token::As => {
        self.flush_expr();
        self.emit_node(Token::As);

        // Peek at the next token — if it's a type, consume
        // and emit it now so it's adjacent to As in the tree.
        if self.pos + 1 < self.tokens.kinds.len()
          && self.tokens.kinds[self.pos + 1].is_ty()
        {
          self.pos += 1;
          self.emit_node(self.tokens.kinds[self.pos]);
        }
      }

      // Enum variant access: Foo::Ok
      Token::ColonColon => {
        self.flush_expr();
        self.emit_node(kind);
      }

      // Everything else gets emitted as-is
      _ => {
        self.emit_node(kind);
      }
    }
  }

  fn handle_load_statement(&mut self) {
    self.flush_expr();

    let node_index = self.emit_node(Token::Load);

    self.introducer_stack.push(Introducer {
      state: self.state,
      token: Token::Load,
      node_index,
      children_start: self.tree.nodes.len() as u32,
    });

    self.state = ParserState::ModulePath;
  }

  fn handle_pack_statement(&mut self) {
    self.flush_expr();

    let node_index = self.emit_node(Token::Pack);

    self.introducer_stack.push(Introducer {
      state: self.state,
      token: Token::Pack,
      node_index,
      children_start: self.tree.nodes.len() as u32,
    });

    self.state = ParserState::ModulePath;
  }

  /// Parses `<$T, $A, $B>` type parameter lists.
  /// Emits each `$T` as `Dollar` + `Ident` nodes.
  /// Called when `Lt` is seen in `FunctionSignature` state
  /// with `Dollar` as the next token.
  fn parse_type_params(&mut self) {
    // Emit `<` as LAngle (generic open, not less-than).
    let span = self.current_span();

    self.emit_node_internal(Token::LAngle, span, None);

    // Parse $T, $A, ... until >
    loop {
      self.pos += 1;

      if self.pos >= self.tokens.kinds.len() {
        break;
      }

      let kind = self.tokens.kinds[self.pos];

      match kind {
        Token::Gt => {
          let span = self.current_span();

          self.emit_node_internal(Token::RAngle, span, None);

          break;
        }
        Token::Dollar => {
          let span = self.current_span();

          self.emit_node_internal(Token::Dollar, span, None);

          // Next should be the type param name ident.
          if self.peek() == Some(Token::Ident) {
            self.pos += 1;

            let span = self.current_span();
            let value = self.extract_value(Token::Ident);

            self.emit_node_internal(Token::Ident, span, value);
          }
        }
        Token::Comma => {
          let span = self.current_span();

          self.emit_node_internal(Token::Comma, span, None);
        }
        // Bare ident without $ — report helpful error.
        Token::Ident => {
          self.error_at(ErrorKind::MissingDollarPrefix, self.pos);

          break;
        }
        _ => break,
      }
    }
  }

  fn handle_fun_introducer(&mut self, token: Token) {
    self.flush_expr();

    // `fun` must be followed by an identifier (name)
    // or `(` for closures (fn).
    if token == Token::Fun
      && self
        .peek()
        .is_some_and(|n| !matches!(n, Token::Ident | Token::Pub))
    {
      self.error_at(ErrorKind::ExpectedIdentifier, self.pos + 1);
    }

    let node_index = self.emit_node(token);

    self.introducer_stack.push(Introducer {
      state: self.state,
      token,
      node_index,
      children_start: self.tree.nodes.len() as u32,
    });

    self.state = ParserState::FunctionSignature;
  }

  fn handle_lparen_introducer(&mut self) {
    // Inside Fn(...) type annotation, buffer ( as part of type
    if self.state == ParserState::TypeAnnotation {
      self
        .expr_buffer
        .push((Token::LParen, self.current_span(), None));

      self.type_paren_depth += 1;

      return;
    }

    // In function signature, this starts parameter list
    if self.state == ParserState::FunctionSignature {
      self.flush_expr();

      let node_index = self.emit_node(Token::LParen);

      self.introducer_stack.push(Introducer {
        state: self.state,
        token: Token::LParen,
        node_index,
        children_start: self.tree.nodes.len() as u32,
      });

      self.state = ParserState::ParameterList;
    } else {
      // For function/method calls, flush the expression
      // first then emit LParen. Save pending unary ops so
      // nested calls don't consume them.
      self.flush_expr();
      self
        .saved_unary_spans
        .push(std::mem::take(&mut self.unary_spans));
      self.emit_node(Token::LParen);
    }
  }

  fn handle_lbrace_introducer(&mut self) {
    if self.state == ParserState::TemplateMode {
      // In template mode, { starts interpolation
      self.flush_expr();

      let node_index = self.emit_node(Token::LBrace);

      self.introducer_stack.push(Introducer {
        state: self.state,
        token: Token::LBrace,
        node_index,
        children_start: self.tree.nodes.len() as u32,
      });

      // Switch to expression mode for interpolation content
      self.state = ParserState::Expression;
    } else {
      // Normal block handling
      self.flush_expr();

      let node_index = self.emit_node(Token::LBrace);

      self.introducer_stack.push(Introducer {
        state: self.state,
        token: Token::LBrace,
        node_index,
        children_start: self.tree.nodes.len() as u32,
      });

      self.state = ParserState::Block;
    }
  }

  fn handle_lbracket(&mut self) {
    // [ can be:
    // 1. Array type: []int (in type annotation context)
    // 2. Array literal: [1, 2, 3] or [true * n]
    // 3. Array indexing: arr[i]

    // Check context
    if self.state == ParserState::TypeAnnotation {
      // This is array type syntax: []int
      // Buffer it for reordering with the identifier
      self
        .expr_buffer
        .push((Token::LBracket, self.current_span(), None));
      // Keep TypeAnnotation state for the type that follows
    } else if matches!(self.state, ParserState::Expression | ParserState::Block)
      && (!self.expr_buffer.is_empty() || self.last_was_value)
    {
      // Array indexing: emit the array name (if in buffer).
      // For chained indexing (a[i][j]), the buffer is empty
      // but last_was_value is true from the prior `]`.
      if let Some((tok, span, val)) = self.expr_buffer.pop() {
        self.emit_node_internal(tok, span, val);
      }
      self.last_was_value = false;

      // Park outer context while we parse the index.
      let saved_ops = std::mem::take(&mut self.operator_stack);
      let saved_buf = std::mem::take(&mut self.expr_buffer);
      let saved_unary = std::mem::take(&mut self.unary_spans);

      self.saved_op_stacks.push(saved_ops);
      self.saved_expr_bufs.push(saved_buf);
      self.saved_unary_spans.push(saved_unary);

      let node_index = self.emit_node(Token::LBracket);

      // Push as introducer for the index expression
      self.introducer_stack.push(Introducer {
        state: self.state,
        token: Token::LBracket,
        node_index,
        children_start: self.tree.nodes.len() as u32,
      });

      // Expect index expression
      self.state = ParserState::Expression;
    } else {
      // This is array literal: [1, 2, 3]
      self.flush_expr();

      let node_index = self.emit_node(Token::LBracket);

      // Push as introducer
      self.introducer_stack.push(Introducer {
        state: self.state,
        token: Token::LBracket,
        node_index,
        children_start: self.tree.nodes.len() as u32,
      });

      // Expect array elements
      self.state = ParserState::Expression;
    }
  }

  fn handle_rparen_closer(&mut self) {
    // Inside Fn(...) type annotation, buffer ) as part of type
    if self.type_paren_depth > 0 {
      self
        .expr_buffer
        .push((Token::RParen, self.current_span(), None));

      self.type_paren_depth -= 1;

      return;
    }

    self.flush_expr();

    // Check if we have a matching LParen introducer
    if let Some(introducer) = self.introducer_stack.last() {
      if introducer.token == Token::LParen {
        self.close_introducer();
        self.emit_node(Token::RParen);
      } else {
        self.emit_node(Token::RParen);
      }
    } else {
      self.emit_node(Token::RParen);
    }

    // Restore outer unary ops saved by LParen.
    if let Some(outer) = self.saved_unary_spans.pop() {
      self.unary_spans = outer;
    }

    // Drain any pending unary ops that apply to the
    // complete call/group result (e.g. `!f(x)`).
    while let Some((tok, sp)) = self.unary_spans.pop() {
      self.emit_node_internal(tok, sp, None);
    }

    // A closed paren is a value-producing context.
    self.last_was_value = true;
  }

  fn handle_rbrace_closer(&mut self) {
    self.flush_expr();

    // Inline closure boundary: `@click={fn() => expr}`.
    // The `}` belongs to the enclosing LBrace, not the Fn.
    // Close the Fn introducer first so the LBrace handler
    // below can find its matching LBrace.
    if let Some(top) = self.introducer_stack.last()
      && top.token == Token::Fn
    {
      self.close_introducer();
    }

    // Template interpolation directive: `{#html expr}`.
    // The `Hash` introducer is normally closed on `;`, but
    // inside a template interpolation there is no `;` — the
    // directive spans until the matching `}`. Close the Hash
    // introducer here so the enclosing LBrace is on top.
    if let Some(top) = self.introducer_stack.last()
      && top.token == Token::Hash
    {
      self.close_introducer();
    }

    // Check if we have a matching LBrace introducer
    if let Some(introducer) = self.introducer_stack.last() {
      if introducer.token == Token::LBrace {
        // Check if this was a template interpolation
        let was_template_interpolation =
          introducer.state == ParserState::TemplateMode;

        // Close the block introducer BEFORE emitting RBrace
        // This ensures RBrace is a sibling, not a child
        self.close_introducer();
        self.emit_node(Token::RBrace);

        // Return to template mode for interpolation.
        if was_template_interpolation {
          self.state = ParserState::TemplateMode;
          return;
        }

        // After closing block, check if we need to close a control flow
        // introducer
        if let Some(parent) = self.introducer_stack.last() {
          if parent.token == Token::If {
            // Only close the if introducer when there's no else clause
            // following This correctly handles if-else and
            // if-else-if chains
            if self.peek() != Some(Token::Else) {
              self.close_introducer();
            }
          } else if parent.token == Token::While || parent.token == Token::For {
            // While/For is complete after its block
            self.close_introducer();
          } else if parent.token == Token::When {
            // Ternary ends at block boundary
            self.close_introducer();
          } else if parent.token == Token::Match {
            // `match` is complete once its arm block `}` is
            // closed. Arm content is already flattened inside
            // the LBrace child.
            self.close_introducer();
          } else if matches!(parent.token, Token::Fun | Token::Fn | Token::Ext)
          {
            // Function/closure is complete after body.
            self.close_introducer();
          } else if matches!(
            parent.token,
            Token::Enum | Token::Struct | Token::Apply
          ) {
            self.close_introducer();
          } else if parent.token == Token::Dollar {
            // Style block is complete after its outer brace.
            self.close_introducer();
          }
        }
      } else {
        // Mismatched delimiter
        self.emit_node(Token::RBrace);
      }
    } else {
      // No matching introducer
      self.emit_node(Token::RBrace);
    }
  }

  fn handle_rbracket_closer(&mut self) {
    // Special case: in type annotation, ] is part of array type
    if self.state == ParserState::TypeAnnotation {
      // Buffer it for reordering
      self
        .expr_buffer
        .push((Token::RBracket, self.current_span(), None));
      // Stay in TypeAnnotation state for the element type
      return;
    }

    self.flush_expr();

    // Check if we have a matching LBracket introducer
    if let Some(introducer) = self.introducer_stack.last() {
      if introducer.token == Token::LBracket {
        // Emit RBracket
        self.emit_node(Token::RBracket);
        self.close_introducer();

        // Restore outer operator/expr context saved when
        // entering array indexing `[`.
        if let Some(ops) = self.saved_op_stacks.pop() {
          self.operator_stack = ops;
        }

        if let Some(buf) = self.saved_expr_bufs.pop() {
          self.expr_buffer = buf;
        }

        // Restore outer unary ops saved by LBracket.
        if let Some(outer) = self.saved_unary_spans.pop() {
          self.unary_spans = outer;
        }

        // Drain pending unary ops that apply to the
        // complete index result (e.g. `!v[0]`).
        while let Some((tok, sp)) = self.unary_spans.pop() {
          self.emit_node_internal(tok, sp, None);
        }
      } else {
        // Mismatched delimiter - emit anyway
        self.emit_node(Token::RBracket);
      }
    } else {
      // No matching introducer - just emit
      self.emit_node(Token::RBracket);
    }

    self.last_was_value = true;
  }

  fn handle_colon(&mut self) {
    // Ternary false-arm delimiter — flush the true
    // arm expression so operators land correctly.
    if self
      .introducer_stack
      .last()
      .is_some_and(|i| i.token == Token::When)
    {
      self.flush_expr();
      self.emit_node(Token::Colon);

      return;
    }

    if self.state == ParserState::ParameterList {
      // In parameter list, : starts type annotation
      // We need to reorder: `a : int` becomes `a int :`
      // The identifier should already be in expr_buffer
      // Don't flush yet - we'll collect the type first
      self.state = ParserState::TypeAnnotation;
    } else if self.state == ParserState::Expression {
      // Check if we're in a variable declaration context
      if let Some(introducer) = self.introducer_stack.last()
        && matches!(introducer.token, Token::Imu | Token::Mut | Token::Val)
      {
        // In variable declaration, start type annotation
        self.state = ParserState::TypeAnnotation;
        return;
      }

      // Otherwise treat as regular token
      self.handle_operand(Token::Colon);
    } else {
      // Otherwise treat as regular token
      self.handle_operand(Token::Colon);
    }
  }

  fn handle_colon_eq(&mut self) {
    // := operator for type inference in variable declarations
    self.flush_expr();
    self.emit_node(Token::ColonEq);

    self.state = ParserState::Expression; // Next expect initializer expression
  }

  fn handle_arrow(&mut self) {
    // Inside Fn(...) -> R type annotation, buffer ->
    if self.state == ParserState::TypeAnnotation {
      self
        .expr_buffer
        .push((Token::Arrow, self.current_span(), None));

      return;
    }

    // Arrow marks return type in function signature
    self.flush_expr();
    self.emit_node(Token::Arrow);

    self.state = ParserState::TypeAnnotation;
  }

  fn handle_fat_arrow(&mut self) {
    // Fat arrow marks inline closure body: fn(x) => expr
    self.flush_expr();
    self.emit_node(Token::FatArrow);

    self.state = ParserState::Expression;
  }

  fn handle_comma(&mut self) {
    // Inside Fn(T1, T2) type annotation, buffer comma
    if self.type_paren_depth > 0 {
      self
        .expr_buffer
        .push((Token::Comma, self.current_span(), None));

      return;
    }

    // Comma separates parameters, array elements, or expressions
    self.flush_expr();
    self.emit_node(Token::Comma);

    // Stay in current list-like state
    if self.state == ParserState::TypeAnnotation
      && self
        .introducer_stack
        .last()
        .map(|i| i.token == Token::LParen)
        .unwrap_or(false)
    {
      // After type annotation in parameter list, back to parameter list
      self.state = ParserState::ParameterList;
    } else if let Some(introducer) = self.introducer_stack.last()
      && introducer.token == Token::LBracket
    {
      // In array literal, continue with next element
      self.state = ParserState::Expression;
    }
  }

  fn handle_semicolon(&mut self) {
    self.flush_expr();
    self.emit_node(Token::Semicolon);

    // Close introducers that terminate on semicolon.
    // Loop handles nested closures: fn(x) => fn(y) => x + y;
    // closes inner Fn, outer Fn, then the binding (Imu/Mut/Val).
    while let Some(introducer) = self.introducer_stack.last() {
      match introducer.token {
        // Auto-close synthetic template fragments at
        // semicolon (for named tags like <h1>...</h1>;).
        Token::TemplateFragmentStart => {
          self.close_introducer();
        }
        Token::Fn | Token::When => {
          self.close_introducer();
        }
        Token::Return
        | Token::Imu
        | Token::Mut
        | Token::Val
        | Token::Hash
        | Token::Load
        | Token::Pack
        | Token::Ext
        | Token::Type
        | Token::Group => {
          self.close_introducer();
          break;
        }
        _ => break,
      }
    }
  }

  fn handle_assignment(&mut self) {
    // If we're in template mode, this is an attribute assignment
    if self.state == ParserState::TemplateMode {
      // Just emit the Eq token, stay in template mode
      self.emit_node(Token::Eq);
      // Stay in template mode to handle attribute value
      return;
    }

    // If we were in type annotation, complete it first
    if self.state == ParserState::TypeAnnotation {
      // We have buffered: [identifier, ...type tokens]
      // Emit as: identifier, type tokens, :
      // Take ownership to avoid borrow conflicts
      let mut buffer = std::mem::take(&mut self.expr_buffer);
      for (token, span, value) in buffer.drain(..) {
        self.emit_node_internal(token, span, value);
      }
      self.expr_buffer = buffer; // Put it back (now empty but with same capacity)
      // Emit the colon AFTER the type
      self.emit_node(Token::Colon);
    } else {
      // Normal assignment - flush left side
      self.flush_expr();
    }

    self.emit_node(Token::Eq);

    self.state = ParserState::Expression;
  }

  fn handle_compound_assignment(&mut self, kind: Token) {
    // Compound assignment operators: +=, -=, *=, etc.
    // Flush the left-hand side expression first
    self.flush_expr();
    self.emit_node(kind);

    // Next expect the right-hand side expression
    self.state = ParserState::Expression;
  }

  fn handle_unary_operator(&mut self, kind: Token) {
    // Unary prefix operators: !, -, *, & .
    // Stash them — they are emitted right after the next
    // operand in handle_operand (postfix order). This keeps
    // them completely out of the shunting-yard operator stack.
    let span = self.current_span();

    if self.state == ParserState::Expression || self.state == ParserState::Block
    {
      self.unary_spans.push((kind, span));
    } else {
      self.emit_node(kind);
    }

    if self.state != ParserState::Expression {
      self.state = ParserState::Expression;
    }
  }

  fn is_unary_context(&self) -> bool {
    // Check if we're in a context where an operator should be unary
    // Unary if:
    // 1. Expression buffer is empty (start of expression)
    // 2. Last token in buffer is an operator (e.g., "x + -y")
    // 3. Last token is an opening delimiter (e.g., "(-x)")
    // 4. Last token is a comma (e.g., "f(x, -y)")
    // 5. Last token is an assignment (e.g., "x = -y")

    if self.expr_buffer.is_empty() {
      return !self.last_was_value;
    }

    if let Some((last_token, _, _)) = self.expr_buffer.last() {
      matches!(
        last_token,
        Token::Eq
          | Token::PlusEq
          | Token::MinusEq
          | Token::StarEq
          | Token::SlashEq
          | Token::PercentEq
          | Token::AmpEq
          | Token::PipeEq
          | Token::CaretEq
          | Token::LShiftEq
          | Token::RShiftEq
          | Token::Plus
          | Token::PlusPlus
          | Token::Minus
          | Token::Star
          | Token::Slash
          | Token::Slash2
          | Token::Percent
          | Token::Lt
          | Token::LtEq
          | Token::Gt
          | Token::GtEq
          | Token::EqEq
          | Token::BangEq
          | Token::AmpAmp
          | Token::PipePipe
          | Token::LParen
          | Token::LBracket
          | Token::Comma
          | Token::ColonEq
          | Token::Return
      )
    } else {
      false
    }
  }

  fn handle_at(&mut self) {
    // Modifier syntax: ident@ident (e.g., check@lt).
    // Buffer the @ in expr_buffer so it stays ordered
    // with the preceding identifier and following modifier.
    if self.state == ParserState::Expression || self.state == ParserState::Block
    {
      let span = self.current_span();

      self.expr_buffer.push((Token::At, span, None));
    } else {
      self.emit_node(Token::At);
    }
  }

  fn handle_binding_keyword(&mut self, kind: Token) {
    // Variable declaration: imu/mut/val are introducers
    self.flush_expr();

    // imu/mut/val must be followed by an identifier.
    match self.peek() {
      Some(Token::Ident) => {}
      Some(_) => {
        self.error_at(ErrorKind::ExpectedIdentifier, self.pos + 1);
      }
      None => self.error(ErrorKind::ExpectedIdentifier),
    }

    // Emit the binding keyword as introducer
    let node_index = self.emit_node(kind);

    // Push as introducer - children will be: pattern, type (optional),
    // initializer
    self.introducer_stack.push(Introducer {
      state: self.state,
      token: kind,
      node_index,
      children_start: self.tree.nodes.len() as u32,
    });

    // Next expect identifier/pattern
    self.state = ParserState::Expression;
  }

  fn handle_if_keyword(&mut self) {
    // Flush any pending expression
    self.flush_expr();

    // `if` condition must not use parentheses.
    if self.peek() == Some(Token::LParen) {
      self.error_at(ErrorKind::ParenthesizedCondition, self.pos + 1);
    }

    // Emit 'if' as introducer
    let node_index = self.emit_node(Token::If);

    // Push as introducer - children will be: condition, then-block, else-block
    // (optional)
    self.introducer_stack.push(Introducer {
      state: self.state,
      token: Token::If,
      node_index,
      children_start: self.tree.nodes.len() as u32,
    });

    // Next expect condition expression
    self.state = ParserState::Expression;
  }

  fn handle_enum_keyword(&mut self) {
    self.flush_expr();

    // `enum` must be followed by an identifier (name).
    if self.peek().is_some_and(|n| n != Token::Ident) {
      self.error_at(ErrorKind::ExpectedIdentifier, self.pos + 1);
    }

    let node_index = self.emit_node(Token::Enum);

    self.introducer_stack.push(Introducer {
      state: self.state,
      token: Token::Enum,
      node_index,
      children_start: self.tree.nodes.len() as u32,
    });

    self.state = ParserState::Expression;
  }

  fn handle_struct_keyword(&mut self) {
    self.flush_expr();

    // `struct` must be followed by an identifier (name).
    if self.peek().is_some_and(|n| n != Token::Ident) {
      self.error_at(ErrorKind::ExpectedIdentifier, self.pos + 1);
    }

    let node_index = self.emit_node(Token::Struct);

    self.introducer_stack.push(Introducer {
      state: self.state,
      token: Token::Struct,
      node_index,
      children_start: self.tree.nodes.len() as u32,
    });

    self.state = ParserState::Expression;
  }

  fn handle_apply_keyword(&mut self) {
    self.flush_expr();

    // `apply` must be followed by an identifier (type name).
    if self.peek().is_some_and(|n| n != Token::Ident) {
      self.error_at(ErrorKind::ExpectedIdentifier, self.pos + 1);
    }

    let node_index = self.emit_node(Token::Apply);

    self.introducer_stack.push(Introducer {
      state: self.state,
      token: Token::Apply,
      node_index,
      children_start: self.tree.nodes.len() as u32,
    });

    self.state = ParserState::Expression;
  }

  fn handle_type_alias_keyword(&mut self) {
    self.flush_expr();

    // `type` must be followed by an identifier (alias name).
    if self.peek().is_some_and(|n| n != Token::Ident) {
      self.error_at(ErrorKind::ExpectedIdentifier, self.pos + 1);
    }

    let node_index = self.emit_node(Token::Type);

    self.introducer_stack.push(Introducer {
      state: self.state,
      token: Token::Type,
      node_index,
      children_start: self.tree.nodes.len() as u32,
    });

    self.state = ParserState::Expression;
  }

  fn handle_group_keyword(&mut self) {
    self.flush_expr();

    let node_index = self.emit_node(Token::Group);

    self.introducer_stack.push(Introducer {
      state: self.state,
      token: Token::Group,
      node_index,
      children_start: self.tree.nodes.len() as u32,
    });

    self.state = ParserState::Expression;
  }

  fn handle_and_keyword(&mut self) {
    self.flush_expr();

    // Close the current Type introducer inside a Group.
    if self
      .introducer_stack
      .last()
      .is_some_and(|i| i.token == Token::Type)
    {
      self.close_introducer();
    }

    self.emit_node(Token::And);

    self.state = ParserState::Expression;
  }

  fn handle_when_keyword(&mut self) {
    self.flush_expr();

    let node_index = self.emit_node(Token::When);

    self.introducer_stack.push(Introducer {
      state: self.state,
      token: Token::When,
      node_index,
      children_start: self.tree.nodes.len() as u32,
    });

    self.state = ParserState::Expression;
  }

  /// `match expr { pat => body, pat => body, ... }`. Mirrors
  /// the `if` / `while` / `when` shape: emit `Token::Match`
  /// as an introducer, parse the scrutinee expression next,
  /// then let `handle_lbrace_introducer` create the block for
  /// the arm list. Arms are flat children inside the LBrace
  /// separated by `FatArrow` + `Comma` markers — the executor
  /// groups them at lowering time.
  fn handle_match_keyword(&mut self) {
    self.flush_expr();

    let node_index = self.emit_node(Token::Match);

    self.introducer_stack.push(Introducer {
      state: self.state,
      token: Token::Match,
      node_index,
      children_start: self.tree.nodes.len() as u32,
    });

    self.state = ParserState::Expression;
  }

  fn handle_else_keyword(&mut self) {
    self.flush_expr();
    self.emit_node(Token::Else);

    // Next could be another if or a block
    self.state = ParserState::Expression;
  }

  fn handle_while_keyword(&mut self) {
    // Flush any pending expression
    self.flush_expr();

    // `while` condition must not use parentheses.
    if self.peek() == Some(Token::LParen) {
      self.error_at(ErrorKind::ParenthesizedCondition, self.pos + 1);
    }

    // Emit 'while' as introducer
    let node_index = self.emit_node(Token::While);

    // Push as introducer - children will be: condition, body-block
    self.introducer_stack.push(Introducer {
      state: self.state,
      token: Token::While,
      node_index,
      children_start: self.tree.nodes.len() as u32,
    });

    // Next expect condition expression
    self.state = ParserState::Expression;
  }

  fn handle_for_keyword(&mut self) {
    // Flush any pending expression
    self.flush_expr();

    // Emit 'for' as introducer
    let node_index = self.emit_node(Token::For);

    // Push as introducer - children will be: iterator variable, range,
    // body-block
    self.introducer_stack.push(Introducer {
      state: self.state,
      token: Token::For,
      node_index,
      children_start: self.tree.nodes.len() as u32,
    });

    // Next expect iterator variable
    self.state = ParserState::Expression;
  }

  fn handle_return_keyword(&mut self) {
    // Flush any pending expression
    self.flush_expr();

    // Emit 'return' as introducer (might have expression child)
    let node_index = self.emit_node(Token::Return);

    // Push as introducer for optional return value
    self.introducer_stack.push(Introducer {
      state: self.state,
      token: Token::Return,
      node_index,
      children_start: self.tree.nodes.len() as u32,
    });

    // Next might be expression or semicolon
    self.state = ParserState::Expression;
  }

  fn handle_directive(&mut self) {
    // Directives follow pattern: #identifier expression
    // Examples: #run foobar(), #dom <>, #inline

    // Flush any pending expression
    self.flush_expr();

    // Emit the Hash token as introducer
    let node_index = self.emit_node(Token::Hash);

    // Push as introducer - the identifier and expression will be children
    self.introducer_stack.push(Introducer {
      state: self.state,
      token: Token::Hash,
      node_index,
      children_start: self.tree.nodes.len() as u32,
    });

    // Next token should be the directive identifier (run, dom, inline, etc.)
    // Then optionally an expression
    self.state = ParserState::Expression;
  }

  fn handle_type(&mut self, kind: Token) {
    if self.state == ParserState::TypeAnnotation {
      // Collect type for reordering with ':'
      // Don't emit yet - we might have array types like []int
      self.expr_buffer.push((kind, self.current_span(), None));
      // Stay in TypeAnnotation state
    } else {
      // Regular type in expression
      self.handle_operand(kind);
    }
  }

  fn handle_template_assign(&mut self) {
    // ::= operator for template literal assignment
    self.flush_expr();
    self.emit_node(Token::TemplateAssign);

    self.state = ParserState::TemplateMode;

    // If the next token is a named tag (LAngle, not
    // TemplateFragmentStart), auto-wrap in a synthetic
    // fragment so execute_template_fragment handles all
    // template content uniformly.
    if self.peek() == Some(Token::LAngle) {
      let node_index = self.emit_node(Token::TemplateFragmentStart);

      self.introducer_stack.push(Introducer {
        state: self.state,
        token: Token::TemplateFragmentStart,
        node_index,
        children_start: self.tree.nodes.len() as u32,
      });
    }
  }

  fn handle_template_fragment_start(&mut self) {
    // <> starts a template fragment
    let node_index = self.emit_node(Token::TemplateFragmentStart);

    // Push as introducer for fragment content
    self.introducer_stack.push(Introducer {
      state: self.state,
      token: Token::TemplateFragmentStart,
      node_index,
      children_start: self.tree.nodes.len() as u32,
    });

    self.state = ParserState::TemplateMode;
  }

  fn handle_template_fragment_end(&mut self) {
    // </> ends a template fragment (only appears in template mode)
    // In code mode, </> is tokenized as TemplateType instead

    self.flush_expr();
    self.emit_node(Token::TemplateFragmentEnd);

    // Close the fragment introducer
    if let Some(introducer) = self.introducer_stack.last()
      && introducer.token == Token::TemplateFragmentStart
    {
      self.close_introducer();
    }
  }

  fn handle_langle(&mut self) {
    if self.state == ParserState::TemplateMode {
      // In template mode, < starts a tag
      self.flush_expr();
      self.emit_node(Token::LAngle);
      // Next token should be tag name
    } else {
      // In normal code, < is less-than operator
      self.handle_operator(Token::Lt);
    }
  }

  fn handle_rangle(&mut self) {
    if self.state == ParserState::TemplateMode {
      // In template mode, > ends tag opening
      self.flush_expr();
      self.emit_node(Token::RAngle);
    } else {
      // In normal code, > is greater-than operator
      self.handle_operator(Token::Gt);
    }
  }

  fn handle_slash(&mut self) {
    if self.state == ParserState::TemplateMode {
      // In template mode, emit whatever slash token we received
      let kind = self.tokens.kinds[self.pos];

      self.emit_node(kind);
    } else {
      // In normal code, / is division operator
      let kind = self.tokens.kinds[self.pos];

      self.handle_operator(kind);
    }
  }

  /// must handle emit_node instead.
  fn handle_template_text(&mut self) {
    // Raw text content in templates
    let span = self.current_span();
    let value = self.extract_value(Token::TemplateText);
    self.emit_node_internal(Token::TemplateText, span, value);
  }

  /// Handles `$: { ... }` style blocks.
  ///
  /// Checks if the previous node was `Pub` to determine global
  /// scope. Emits Dollar as an introducer whose children are
  /// the style rule tokens (selectors, properties, values).
  /// Handles `$: { ... }` style blocks.
  ///
  /// Checks if the previous node was `Pub` to determine global
  /// scope. Emits Dollar as an introducer whose children are
  /// the style rule tokens (selectors, properties, values).
  fn handle_style_block(&mut self) {
    self.flush_expr();

    // Emit Dollar as the style block introducer.
    let node_index = self.emit_node(Token::Dollar);

    self.introducer_stack.push(Introducer {
      state: self.state,
      token: Token::Dollar,
      node_index,
      children_start: self.tree.nodes.len() as u32,
    });

    self.state = ParserState::StyleBlock;

    // Skip the Colon (already peeked).
    self.pos += 1;
    self.emit_node(Token::Colon);
  }

  fn handle_operator(&mut self, kind: Token) {
    // Add to operator stack for Shunting Yard
    if let Some((prec, assoc)) = self.op_precedence(kind) {
      // Pop higher precedence operators
      while let Some(&(op_token, op_prec, _op_assoc)) =
        self.operator_stack.last()
      {
        let should_pop = if assoc == 0 {
          // Left-associative: pop if stack has higher OR equal precedence
          op_prec >= prec
        } else {
          // Right-associative: pop only if stack has strictly higher precedence
          op_prec > prec
        };

        if should_pop {
          self.operator_stack.pop();
          // Find the operator and move it to the end of buffer.
          if let Some(pos) = self
            .expr_buffer
            .iter()
            .rposition(|(t, _, _)| *t == op_token)
          {
            let op = self.expr_buffer.remove(pos);
            self.expr_buffer.push(op);
          }
        } else {
          break;
        }
      }

      // Push current operator
      self.operator_stack.push((kind, prec, assoc));
      // Buffer the operator (will be reordered later)
      self.expr_buffer.push((kind, self.current_span(), None));
    } else {
      // Not an operator we recognize
      self.handle_operand(kind);
    }
  }

  fn handle_operand(&mut self, kind: Token) {
    let span = self.current_span();
    let value = self.extract_value(kind);

    // In template mode, emit directly (for attribute values, etc.)
    if self.state == ParserState::TemplateMode {
      self.emit_node_internal(kind, span, value);
      return;
    }

    // In expression context, buffer for potential reordering
    if self.state == ParserState::Expression
      || self.state == ParserState::TypeAnnotation
      || (self.state == ParserState::Block && !kind.is_keyword())
    {
      self.last_was_value = false;
      self.expr_buffer.push((kind, span, value));

      // Emit pending unary operators right after the operand
      // (postfix order). But NOT if the next token starts a
      // call `(` or index `[` — the unary applies to the
      // complete result, not the bare name.
      let next = self.peek();

      if next != Some(Token::LParen) && next != Some(Token::LBracket) {
        while let Some((tok, sp)) = self.unary_spans.pop() {
          self.expr_buffer.push((tok, sp, None));
        }
      }
    } else {
      // Direct emission
      self.emit_node_internal(kind, span, value);
    }
  }

  fn flush_expr(&mut self) {
    if self.expr_buffer.is_empty() {
      return;
    }

    self.last_was_value = false;

    // Pop remaining binary operators from stack.
    while let Some((op_token, _, _)) = self.operator_stack.pop() {
      if let Some(pos) = self
        .expr_buffer
        .iter()
        .rposition(|(t, _, _)| *t == op_token)
      {
        let op = self.expr_buffer.remove(pos);
        self.expr_buffer.push(op);
      }
    }

    // Emit all buffered nodes
    // Take ownership to avoid borrow conflicts
    let mut buffer = std::mem::take(&mut self.expr_buffer);
    for (token, span, value) in buffer.drain(..) {
      self.emit_node_internal(token, span, value);
    }
    self.expr_buffer = buffer; // Put it back (now empty but with same capacity)

    self.operator_stack.clear();
  }

  fn close_introducer(&mut self) {
    if let Some(introducer) = self.introducer_stack.pop() {
      // Set children range for the introducer node
      let children_end = self.tree.nodes.len() as u32;
      let children_count = children_end - introducer.children_start;

      if children_count > 0 {
        self.tree.set_children(
          introducer.node_index,
          introducer.children_start,
          children_count as u16,
        );
      }

      self.state = introducer.state;
    }
  }

  fn emit_node(&mut self, kind: Token) -> u32 {
    let span = self.current_span();
    let value = self.extract_value(kind);

    self.emit_node_internal(kind, span, value)
  }

  fn emit_node_internal(
    &mut self,
    kind: Token,
    span: Span,
    value: Option<NodeValue>,
  ) -> u32 {
    match value {
      Some(v) => self.tree.push_node_with_value(kind, span, v),
      None => self.tree.push_node(kind, span),
    }
  }

  fn extract_value(&mut self, kind: Token) -> Option<NodeValue> {
    match kind {
      Token::Ident => {
        let lit_idx = self.tokens.literal_indices[self.pos];
        let symbol = self.literals.identifiers[lit_idx as usize];

        Some(NodeValue::Symbol(symbol))
      }
      Token::TemplateText => {
        // Template text is now interned like identifiers
        let lit_idx = self.tokens.literal_indices[self.pos];
        let symbol = self.literals.identifiers[lit_idx as usize];

        Some(NodeValue::Symbol(symbol))
      }
      Token::InterpString => {
        // Packed: low 16 = string_literals idx,
        // high 16 = interp_ranges idx.
        let packed = self.tokens.literal_indices[self.pos];
        // Store packed value so executor can unpack both.
        Some(NodeValue::Literal(packed))
      }
      Token::String | Token::RawString | Token::StyleValue => {
        let lit_idx = self.tokens.literal_indices[self.pos];
        let symbol = self.literals.string_literals[lit_idx as usize];

        Some(NodeValue::Symbol(symbol))
      }
      Token::Int | Token::Float | Token::Char | Token::Bytes => {
        Some(NodeValue::Literal(self.tokens.literal_indices[self.pos]))
      }
      _ => None,
    }
  }

  #[inline(always)]
  fn is_operator(&self, kind: Token) -> bool {
    self.op_precedence(kind).is_some()
  }

  #[inline(always)]
  const fn op_precedence(&self, token: Token) -> Option<(u8, u8)> {
    // Precedence table: (precedence, associativity)
    // Higher precedence = tighter binding
    // associativity: 0 = left-to-right, 1 = right-to-left
    const PREC_TABLE: [(u8, u8); 256] = {
      let mut table = [(0, 0); 256];
      table[Token::PipePipe as usize] = (1, 0);
      table[Token::AmpAmp as usize] = (2, 0);
      table[Token::EqEq as usize] = (3, 0);
      table[Token::BangEq as usize] = (3, 0);
      table[Token::Lt as usize] = (4, 0);
      table[Token::LtEq as usize] = (4, 0);
      table[Token::Gt as usize] = (4, 0);
      table[Token::GtEq as usize] = (4, 0);
      table[Token::Pipe as usize] = (5, 0);
      table[Token::Caret as usize] = (6, 0);
      table[Token::Amp as usize] = (7, 0);
      table[Token::LShift as usize] = (8, 0);
      table[Token::RShift as usize] = (8, 0);
      table[Token::DotDot as usize] = (2, 0); // Range has low precedence
      table[Token::DotDotEq as usize] = (2, 0); // so a+b..c+d works correctly
      table[Token::Plus as usize] = (9, 0);
      table[Token::PlusPlus as usize] = (9, 0);
      table[Token::Minus as usize] = (9, 0);
      table[Token::Star as usize] = (10, 0);
      table[Token::Slash as usize] = (10, 0);
      table[Token::Percent as usize] = (10, 0);
      table[Token::Dot as usize] = (12, 0); // Member access has highest precedence
      table
    };

    let (prec, assoc) = PREC_TABLE[token as usize];
    if prec > 0 { Some((prec, assoc)) } else { None }
  }

  pub fn debug_print_tree(&self) {
    println!("\n——— Parse Tree (State Machine with Introducers) ———");

    for (i, node) in self.tree.nodes.iter().enumerate() {
      let token_str = format!("{:?}", node.token);

      let value_str = match self.tree.value(i as u32) {
        None => "".into(),
        Some(NodeValue::Symbol(sym)) => format!(" [sym:{sym}]"),
        Some(NodeValue::Literal(idx)) => format!(" [lit:{idx}]"),
        Some(NodeValue::TextRange(start, len)) => {
          let text =
            &self.source[start as usize..(start + len as u32) as usize];

          format!(" [text:'{text}']")
        }
      };

      let children_str = if node.child_count > 0 {
        format!(
          " (children: {}..{})",
          node.child_start,
          node.child_start + node.child_count
        )
      } else {
        "".into()
      };

      println!("{i:3}: {token_str:<20}{value_str}{children_str}");
    }
  }
}
