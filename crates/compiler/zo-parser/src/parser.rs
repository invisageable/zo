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
}
impl<'a> Parser<'a> {
  /// Creates a new [`Parser`] instance.
  pub fn new(tokenization: &'a TokenizationResult, source: &'a str) -> Self {
    Self {
      source,
      tokens: &tokenization.tokens,
      literals: &tokenization.literals,
      tree: Tree::new(),
      pos: 0,
      state: ParserState::TopLevel,
      introducer_stack: Vec::with_capacity(32),
      expr_buffer: Vec::with_capacity(64),
      operator_stack: Vec::with_capacity(16),
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
    match kind {
      // Introducers - these start new contexts
      Token::Fun => self.handle_fun_introducer(),
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
        self.handle_unary_operator(kind)
      }
      Token::Star | Token::Amp if self.is_unary_context() => {
        self.handle_unary_operator(kind)
      }

      // Binary operators
      _ if self.is_operator(kind) => self.handle_operator(kind),

      // Operands (identifiers, literals)
      _ if kind.is_operand() => self.handle_operand(kind),

      // Keywords that introduce statements
      Token::Imu | Token::Mut | Token::Val => self.handle_binding_keyword(kind),

      // Control flow keywords
      Token::If => self.handle_if_keyword(),
      Token::Else => self.handle_else_keyword(),
      Token::While => self.handle_while_keyword(),
      Token::For => self.handle_for_keyword(),
      Token::Return => self.handle_return_keyword(),

      // Directives
      Token::Hash => self.handle_directive(),

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

      // Everything else gets emitted as-is
      _ => {
        self.emit_node(kind);
      }
    }
  }

  fn handle_fun_introducer(&mut self) {
    // Flush any pending expression first
    self.flush_expr();

    // Emit the Fun token
    let node_index = self.emit_node(Token::Fun);

    // Push introducer to stack
    self.introducer_stack.push(Introducer {
      state: self.state,
      token: Token::Fun,
      node_index,
      children_start: self.tree.nodes.len() as u32,
    });

    // Transition to function signature state
    self.state = ParserState::FunctionSignature;
  }

  fn handle_lparen_introducer(&mut self) {
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
      // For function/method calls, flush the expression first then emit LParen
      self.flush_expr();
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
    } else if !self.expr_buffer.is_empty()
      && self.state == ParserState::Expression
    {
      // We have a pending expression - this is array indexing
      // Flush the array name first
      self.flush_expr();

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
    self.flush_expr();

    // Check if we have a matching LParen introducer
    if let Some(introducer) = self.introducer_stack.last() {
      if introducer.token == Token::LParen {
        // Close the parameter list introducer BEFORE emitting RParen
        // This ensures RParen is a sibling, not a child
        self.close_introducer();

        // Now emit RParen as a sibling
        self.emit_node(Token::RParen);
      } else {
        // Mismatched delimiter - emit anyway but don't pop
        self.emit_node(Token::RParen);
      }
    } else {
      // No matching introducer - just emit
      self.emit_node(Token::RParen);
    }
  }

  fn handle_rbrace_closer(&mut self) {
    self.flush_expr();

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

        // If we were in template interpolation, return to template mode
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
      } else {
        // Mismatched delimiter - emit anyway
        self.emit_node(Token::RBracket);
      }
    } else {
      // No matching introducer - just emit
      self.emit_node(Token::RBracket);
    }
  }

  fn handle_colon(&mut self) {
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
    // Arrow marks return type in function signature
    self.flush_expr();
    self.emit_node(Token::Arrow);

    self.state = ParserState::TypeAnnotation;
  }

  fn handle_comma(&mut self) {
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

    // Check if we need to close a statement introducer
    if let Some(introducer) = self.introducer_stack.last() {
      match introducer.token {
        Token::Return | Token::Imu | Token::Mut | Token::Val | Token::Hash => {
          self.close_introducer();
        }
        _ => {}
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
    // Unary operators: !, -, *, & (in prefix position)
    // These are prefix operators that apply to the following expression

    // Don't flush expression buffer - unary ops have high precedence
    // and should be part of the current expression
    let span = self.current_span();

    // In expression context, buffer the unary operator
    if self.state == ParserState::Expression || self.state == ParserState::Block
    {
      self.expr_buffer.push((kind, span, None));
    } else {
      // Direct emission in other contexts
      self.emit_node(kind);
    }

    // Stay in expression state to parse the operand
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
      return true;
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

  fn handle_binding_keyword(&mut self, kind: Token) {
    // Variable declaration: imu/mut/val are introducers
    self.flush_expr();

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

  fn handle_else_keyword(&mut self) {
    self.flush_expr();
    self.emit_node(Token::Else);

    // Next could be another if or a block
    self.state = ParserState::Expression;
  }

  fn handle_while_keyword(&mut self) {
    // Flush any pending expression
    self.flush_expr();

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
          // Find the operator and move it to the end of buffer
          if let Some(pos) =
            self.expr_buffer.iter().position(|(t, _, _)| *t == op_token)
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
      self.expr_buffer.push((kind, span, value));
    } else {
      // Direct emission
      self.emit_node_internal(kind, span, value);
    }
  }

  fn flush_expr(&mut self) {
    if self.expr_buffer.is_empty() {
      return;
    }

    // Pop remaining operators from stack
    while let Some((op_token, _, _)) = self.operator_stack.pop() {
      // Find operator in buffer and move it to the end
      if let Some(pos) =
        self.expr_buffer.iter().position(|(t, _, _)| *t == op_token)
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
      Token::Bytes => {
        let start = self.tokens.starts[self.pos];
        let len = self.tokens.lengths[self.pos];
        Some(NodeValue::TextRange(start, len))
      }
      Token::TemplateText => {
        // Template text is now interned like identifiers
        let lit_idx = self.tokens.literal_indices[self.pos];
        let symbol = self.literals.identifiers[lit_idx as usize];
        Some(NodeValue::Symbol(symbol))
      }
      Token::String | Token::RawString => {
        let lit_idx = self.tokens.literal_indices[self.pos];
        let symbol = self.literals.string_literals[lit_idx as usize];
        Some(NodeValue::Symbol(symbol))
      }
      Token::Int | Token::Float | Token::Char => {
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
