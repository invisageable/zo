//! ...

use super::precedence::Precedence;

use zo_ast::ast::{Ast, BinOp, BinOpKind, Expr, ExprKind, Lit, LitKind};
use zo_session::session::Session;
use zo_tokenizer::token::kw::Kw;
use zo_tokenizer::token::{Token, TokenKind};

use zo_core::interner::Interner;
use zo_core::reporter::report::syntax::Syntax;
use zo_core::reporter::report::ReportError;
use zo_core::reporter::Reporter;
use zo_core::span::Span;
use zo_core::Result;

type ParsePrefixFn = fn(&mut Parser) -> Result<Expr>;
type ParseInfixFn = fn(&mut Parser, Expr) -> Result<Expr>;

struct Parser<'tokens> {
  interner: &'tokens mut Interner,
  reporter: &'tokens Reporter,
  tokens: &'tokens [Token],
  index: usize,
  maybe_token_current: Option<&'tokens Token>,
  maybe_token_next: Option<&'tokens Token>,
  span_current: Span,
}

impl<'tokens> Parser<'tokens> {
  fn new(
    interner: &'tokens mut Interner,
    reporter: &'tokens Reporter,
    tokens: &'tokens [Token],
  ) -> Self {
    Self {
      interner,
      reporter,
      tokens,
      index: 0usize,
      maybe_token_current: None,
      maybe_token_next: None,
      span_current: Span::ZERO,
    }
  }

  #[inline]
  fn has_tokens(&self) -> bool {
    self.index < self.tokens.len()
  }

  #[inline]
  fn peek(&self) -> Option<&'tokens Token> {
    self.tokens.get(self.index)
  }

  #[inline]
  fn should_precedence_has_priority(&mut self, precedence: Precedence) -> bool {
    precedence < Precedence::from(self.maybe_token_next)
  }

  #[inline]
  fn current_span(&mut self) -> Span {
    self.maybe_token_current.map(|token| token.span).unwrap()
  }

  fn parse(&mut self) -> Result<Ast> {
    let mut ast = Ast::new();

    if ast.is_empty() {
      return Ok(ast);
    }

    self.next();
    self.next();

    while self.has_tokens() {
      match self.parse_expr(Precedence::Low) {
        Ok(expr) => ast.add_expr(expr),
        Err(report_error) => self.reporter.add_report(report_error),
      }

      self.next();
    }

    self.reporter.abort_if_has_errors();

    Ok(ast)
  }

  fn parse_expr(&mut self, precedence: Precedence) -> Result<Expr> {
    self
      .parse_prefix_fn()
      .map(|parse_prefix| {
        let mut lhs = parse_prefix(self)?;

        while self.has_tokens()
          && self.should_precedence_has_priority(precedence)
        {
          if let Some(parse_infix) = self.parse_infix_fn() {
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

  fn parse_prefix_fn(&self) -> Option<ParsePrefixFn> {
    let token = self.maybe_token_current.unwrap();

    match token.kind {
      TokenKind::Int(..) => Some(Self::parse_expr_lit_int),
      TokenKind::Float(_) => Some(Self::parse_expr_lit_float),
      TokenKind::Ident(_) => Some(Self::parse_expr_lit_ident),
      TokenKind::Kw(Kw::False) | TokenKind::Kw(Kw::True) => {
        Some(Self::parse_expr_lit_bool)
      }
      TokenKind::Char(_) => Some(Self::parse_expr_lit_char),
      TokenKind::Str(_) => Some(Self::parse_expr_lit_str),
      _ => None,
    }
  }

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
        _ => Err(ReportError::Syntax(Syntax::ExpectedLitInt(
          token.span,
          token.kind.to_string(),
        ))),
      })
      .unwrap()
  }

  fn parse_expr_lit_float(parser: &mut Parser) -> Result<Expr> {
    parser
      .maybe_token_current
      .map(|token| match token.kind {
        TokenKind::Float(symbol) => Ok(Expr {
          kind: ExprKind::Lit(Lit {
            kind: LitKind::Float(symbol),
            span: token.span,
          }),
          span: token.span,
        }),
        _ => Err(ReportError::Syntax(Syntax::ExpectedLitFloat(
          token.span,
          token.kind.to_string(),
        ))),
      })
      .unwrap()
  }

  fn parse_expr_lit_ident(parser: &mut Parser) -> Result<Expr> {
    todo!()
  }

  fn parse_expr_lit_bool(parser: &mut Parser) -> Result<Expr> {
    todo!()
  }

  fn parse_expr_lit_char(parser: &mut Parser) -> Result<Expr> {
    todo!()
  }

  fn parse_expr_lit_str(parser: &mut Parser) -> Result<Expr> {
    todo!()
  }

  fn parse_infix_fn(&self) -> Option<ParseInfixFn> {
    let token = self.maybe_token_next.unwrap(); // should be unwrap properly.

    match token.kind {
      kind if kind.is_binop() => Some(Self::parse_expr_binop),
      // kind if kind.is_assignement() => Some(Self::parse_expr_assignment),
      // kind if kind.is_calling() => Some(Self::parse_expr_call),
      // kind if kind.is_index() => Some(Self::parse_expr_array_access),
      // kind if kind.is_chaining() => Some(Self::parse_expr_field_access),
      _ => None,
    }
  }

  fn parse_expr_binop(parser: &mut Parser, lhs: Expr) -> Result<Expr> {
    todo!()
  }
}

impl<'tokens> Iterator for Parser<'tokens> {
  type Item = &'tokens Token;

  fn next(&mut self) -> Option<Self::Item> {
    std::mem::swap(&mut self.maybe_token_current, &mut self.maybe_token_next);

    self.peek().and_then(|token| {
      self.index += 1;
      self.span_current = token.span;
      self.maybe_token_next = Some(token);

      self.maybe_token_current
    })
  }
}

/// ...
///
/// ## examples.
///
/// ```rs
/// ```
pub fn parse(session: &mut Session, tokens: &[Token]) -> Result<Ast> {
  Parser::new(&mut session.interner, &session.reporter, tokens).parse()
}
