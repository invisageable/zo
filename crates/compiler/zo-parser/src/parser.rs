//! ...

use super::precedence::Precedence;

use zo_ast::ast::{
  Arg, Args, Ast, BinOp, BinOpKind, Expr, ExprKind, Lit, LitKind, UnOp,
};

use zo_session::session::Session;
use zo_tokenizer::token::group::Group;
use zo_tokenizer::token::kw::Kw;
use zo_tokenizer::token::op::Op;
use zo_tokenizer::token::punctuation::Punctuation;
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

  #[inline]
  fn ensure(&mut self, kind: TokenKind) -> bool {
    self
      .maybe_token_current
      .map(|token| token.is(kind))
      .unwrap()
  }

  #[inline]
  fn ensure_peek(&mut self, kind: TokenKind) -> bool {
    self.maybe_token_next.map(|token| token.is(kind)).unwrap()
  }

  #[inline]
  fn expect(&mut self, kind: TokenKind) -> Result<()> {
    self
      .maybe_token_current
      .map(|token| {
        if token.is(kind) {
          self.next();

          return Ok(());
        }

        Err(ReportError::Syntax(Syntax::UnexpectedToken(
          token.span,
          token.to_string(),
        )))
      })
      .unwrap()
  }

  #[inline]
  fn expect_peek(&mut self, kind: TokenKind) -> Result<()> {
    self
      .maybe_token_next
      .map(|token| {
        if token.is(kind) {
          self.next();

          return Ok(());
        }

        Err(ReportError::Syntax(Syntax::UnexpectedToken(
          token.span,
          token.to_string(),
        )))
      })
      .unwrap()
  }

  fn parse(&mut self) -> Result<Ast> {
    let mut ast = Ast::new();

    if self.tokens.is_empty() {
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
      TokenKind::Op(Op::Minus) | TokenKind::Op(Op::Exclamation) => {
        Some(Self::parse_expr_unop)
      }
      TokenKind::Group(Group::ParenOpen) => Some(Self::parse_expr_group),
      TokenKind::Group(Group::BracketOpen) => Some(Self::parse_expr_array),
      TokenKind::Group(Group::BraceOpen) => Some(Self::parse_expr_record),
      TokenKind::Kw(Kw::Fn) => Some(Self::parse_expr_fn),
      TokenKind::Kw(Kw::If) => Some(Self::parse_expr_if_else),
      TokenKind::Kw(Kw::When) => Some(Self::parse_expr_when),
      TokenKind::Kw(Kw::Loop) => Some(Self::parse_expr_loop),
      TokenKind::Kw(Kw::While) => Some(Self::parse_expr_while),
      TokenKind::Kw(Kw::Return) => Some(Self::parse_expr_return),
      TokenKind::Kw(Kw::Break) => Some(Self::parse_expr_break),
      TokenKind::Kw(Kw::Continue) => Some(Self::parse_expr_continue),
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
    parser
      .maybe_token_current
      .map(|token| match token.kind {
        TokenKind::Ident(symbol) => Ok(Expr {
          kind: ExprKind::Lit(Lit {
            kind: LitKind::Ident(symbol),
            span: token.span,
          }),
          span: token.span,
        }),
        _ => Err(ReportError::Syntax(Syntax::ExpectedLitIdent(
          token.span,
          token.kind.to_string(),
        ))),
      })
      .unwrap()
  }

  fn parse_expr_lit_bool(parser: &mut Parser) -> Result<Expr> {
    parser
      .maybe_token_current
      .map(|token| {
        let kind = match token.kind {
          TokenKind::Kw(Kw::False) => LitKind::Bool(false),
          TokenKind::Kw(Kw::True) => LitKind::Bool(true),
          _ => {
            return Err(ReportError::Syntax(Syntax::ExpectedLitBool(
              token.span,
              token.kind.to_string(),
            )))
          }
        };

        Ok(Expr {
          kind: ExprKind::Lit(Lit {
            kind,
            span: token.span,
          }),
          span: token.span,
        })
      })
      .unwrap()
  }

  fn parse_expr_lit_char(parser: &mut Parser) -> Result<Expr> {
    parser
      .maybe_token_current
      .map(|token| match token.kind {
        TokenKind::Char(symbol) => Ok(Expr {
          kind: ExprKind::Lit(Lit {
            kind: LitKind::Char(symbol),
            span: token.span,
          }),
          span: token.span,
        }),
        _ => Err(ReportError::Syntax(Syntax::ExpectedLitChar(
          token.span,
          token.kind.to_string(),
        ))),
      })
      .unwrap()
  }

  fn parse_expr_lit_str(parser: &mut Parser) -> Result<Expr> {
    parser
      .maybe_token_current
      .map(|token| match token.kind {
        TokenKind::Str(symbol) => Ok(Expr {
          kind: ExprKind::Lit(Lit {
            kind: LitKind::Str(symbol),
            span: token.span,
          }),
          span: token.span,
        }),
        _ => Err(ReportError::Syntax(Syntax::ExpectedLitStr(
          token.span,
          token.kind.to_string(),
        ))),
      })
      .unwrap()
  }

  fn parse_expr_unop(parser: &mut Parser) -> Result<Expr> {
    parser
      .maybe_token_current
      .map(|token| {
        let precedence = Precedence::from(Some(token));
        let unop = UnOp::from(token);

        parser.next();

        let expr = parser.parse_expr(precedence)?;

        match token.kind {
          kind if kind.is_unop() => Ok(Expr {
            kind: ExprKind::UnOp(unop, Box::new(expr)),
            span: token.span,
          }),
          _ => panic!("expected unop."),
        }
      })
      .unwrap()
  }

  fn parse_expr_fn(_parser: &mut Parser) -> Result<Expr> {
    todo!()
  }

  fn parse_expr_group(_parser: &mut Parser) -> Result<Expr> {
    todo!()
  }

  fn parse_expr_array(_parser: &mut Parser) -> Result<Expr> {
    todo!()
  }

  fn parse_expr_record(_parser: &mut Parser) -> Result<Expr> {
    todo!()
  }

  fn parse_expr_if_else(_parser: &mut Parser) -> Result<Expr> {
    todo!()
  }

  fn parse_expr_when(_parser: &mut Parser) -> Result<Expr> {
    todo!()
  }

  fn parse_expr_loop(_parser: &mut Parser) -> Result<Expr> {
    todo!()
  }

  fn parse_expr_while(_parser: &mut Parser) -> Result<Expr> {
    todo!()
  }

  fn parse_expr_return(_parser: &mut Parser) -> Result<Expr> {
    todo!()
  }

  fn parse_expr_break(_parser: &mut Parser) -> Result<Expr> {
    todo!()
  }

  fn parse_expr_continue(_parser: &mut Parser) -> Result<Expr> {
    todo!()
  }

  fn parse_infix_fn(&self) -> Option<ParseInfixFn> {
    let token = self.maybe_token_next.unwrap(); // should be unwrap properly.

    match token.kind {
      kind if kind.is_binop() => Some(Self::parse_expr_binop),
      kind if kind.is_assignement() => Some(Self::parse_expr_assignment),
      kind if kind.is_calling() => Some(Self::parse_expr_call),
      kind if kind.is_index() => Some(Self::parse_expr_array_access),
      _ => None,
    }
  }

  fn parse_expr_binop(parser: &mut Parser, lhs: Expr) -> Result<Expr> {
    let lo = lhs.span;

    let (precedence, maybe_binop) = parser
      .maybe_token_current
      .map(|token| {
        let binop = BinOp::from(token);

        match token.kind {
          kind if kind.is_assignement() => {
            (Precedence::Assignement, Some(binop))
          }
          kind if kind.is_conditional() => {
            (Precedence::Conditional, Some(binop))
          }
          kind if kind.is_comparison() => (Precedence::Comparison, Some(binop)),
          kind if kind.is_sum() => (Precedence::Sum, Some(binop)),
          kind if kind.is_exponent() => (Precedence::Exponent, Some(binop)),
          kind if kind.is_calling() => (Precedence::Calling, None),
          kind if kind.is_index() => (Precedence::Index, None),
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

  fn parse_expr_assignment(parser: &mut Parser, lhs: Expr) -> Result<Expr> {
    if parser.ensure(TokenKind::Op(Op::Equal)) {
      parser.next();

      let lo = lhs.span;
      let rhs = parser.parse_expr(Precedence::Assignement)?;
      let hi = parser.current_span();

      return Ok(Expr {
        kind: ExprKind::Assign(Box::new(lhs), Box::new(rhs)),
        span: Span::merge(lo, hi),
      });
    }

    Self::parse_expr_assignop(parser, lhs)
  }

  fn parse_expr_assignop(parser: &mut Parser, lhs: Expr) -> Result<Expr> {
    let maybe_binop = parser
      .maybe_token_current
      .and_then(|token| match token.kind {
        TokenKind::Op(Op::PlusEqual) => Some((BinOpKind::Add, token.span)),
        TokenKind::Op(Op::MinusEqual) => Some((BinOpKind::Sub, token.span)),
        TokenKind::Op(Op::AsteriskEqual) => Some((BinOpKind::Mul, token.span)),
        TokenKind::Op(Op::SlashEqual) => Some((BinOpKind::Div, token.span)),
        TokenKind::Op(Op::PercentEqual) => Some((BinOpKind::Rem, token.span)),
        TokenKind::Op(Op::CircumflexEqual) => {
          Some((BinOpKind::BitXor, token.span))
        }
        TokenKind::Op(Op::AmspersandEqual) => {
          Some((BinOpKind::BitAnd, token.span))
        }
        TokenKind::Op(Op::PipeEqual) => Some((BinOpKind::BitOr, token.span)),
        TokenKind::Op(Op::LessThanLessThanEqual) => {
          Some((BinOpKind::Shl, token.span))
        }
        TokenKind::Op(Op::GreaterThanGreaterThanEqual) => {
          Some((BinOpKind::Shr, token.span))
        }
        _ => None,
      })
      .map(|(kind, span)| BinOp { kind, span });

    let Some(binop) = maybe_binop else {
      // todo (ivs) — should report a proper error message.
      panic!("expected assignop.")
    };

    parser.next();

    let rhs = parser.parse_expr(Precedence::Low)?;
    let hi = parser.current_span();
    let span = Span::merge(lhs.span, hi);

    Ok(Expr {
      kind: ExprKind::AssignOp(binop, Box::new(lhs), Box::new(rhs)),
      span,
    })
  }

  fn parse_expr_call(parser: &mut Parser, lhs: Expr) -> Result<Expr> {
    let args = parser.parse_args()?;
    let hi = parser.current_span();
    let span = Span::merge(lhs.span, hi);

    Ok(Expr {
      kind: ExprKind::Call(Box::new(lhs), args),
      span,
    })
  }

  fn parse_args(&mut self) -> Result<Args> {
    let mut args = Args::new();

    while !self.ensure_peek(TokenKind::Group(Group::ParenClose)) {
      if self
        .expect_peek(TokenKind::Punctuation(Punctuation::Comma))
        .is_ok()
      {
        continue;
      }

      self.next();
      args.add_arg(self.parse_arg()?);
    }

    self.expect_peek(TokenKind::Group(Group::ParenClose))?;

    Ok(args)
  }

  fn parse_arg(&mut self) -> Result<Arg> {
    let lo = self.current_span();
    let expr = self.parse_expr(Precedence::Low)?;
    let hi = self.current_span();
    let span = Span::merge(lo, hi);

    Ok(Arg { expr, span })
  }

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
