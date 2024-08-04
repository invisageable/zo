use super::precedence::Precedence;

use zo_ast::ast::{
  Ast, BinOp, Expr, ExprKind, Lit, LitKind, Stmt, StmtKind, UnOp, UnOpKind,
};

use zo_interner::interner::Interner;
use zo_reporter::reporter::Reporter;
use zo_reporter::{error, Result};
use zo_session::session::Session;
use zo_tokenizer::token::group::Group;
use zo_tokenizer::token::kw::Kw;
use zo_tokenizer::token::punctuation::Punctuation;
use zo_tokenizer::token::{Token, TokenKind};

use swisskit::span::Span;

use smol_str::ToSmolStr;

type PrefixFn = Box<dyn FnOnce(&mut Parser) -> Result<Expr>>;
type InfixFn = Box<dyn FnOnce(&mut Parser, Expr) -> Result<Expr>>;

/// The representation of a parser.
struct Parser<'tokens> {
  /// An interner — see also [`Interner`] for more information.
  interner: &'tokens mut Interner,
  /// A reporter — see also [`Reporter`] for more information.
  reporter: &'tokens Reporter,
  /// A group of tokens — see also [`Token`] for more information.
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
  fn should_precedence(&mut self, precedence: Precedence) -> bool {
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

  /// Checks if the current token is a specific kind.
  #[inline]
  fn ensure(&mut self, kind: TokenKind) -> bool {
    self
      .maybe_token_current
      .map(|token| token.is(kind))
      .unwrap()
  }

  /// Checks if the next token is a specific kind.
  #[inline]
  fn ensure_peek(&mut self, kind: TokenKind) -> bool {
    self.maybe_token_next.map(|token| token.is(kind)).unwrap()
  }

  /// Moves only if the current token is a specific kind.
  #[inline]
  fn expect(&mut self, kind: TokenKind) -> Result<()> {
    self
      .maybe_token_current
      .map(|token| {
        if token.is(kind) {
          self.next();

          return Ok(());
        }

        Err(error::syntax::unexpected_token(
          token.span,
          token.to_smolstr(),
        ))
      })
      .unwrap()
  }

  /// Moves only if the next token is a specific kind.
  #[inline]
  fn expect_peek(&mut self, kind: TokenKind) -> Result<()> {
    self
      .maybe_token_next
      .map(|token| {
        if token.is(kind) {
          self.next();

          return Ok(());
        }

        Err(error::syntax::unexpected_token(
          token.span,
          token.to_smolstr(),
        ))
      })
      .unwrap()
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
        Err(error) => self.reporter.add_report(error),
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

    if self.ensure_peek(TokenKind::Punctuation(Punctuation::Semicolon)) {
      self.next();
    }

    Ok(stmt)
  }

  /// Parses an expression statement.
  fn parse_stmt_expr(&mut self) -> Result<Stmt> {
    let lo = self.current_span();
    let expr = self.parse_expr(Precedence::Low)?;
    let hi = self.current_span();
    let span = Span::merge(lo, hi);

    Ok(Stmt {
      kind: StmtKind::Expr(Box::new(expr)),
      span,
    })
  }

  /// Parses an expression.
  fn parse_expr(&mut self, precedence: Precedence) -> Result<Expr> {
    self.parse_prefix_fn().and_then(|prefix_fn| {
      let mut lhs = prefix_fn(self)?;

      while self.has_tokens() && self.should_precedence(precedence) {
        if let Ok(infix_fn) = self.parse_infix_fn() {
          self.next();

          lhs = infix_fn(self, lhs)?;
        } else {
          return Ok(lhs);
        }
      }

      Ok(lhs)
    })
  }

  /// Gets the prefix function.
  fn parse_prefix_fn(&self) -> Result<PrefixFn> {
    let token = self.maybe_token_current.unwrap();

    match token.kind {
      TokenKind::Int(..) => Ok(Box::new(Self::parse_expr_lit_int)),
      TokenKind::Float(..) => Ok(Box::new(Self::parse_expr_lit_float)),
      TokenKind::Ident(..) => Ok(Box::new(Self::parse_expr_ident)),
      TokenKind::Kw(Kw::False) | TokenKind::Kw(Kw::True) => {
        Ok(Box::new(Self::parse_expr_lit_bool))
      }
      TokenKind::Punctuation(Punctuation::Minus)
      | TokenKind::Punctuation(Punctuation::Exclamation) => {
        Ok(Box::new(Self::parse_expr_unop))
      }
      TokenKind::Group(Group::ParenOpen) => {
        Ok(Box::new(Self::parse_expr_group))
      }
      TokenKind::Group(Group::BracketOpen) => {
        Ok(Box::new(Self::parse_expr_array))
      }
      _ => Err(error::syntax::invalid_prefix(
        token.span,
        token.to_smolstr(),
      )),
    }
  }

  /// Parses an integer literal.
  fn parse_expr_lit_int(parser: &mut Parser) -> Result<Expr> {
    parser
      .maybe_token_current
      .map(|token| match token.kind {
        TokenKind::Int(sym, base) => Ok(Expr {
          kind: ExprKind::Lit(Lit {
            kind: LitKind::Int(sym, base),
            span: token.span,
          }),
          span: token.span,
        }),
        _ => Err(error::syntax::expected_int(token.span, token.to_smolstr())),
      })
      .unwrap()
  }

  /// Parses a floating-point literal.
  fn parse_expr_lit_float(parser: &mut Parser) -> Result<Expr> {
    parser
      .maybe_token_current
      .map(|token| match token.kind {
        TokenKind::Float(sym) => Ok(Expr {
          kind: ExprKind::Lit(Lit {
            kind: LitKind::Float(sym),
            span: token.span,
          }),
          span: token.span,
        }),
        _ => Err(error::syntax::expected_float(
          token.span,
          token.to_smolstr(),
        )),
      })
      .unwrap()
  }

  /// Parses an identifier expression.
  fn parse_expr_ident(parser: &mut Parser) -> Result<Expr> {
    parser
      .maybe_token_current
      .map(|token| match token.kind {
        TokenKind::Ident(sym) => Ok(Expr {
          kind: ExprKind::Lit(Lit {
            kind: LitKind::Ident(sym),
            span: token.span,
          }),
          span: token.span,
        }),
        _ => Err(error::syntax::expected_ident(
          token.span,
          token.to_smolstr(),
        )),
      })
      .unwrap()
  }

  /// Parses a boolean expression.
  fn parse_expr_lit_bool(parser: &mut Parser) -> Result<Expr> {
    parser
      .maybe_token_current
      .map(|token| {
        Ok(Expr {
          kind: ExprKind::Lit(Lit {
            kind: match token.kind {
              TokenKind::Kw(Kw::False) => LitKind::Bool(false),
              TokenKind::Kw(Kw::True) => LitKind::Bool(true),
              _ => {
                return Err(error::syntax::expected_bool(
                  token.span,
                  token.to_smolstr(),
                ));
              }
            },
            span: token.span,
          }),
          span: token.span,
        })
      })
      .unwrap()
  }

  /// Parses an unary operator expression.
  fn parse_expr_unop(parser: &mut Parser) -> Result<Expr> {
    parser
      .maybe_token_current
      .map(|token| {
        let precedence = Precedence::from(Some(token));

        let unop = match token.kind {
          TokenKind::Punctuation(Punctuation::Minus) => UnOp {
            kind: UnOpKind::Neg,
            span: token.span,
          },
          TokenKind::Punctuation(Punctuation::Exclamation) => UnOp {
            kind: UnOpKind::Not,
            span: token.span,
          },
          _ => panic!(), // expected unop syntax error.
        };

        parser.next();

        let expr = parser.parse_expr(precedence)?;

        Ok(Expr {
          kind: ExprKind::UnOp(unop, Box::new(expr)),
          span: token.span,
        })
      })
      .unwrap()
  }

  /// Parses a group expression.
  fn parse_expr_group(parser: &mut Parser) -> Result<Expr> {
    parser.next();

    let expr = parser.parse_expr(Precedence::Low)?;

    parser.expect_peek(TokenKind::Group(Group::ParenClose))?;

    Ok(expr)
  }

  /// Parses an array expression.
  fn parse_expr_array(parser: &mut Parser) -> Result<Expr> {
    let lo = parser.current_span();
    let elmts = parser.parse_exprs()?;
    let hi = parser.current_span();

    Ok(Expr {
      kind: ExprKind::Array(elmts),
      span: Span::merge(lo, hi),
    })
  }

  /// Parses expressions.
  fn parse_exprs(&mut self) -> Result<Vec<Expr>> {
    let mut exprs = Vec::with_capacity(0usize); // no allocation.

    while !self.ensure_peek(TokenKind::Group(Group::BracketClose)) {
      if self
        .expect_peek(TokenKind::Punctuation(Punctuation::Comma))
        .is_ok()
      {
        continue;
      }

      self.next();
      exprs.push(self.parse_expr(Precedence::Low)?);
    }

    self.expect_peek(TokenKind::Group(Group::BracketClose))?;

    Ok(exprs)
  }

  /// Gets the infix function.
  fn parse_infix_fn(&self) -> Result<InfixFn> {
    let token = self.maybe_token_next.unwrap();

    match token.kind {
      k if k.is_binop() => Ok(Box::new(Self::parse_expr_infix)),
      k if k.is_index() => Ok(Box::new(Self::parse_expr_array_access)),
      _ => Err(error::syntax::invalid_infix(token.span, token.to_smolstr())),
    }
  }

  /// Parses an infix expression.
  fn parse_expr_infix(parser: &mut Parser, lhs: Expr) -> Result<Expr> {
    let lo = lhs.span;

    let (precedence, maybe_binop) = parser
      .maybe_token_current
      .map(|token| {
        let binop = BinOp::from(token);

        match token.kind {
          k if k.is_assignment() => (Precedence::Assignement, Some(binop)),
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

  /// Parses an array access expression.
  fn parse_expr_array_access(parser: &mut Parser, lhs: Expr) -> Result<Expr> {
    let lo = lhs.span;

    parser.next();

    let access = parser.parse_expr(Precedence::Index)?;

    parser.expect_peek(TokenKind::Group(Group::BracketClose))?;

    let hi = parser.current_span();

    Ok(Expr {
      kind: ExprKind::ArrayAccess(Box::new(lhs), Box::new(access)),
      span: Span::merge(lo, hi),
    })
  }
}

impl<'tokens> Iterator for Parser<'tokens> {
  type Item = &'tokens Token;

  /// Moves to the next token.
  fn next(&mut self) -> Option<Self::Item> {
    std::mem::swap(&mut self.maybe_token_current, &mut self.maybe_token_next);

    self.peek().and_then(|token| {
      self.index += 1;
      self.span_current = token.span;
      self.maybe_token_next = Some(token);

      Some(token)
    })
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
