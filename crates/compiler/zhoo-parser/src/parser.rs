//! this parser parses a sequence of tokens and constructs an
//! abstract syntax tree.

use super::precedence::Precedence;

use zhoo_ast::ast::{
  BinOp, Block, Expr, ExprKind, Fun, Input, Inputs, Item, ItemKind, Lit,
  LitKind, Mutability, OutputTy, Pattern, PatternKind, Program, Prototype,
  Stmt, StmtKind, UnOp, Var, VarKind,
};

use zhoo_session::session::Session;
use zhoo_tokenizer::token::group::Group;
use zhoo_tokenizer::token::kw::Kw;
use zhoo_tokenizer::token::op::Op;
use zhoo_tokenizer::token::punctuation::Punctuation;
use zhoo_tokenizer::token::{Token, TokenKind};
use zhoo_ty::ty::Ty;

use zo_core::interner::Interner;
use zo_core::reporter::report::syntax::Syntax;
use zo_core::reporter::report::ReportError;
use zo_core::reporter::Reporter;
use zo_core::span::Span;
use zo_core::Result;

type ParsePrefixFn = fn(&mut Parser) -> Result<Expr>;
type ParseInfixFn = fn(&mut Parser, Expr) -> Result<Expr>;

#[derive(Debug)]
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
  #[inline]
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

  #[allow(dead_code)]
  #[inline]
  fn ensure(&mut self, kind: TokenKind) -> bool {
    self
      .maybe_token_current
      .map(|token| token.kind == kind)
      .unwrap()
  }

  #[inline]
  fn ensure_peek(&mut self, kind: TokenKind) -> bool {
    self
      .maybe_token_next
      .map(|token| token.kind == kind)
      .unwrap()
  }

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

  fn parse(&mut self) -> Result<Program> {
    let mut program = Program::new();

    self.next();
    self.next();

    while self.has_tokens() {
      match self.parse_item() {
        Ok(item) => program.add_item(item),
        Err(report_error) => self.reporter.add_report(report_error),
      }

      self.next();
    }

    self.reporter.abort_if_has_errors();

    Ok(program)
  }

  fn parse_item(&mut self) -> Result<Item> {
    self
      .maybe_token_current
      .map(|token| match token.kind {
        TokenKind::Kw(Kw::Val) => self.parse_item_val(),
        TokenKind::Kw(Kw::Fun) => self.parse_item_fun(),
        _ => Err(ReportError::Syntax(Syntax::ExpectedItem(
          token.span,
          token.kind.to_string(),
        ))),
      })
      .unwrap()
  }

  fn parse_item_val(&mut self) -> Result<Item> {
    self.parse_global_var()
  }

  fn parse_global_var(&mut self) -> Result<Item> {
    let lo = self.current_span();
    let kind = VarKind::from(self.maybe_token_current);

    self.next();

    let pattern = self.parse_pattern()?;
    let ty = self.parse_ty()?;

    self.expect_peek(TokenKind::Op(Op::Equal))?;
    self.next();

    let value = self.parse_expr(Precedence::Low)?;

    self.next();

    let hi = self.current_span();
    let span = Span::merge(lo, hi);

    match kind {
      VarKind::Val => Ok(Item {
        kind: ItemKind::Var(Var {
          kind,
          mutability: Mutability::No,
          pubness: zhoo_ast::ast::Pub::No,
          pattern,
          maybe_ty: Some(ty),
          value: Box::new(value),
          span,
        }),
        span,
      }),
      _ => panic!("expected global var."),
    }
  }

  fn parse_item_fun(&mut self) -> Result<Item> {
    self.next();

    let lo = self.current_span();
    let prototype = self.parse_prototype()?;
    let body = self.parse_block()?;
    let hi = self.current_span();
    let span = Span::merge(lo, hi);

    Ok(Item {
      kind: ItemKind::Fun(Fun {
        prototype,
        body,
        span,
      }),
      span,
    })
  }

  fn parse_prototype(&mut self) -> Result<Prototype> {
    let lo = self.current_span();
    let pattern = self.parse_pattern()?;
    let inputs = self.parse_inputs()?;
    let output = self.parse_output()?;
    let hi = self.current_span();
    let span = Span::merge(lo, hi);

    Ok(Prototype {
      pattern,
      inputs,
      output,
      span,
    })
  }

  fn parse_pattern(&mut self) -> Result<Pattern> {
    self
      .maybe_token_current
      .map(|token| match token.kind {
        TokenKind::Ident(_) => {
          let expr = Self::parse_expr_lit_ident(self)?;

          Ok(Pattern {
            kind: PatternKind::Ident(Box::new(expr)),
            span: token.span,
          })
        }
        _ => Err(ReportError::Syntax(Syntax::UnexpectedToken(
          token.span,
          token.kind.to_string(),
        ))),
      })
      .unwrap()
  }

  fn parse_inputs(&mut self) -> Result<Inputs> {
    // no allocation.
    let mut inputs = Vec::with_capacity(0usize);

    self.expect_peek(TokenKind::Group(Group::ParenOpen))?;

    while !self.ensure_peek(TokenKind::Group(Group::ParenClose)) {
      if self.ensure_peek(TokenKind::Punctuation(Punctuation::Comma)) {
        self.next();

        continue;
      }

      self.next();
      inputs.push(self.parse_input()?);
    }

    self.expect_peek(TokenKind::Group(Group::ParenClose))?;

    Ok(Inputs(inputs))
  }

  fn parse_input(&mut self) -> Result<Input> {
    let lo = self.current_span();
    let pattern = self.parse_pattern()?;
    let ty = self.parse_ty()?;
    let hi = self.current_span();
    let span = Span::merge(lo, hi);

    Ok(Input { pattern, ty, span })
  }

  fn parse_ty(&mut self) -> Result<Ty> {
    self.next();

    if self
      .expect(TokenKind::Punctuation(Punctuation::Colon))
      .is_ok()
    {
      self
        .maybe_token_current
        .map(|token| match token.kind {
          TokenKind::Ident(ident) => {
            let ident = self.interner.lookup_ident(ident);

            Ok(Ty::from((ident, token.span)))
          }
          _ => Err(ReportError::Syntax(Syntax::UnexpectedToken(
            token.span,
            token.kind.to_string(),
          ))),
        })
        .unwrap()
    } else {
      Ok(Ty::UNIT)
    }
  }

  fn parse_output(&mut self) -> Result<OutputTy> {
    Ok(OutputTy::Default(Span::ZERO))
  }

  fn parse_block(&mut self) -> Result<Block> {
    let mut stmts = Vec::with_capacity(0usize);
    let lo = self.current_span();

    self.expect_peek(TokenKind::Group(Group::BraceOpen))?;

    while !self.ensure_peek(TokenKind::Group(Group::BraceClose))
      && self.has_tokens()
    {
      if self.ensure_peek(TokenKind::Punctuation(Punctuation::Semicolon)) {
        self.next();

        continue;
      }

      self.next();
      stmts.push(self.parse_stmt()?);
    }

    self.expect_peek(TokenKind::Group(Group::BraceClose))?;

    let hi = self.current_span();
    let span = Span::merge(lo, hi);

    Ok(Block { stmts, span })
  }

  fn parse_stmt(&mut self) -> Result<Stmt> {
    self
      .maybe_token_current
      .map(|token| match token.kind {
        kind if kind.is_var_local() => self.parse_stmt_var(),
        _ => self.parse_stmt_expr(),
      })
      .unwrap()
  }

  fn parse_stmt_var(&mut self) -> Result<Stmt> {
    self.parse_local_var()
  }

  fn parse_local_var(&mut self) -> Result<Stmt> {
    let lo = self.current_span();
    let kind = VarKind::from(self.maybe_token_current);

    self.next();

    let pattern = self.parse_pattern()?;
    let ty = self.parse_ty()?;

    self.next();

    let value = self.parse_expr(Precedence::Low)?;

    self.next();

    let hi = self.current_span();
    let span = Span::merge(lo, hi);

    match kind {
      VarKind::Imu => Ok(Stmt {
        kind: StmtKind::Var(Var {
          kind,
          mutability: Mutability::No,
          pubness: zhoo_ast::ast::Pub::No,
          pattern,
          maybe_ty: Some(ty),
          value: Box::new(value),
          span,
        }),
        span,
      }),
      VarKind::Mut => Ok(Stmt {
        kind: StmtKind::Var(Var {
          kind,
          mutability: Mutability::Yes(Span::ZERO),
          pubness: zhoo_ast::ast::Pub::No,
          pattern,
          maybe_ty: Some(ty),
          value: Box::new(value),
          span,
        }),
        span,
      }),
      _ => panic!("expected local var."),
    }
  }

  fn parse_stmt_expr(&mut self) -> Result<Stmt> {
    let lo = self.current_span();
    let expr = self.parse_expr(Precedence::Low)?;

    self
      .maybe_token_current
      .map(|token| {
        if let TokenKind::Punctuation(Punctuation::Semicolon) = token.kind {
          self.next();
        }
      })
      .unwrap();

    let hi = self.current_span();
    let span = Span::merge(lo, hi);

    Ok(Stmt {
      kind: StmtKind::Expr(Box::new(expr)),
      span,
    })
  }

  #[allow(dead_code)]
  fn parse_expr_stmt(parser: &mut Parser) -> Result<Expr> {
    let expr = parser.parse_expr(Precedence::Low)?;

    parser
      .maybe_token_next
      .map(|token| {
        if let TokenKind::Punctuation(Punctuation::Semicolon) = token.kind {
          parser.next();
        }
      })
      .unwrap();

    Ok(expr)
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
      TokenKind::Int(_) => Some(Self::parse_expr_lit_int),
      TokenKind::Float(_) => Some(Self::parse_expr_lit_float),
      TokenKind::Ident(_) => Some(Self::parse_expr_lit_ident),
      TokenKind::Char(_) => Some(Self::parse_expr_lit_char),
      TokenKind::Str(_) => Some(Self::parse_expr_lit_str),
      kind if kind.is_unop() => Some(Self::parse_expr_unop),
      TokenKind::Kw(Kw::Fn) => Some(Self::parse_expr_fn),
      // kind if kind.is_stmt() => Some(Self::parse_expr_stmt),
      _ => None,
    }
  }

  fn parse_expr_lit_int(parser: &mut Parser) -> Result<Expr> {
    parser
      .maybe_token_current
      .map(|token| match token.kind {
        TokenKind::Int(int) => Ok(Expr {
          kind: ExprKind::Lit(Lit {
            kind: LitKind::Int(int),
            span: token.span,
          }),
          span: token.span,
        }),
        _ => panic!("expected int."),
      })
      .unwrap()
  }

  fn parse_expr_lit_float(parser: &mut Parser) -> Result<Expr> {
    parser
      .maybe_token_current
      .map(|token| match token.kind {
        TokenKind::Float(float) => Ok(Expr {
          kind: ExprKind::Lit(Lit {
            kind: LitKind::Float(float),
            span: token.span,
          }),
          span: token.span,
        }),
        _ => panic!("expected float."),
      })
      .unwrap()
  }

  fn parse_expr_lit_ident(parser: &mut Parser) -> Result<Expr> {
    parser
      .maybe_token_current
      .map(|token| match token.kind {
        TokenKind::Ident(ident) => {
          // because `true` and `false` are not keywords, we need to detect
          // first if we have a match. in a future it should be noice to used
          // implement a trait to &str.
          let word = parser.interner.lookup_ident(ident);

          if word == "true" || word == "false" {
            // return Self::parse_expr_lit_bool(parser, word);
          }

          Ok(Expr {
            kind: ExprKind::Lit(Lit {
              kind: LitKind::Ident(ident),
              span: token.span,
            }),
            span: token.span,
          })
        }
        _ => panic!("expected ident."),
      })
      .unwrap()
  }

  #[allow(dead_code)]
  fn parse_expr_lit_bool(parser: &mut Parser, word: &str) -> Result<Expr> {
    parser
      .maybe_token_current
      .map(|token| {
        let kind = {
          if word == "true" {
            LitKind::Bool(true)
          } else {
            LitKind::Bool(false)
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
        TokenKind::Char(float) => Ok(Expr {
          kind: ExprKind::Lit(Lit {
            kind: LitKind::Char(float),
            span: token.span,
          }),
          span: token.span,
        }),
        _ => panic!("expected char."),
      })
      .unwrap()
  }

  fn parse_expr_lit_str(parser: &mut Parser) -> Result<Expr> {
    parser
      .maybe_token_current
      .map(|token| match token.kind {
        TokenKind::Str(string) => Ok(Expr {
          kind: ExprKind::Lit(Lit {
            kind: LitKind::Str(string),
            span: token.span,
          }),
          span: token.span,
        }),
        _ => panic!("expected char."),
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

  // todo(ivs) — needs improvements.
  fn parse_expr_fn(parser: &mut Parser) -> Result<Expr> {
    parser
      .maybe_token_current
      .map(|token| {
        let lo = parser.current_span();
        let symbol = parser.interner.intern(&format!("anon_{}", 0usize)); // todo(ivs) — should be dynamic..

        let pattern = Pattern {
          kind: PatternKind::Ident(Box::new(Expr {
            kind: ExprKind::Lit(Lit {
              kind: LitKind::Ident(symbol),
              span: token.span,
            }),
            span: token.span,
          })),
          span: token.span,
        };

        let inputs = parser.parse_inputs()?;

        let prototype = Prototype {
          pattern: pattern.clone(),
          inputs: inputs.clone(),
          output: OutputTy::Default(Span::ZERO),
          // span: Span::merge(pattern.span, inputs.0.last().unwrap().span),
          span: Span::merge(pattern.span, Span::ZERO),
        };

        parser
          .expect_peek(TokenKind::Punctuation(Punctuation::MinusGreaterThan))?;

        parser.next();

        let stmt = parser.parse_stmt()?;
        let hi = parser.current_span();
        let span = Span::merge(lo, hi);

        Ok(Expr {
          kind: ExprKind::Fn(
            prototype,
            Block {
              stmts: vec![stmt.clone()],
              span: stmt.span,
            },
          ),
          span,
        })
      })
      .unwrap()
  }

  #[allow(dead_code)]
  fn parse_infix_fn(&self) -> Option<ParseInfixFn> {
    let token = self.maybe_token_next.unwrap();

    match token.kind {
      kind if kind.is_binop() => Some(Self::parse_expr_binop),
      _ => None,
    }
  }

  fn parse_expr_binop(parser: &mut Parser, lhs: Expr) -> Result<Expr> {
    let lo = parser.current_span();

    let (precedence, maybe_binop) = parser
      .maybe_token_current
      .map(|token| {
        let binop = BinOp::from(token);

        match token.kind {
          kind if kind.is_sum() => (Precedence::Sum, Some(binop)),
          kind if kind.is_exponent() => (Precedence::Exponent, Some(binop)),
          _ => (Precedence::Low, None),
        }
      })
      .unwrap();

    parser.next();

    let rhs = parser.parse_expr(precedence)?;
    let hi = parser.current_span();
    let span = Span::merge(lo, hi);
    let binop = maybe_binop.unwrap();

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

    self.peek().and_then(|token| {
      self.index += 1;
      self.span_current = token.span;
      self.maybe_token_next = Some(token);

      self.maybe_token_current
    })
  }
}

/// Public function that parses a sequence of tokens.
///
/// ## arguments.
///
/// * `session` — a reference to the session.
/// * `tokens`  — a sequence of tokens to be parsed.
///
/// ## returns.
///
/// a [`Result`] containing an ast ([`Program`]) or an error.
///
/// ## examples.
///
/// ```
/// ```
pub fn parse(session: &mut Session, tokens: &[Token]) -> Result<Program> {
  Parser::new(&mut session.interner, &session.reporter, tokens).parse()
}
