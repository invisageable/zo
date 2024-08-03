use super::precedence::Precedence;

use zo_ast::ast::{Ast, BinOp, Expr, ExprKind, Lit, LitKind, Stmt, StmtKind};

use zo_interner::interner::Interner;
use zo_reporter::reporter::Reporter;
use zo_reporter::{error, Result};
use zo_session::session::Session;
use zo_tokenizer::token::punctuation::Punctuation;
use zo_tokenizer::token::{Token, TokenKind};

use swisskit::span::Span;

use smol_str::ToSmolStr;

type PrefixFn = Box<dyn FnOnce(&mut Parser) -> Result<Expr>>;
type InfixFn = Box<dyn FnOnce(&mut Parser, Expr) -> Result<Expr>>;

/// The representation of a parser.
struct Parser<'tokens> {
  interner: &'tokens mut Interner,
  reporter: &'tokens Reporter,
  tokens: &'tokens [Token],
  /// The index of the token slice.
  index: usize,
  /// An optional current token.
  maybe_token_current: Option<&'tokens Token>,
  /// An optional next token.
  maybe_token_next: Option<&'tokens Token>,
  /// The current span.
  span_current: Span,
}

impl<'tokens> Parser<'tokens> {
  /// Creates a new parser instance from tokens, interner and reporter.
  #[inline]
  fn new(
    tokens: &'tokens [Token],
    interner: &'tokens mut Interner,
    reporter: &'tokens Reporter,
  ) -> Self {
    Self {
      tokens,
      interner,
      reporter,
      index: 0usize,
      maybe_token_current: None,
      maybe_token_next: None,
      span_current: Span::ZERO,
    }
  }

  /// Checks token availability.
  #[inline]
  fn has_tokens(&self) -> bool {
    self.index < self.tokens.len()
  }

  /// Peeks ahead in the token stram to look at a token based of the index.
  #[inline]
  fn peek(&self) -> Option<&'tokens Token> {
    self.tokens.get(self.index)
  }

  /// Chekcs and reveals the precendence ordering from other precedence.
  #[inline]
  fn should_precedence_has_priority(&mut self, precedence: Precedence) -> bool {
    precedence < Precedence::from(self.maybe_token_next)
  }

  /// Gets the current span.
  #[inline]
  fn current_span(&mut self) -> Span {
    self
      .maybe_token_current
      .map(|token| token.span)
      .unwrap_or_default()
  }

  /// Transforms a collection of tokens into an abstract syntax tree.
  ///
  /// #### result.
  ///
  /// The resulting is an AST.
  fn parse(&mut self) -> Result<Ast> {
    let mut ast = Ast::new();

    if self.tokens.is_empty() {
      return Ok(ast);
    }

    self.next();
    self.next();

    while self.has_tokens() {
      match self.parse_stmt() {
        Ok(stmt) => ast.add_stmt(stmt),
        Err(report_error) => self.reporter.add_report(report_error),
      }

      self.next();
    }

    self.reporter.abort_if_has_errors();

    Ok(ast)
  }

  /// Parses a statement.
  fn parse_stmt(&mut self) -> Result<Stmt> {
    let stmt = self
      .maybe_token_current
      .map(|token| match token.kind {
        _ => self.parse_stmt_expr(),
      })
      .unwrap()?;

    Ok(stmt)
  }

  /// Parses an expression statement.
  fn parse_stmt_expr(&mut self) -> Result<Stmt> {
    let lo = self.current_span();
    let expr = self.parse_expr(Precedence::Low)?;

    self
      .maybe_token_next
      .map(|token| {
        if token.is(TokenKind::Punctuation(Punctuation::Semicolon)) {
          self.next();
        }
      })
      .unwrap_or_default();

    let hi = self.current_span();
    let span = Span::merge(lo, hi);

    Ok(Stmt {
      kind: StmtKind::Expr(Box::new(expr)),
      span,
    })
  }

  /// Parses an expression.
  fn parse_expr(&mut self, precedence: Precedence) -> Result<Expr> {
    self
      .parse_prefix_fn()
      .map(|parse_prefix| {
        let mut lhs = parse_prefix(self)?;

        while self.has_tokens()
          && self.should_precedence_has_priority(precedence)
        {
          if let Ok(parse_infix) = self.parse_infix_fn() {
            self.next();

            lhs = parse_infix(self, lhs)?;
          } else {
            return Ok(lhs);
          }
        }

        Ok(lhs)
      })
      .unwrap()
  }

  /// Gets the prefix function.
  fn parse_prefix_fn(&self) -> Option<PrefixFn> {
    let token = self.maybe_token_current.unwrap();

    match token.kind {
      TokenKind::Int(..) => Some(Box::new(Self::parse_expr_lit_int)),
      _ => None,
    }
  }

  /// Parses an integer literal.
  fn parse_expr_lit_int(parser: &mut Parser) -> Result<Expr> {
    parser
      .maybe_token_current
      .map(|token| match token.kind {
        TokenKind::Int(symbol, _) => Ok(Expr {
          kind: ExprKind::Lit(Lit {
            kind: LitKind::Int(symbol),
            span: token.span,
          }),
          span: token.span,
        }),
        _ => Err(error::syntax::expected_int(token.span, token.to_smolstr())),
      })
      .unwrap()
  }

  /// Gets the infix function.
  fn parse_infix_fn(&self) -> Result<InfixFn> {
    let token = self.maybe_token_next.unwrap(); // should be unwrap properly.

    match token.kind {
      k if k.is_binop() => Ok(Box::new(Self::parse_expr_binop)),
      _ => Err(error::syntax::invalid_infix(token.span, token.to_smolstr())),
    }
  }

  /// Parses a binop expression.
  fn parse_expr_binop(parser: &mut Parser, lhs: Expr) -> Result<Expr> {
    let lo = lhs.span;

    let (precedence, maybe_binop) = parser
      .maybe_token_current
      .map(|token| {
        let binop = BinOp::from(token);

        match token.kind {
          k if k.is_assignement() => (Precedence::Assignement, Some(binop)),
          k if k.is_conditional() => (Precedence::Conditional, Some(binop)),
          k if k.is_comparison() => (Precedence::Comparison, Some(binop)),
          k if k.is_sum() => (Precedence::Sum, Some(binop)),
          k if k.is_exponent() => (Precedence::Exponent, Some(binop)),
          k if k.is_calling() => (Precedence::Calling, None),
          k if k.is_index() => (Precedence::Index, None),
          _ => (Precedence::Low, None),
        }
      })
      .unwrap();

    parser.next();

    let rhs = parser.parse_expr(precedence)?;
    let binop = maybe_binop.unwrap();
    let hi = parser.current_span();
    let span = Span::merge(lo, hi);

    Ok(Expr {
      kind: ExprKind::BinOp(binop, Box::new(lhs), Box::new(rhs)),
      span,
    })
  }
}

impl<'tokens> Iterator for Parser<'tokens> {
  type Item = &'tokens Token;

  fn next(&mut self) -> Option<Self::Item> {
    std::mem::swap(&mut self.maybe_token_current, &mut self.maybe_token_next);

    match self.peek() {
      None => None,
      Some(token_current) => {
        self.index += 1;
        self.span_current = token_current.span;
        self.maybe_token_next = Some(token_current);

        Some(token_current)
      }
    }
  }
}

/// A wrapper of [`Parser::new`] and [`Parser::parse`].
///
/// ```ignore
/// use zo_parser::parser;
/// use zo_session::session::Session;
/// use zo_tokenizer::tokenizer;
///
/// let mut session = Session::default();
/// let tokens = tokenizer::tokenize(&mut session, b"4 + 2");
///
/// parser::parse(&mut session, &tokens);
/// ```
pub fn parse(session: &mut Session, tokens: &[Token]) -> Result<Ast> {
  Parser::new(tokens, &mut session.interner, &session.reporter).parse()
}
