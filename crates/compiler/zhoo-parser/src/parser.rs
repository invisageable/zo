//! this parser parses a sequence of tokens and constructs an
//! abstract syntax tree.

use super::precedence::Precedence;

use zhoo_ast::ast::{
  Arg, Args, BinOp, BinOpKind, Block, Expr, ExprKind, Fun, Input, Inputs, Item,
  ItemKind, Lit, LitKind, Mutability, OutputTy, Pattern, PatternKind, Program,
  Prop, Prototype, Pub, Stmt, StmtKind, StructExpr, TyAlias, UnOp, Var,
  VarKind,
};

use zhoo_session::session::Session;
use zhoo_tokenizer::token::group::Group;
use zhoo_tokenizer::token::kw::Kw;
use zhoo_tokenizer::token::op::Op;
use zhoo_tokenizer::token::punctuation::Punctuation;
use zhoo_tokenizer::token::{Token, TokenKind};
use zhoo_ty::ty::{Ty, TyKind};

use zo_core::interner::Interner;
use zo_core::reporter::report::syntax::Syntax;
use zo_core::reporter::report::ReportError;
use zo_core::reporter::Reporter;
use zo_core::span::{AsSpan, Span};
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

  fn parse(&mut self) -> Result<Program> {
    let mut program = Program::new();

    self.next();
    self.next();

    while self.has_tokens() {
      match self.parse_item() {
        Ok(item) => program.add_item(item),
        // todo (ivs) — raise an error instead.
        Err(report_error) => self.reporter.add_report(report_error),
      }

      self.next();
    }

    self.reporter.abort_if_has_errors();

    Ok(program)
  }

  /// ## syntax.
  ///
  /// `<item>`
  fn parse_item(&mut self) -> Result<Item> {
    self
      .maybe_token_current
      .map(|token| match token.kind {
        TokenKind::Kw(Kw::Ext) => self.parse_item_ext(),
        TokenKind::Kw(Kw::Pack) => self.parse_item_pack(),
        TokenKind::Kw(Kw::Load) => self.parse_item_load(),
        TokenKind::Kw(Kw::Val) => self.parse_item_val(),
        TokenKind::Kw(Kw::Type) => self.parse_item_ty_alias(),
        TokenKind::Kw(Kw::Abstract) => self.parse_item_abstract(),
        TokenKind::Kw(Kw::Enum) => self.parse_item_enum(),
        TokenKind::Kw(Kw::Struct) => self.parse_item_struct(),
        TokenKind::Kw(Kw::Fun) => self.parse_item_fun(),
        _ => self
          .reporter
          .raise(ReportError::Syntax(Syntax::ExpectedItem(
            token.span,
            token.kind.to_string(),
          ))),
      })
      .unwrap()
  }

  /// ## syntax.
  ///
  /// `ext <prototype> { <body> }`.
  fn parse_item_ext(&mut self) -> Result<Item> {
    todo!()
  }

  /// ## syntax.
  ///
  /// `pub? pack <paths> ;`.
  fn parse_item_pack(&mut self) -> Result<Item> {
    todo!()
  }

  /// ## syntax.
  ///
  /// `pub? load <paths> ( <nodes> );`.
  fn parse_item_load(&mut self) -> Result<Item> {
    todo!()
  }

  /// ## syntax.
  ///
  /// `pub? val <pattern> : <ty> = <expr>;`.
  fn parse_item_val(&mut self) -> Result<Item> {
    self.parse_global_var()
  }

  /// ## notes.
  ///
  /// @see [`Parser::parse_item_val`].
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
          pubness: Pub::No,
          pattern,
          maybe_ty: if ty.is(TyKind::Unit) { None } else { Some(ty) },
          value: Box::new(value),
          span,
        }),
        span,
      }),
      _ => panic!("expected global var."),
    }
  }

  /// ## syntax.
  ///
  /// `pub? type <pattern> = <ty>;`.
  fn parse_item_ty_alias(&mut self) -> Result<Item> {
    let lo = self.current_span();

    self.next();

    let pattern = self.parse_pattern()?;

    self.expect_peek(TokenKind::Op(Op::Equal))?;

    let ty = self.parse_ty()?;
    let hi = self.current_span();
    let span = Span::merge(lo, hi);

    self.expect_peek(TokenKind::Punctuation(Punctuation::Semicolon))?;

    Ok(Item {
      kind: ItemKind::TyAlias(TyAlias {
        pubness: Pub::No,
        pattern,
        maybe_ty: Some(ty),
        span,
      }),
      span,
    })
  }

  /// ## syntax.
  ///
  /// `pub? abstract <pattern> { <behaviors> }`.
  fn parse_item_abstract(&mut self) -> Result<Item> {
    todo!()
  }

  /// ## syntax.
  ///
  /// `pub? enum <pattern> { <variants> }`.
  fn parse_item_enum(&mut self) -> Result<Item> {
    todo!()
  }

  /// ## syntax.
  ///
  /// `pub? struct <pattern> { <fields> }`.
  fn parse_item_struct(&mut self) -> Result<Item> {
    todo!()
  }

  /// ## syntax.
  ///
  /// `pub? fun <prototype> { <body> }`.
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

  /// ## syntax.
  ///
  /// `<pattern> ( <inputs> ) : <ty> { <body> }`.
  fn parse_prototype(&mut self) -> Result<Prototype> {
    let lo = self.current_span();
    let pattern = self.parse_pattern()?;
    let inputs = self.parse_inputs()?;
    let output_ty = self.parse_output_ty()?;
    let hi = self.current_span();
    let span = Span::merge(lo, hi);

    Ok(Prototype {
      pattern,
      inputs,
      output_ty,
      span,
    })
  }

  /// ## syntax.
  ///
  /// `<ident>`.
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

  /// ## syntax.
  ///
  /// `( <input*>  )`.
  fn parse_inputs(&mut self) -> Result<Inputs> {
    // no allocation.
    let mut inputs = Vec::with_capacity(0usize);

    self.expect_peek(TokenKind::Group(Group::ParenOpen))?;

    while !self.ensure_peek(TokenKind::Group(Group::ParenClose)) {
      if self
        .expect_peek(TokenKind::Punctuation(Punctuation::Comma))
        .is_ok()
      {
        continue;
      }

      self.next();
      inputs.push(self.parse_input()?);
    }

    self.expect_peek(TokenKind::Group(Group::ParenClose))?;

    Ok(Inputs(inputs))
  }

  /// ## syntax.
  ///
  /// `<pattern> : <ty>`.
  fn parse_input(&mut self) -> Result<Input> {
    let lo = self.current_span();
    let pattern = self.parse_pattern()?;
    let ty = self.parse_ty()?;
    let hi = self.current_span();
    let span = Span::merge(lo, hi);

    Ok(Input { pattern, ty, span })
  }

  /// ## syntax.
  ///
  /// `<ident>`.
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

  /// ## syntax.
  ///
  /// `: <ty>`.
  fn parse_output_ty(&mut self) -> Result<OutputTy> {
    self
      .maybe_token_next
      .map(|token| match token.kind {
        TokenKind::Punctuation(Punctuation::Colon) => {
          self.next();
          self.next();

          let ty = self.parse_ty()?;

          Ok(OutputTy::Ty(ty))
        }
        _ => Ok(OutputTy::Default(token.span)),
      })
      .unwrap()
  }

  /// ## syntax.
  ///
  /// `{ <stmt*> }`.
  fn parse_block(&mut self) -> Result<Block> {
    let mut stmts = Vec::with_capacity(0usize);
    let lo = self.current_span();

    self.expect_peek(TokenKind::Group(Group::BraceOpen))?;

    while !self.ensure_peek(TokenKind::Group(Group::BraceClose))
      && self.has_tokens()
    {
      self.next();
      stmts.push(self.parse_stmt()?);
    }

    self.expect_peek(TokenKind::Group(Group::BraceClose))?;

    let hi = self.current_span();
    let span = Span::merge(lo, hi);

    Ok(Block { stmts, span })
  }

  /// ## syntax.
  ///
  /// `<stmt*>`.
  fn parse_stmt(&mut self) -> Result<Stmt> {
    self
      .maybe_token_current
      .map(|token| match token.kind {
        kind if kind.is_var_local() => self.parse_stmt_var(),
        kind if kind.is_item() => self.parse_stmt_item(),
        _ => self.parse_stmt_expr(),
      })
      .unwrap()
  }

  /// ## notes.
  ///
  /// @see [`Parser::parse_local_var`].
  fn parse_stmt_var(&mut self) -> Result<Stmt> {
    self.parse_local_var()
  }

  /// ## syntax.
  ///
  /// `pub? imu|mut <pattern> <ty?> = <expr> ;`.
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
          pubness: Pub::No,
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
          pubness: Pub::No,
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

  /// ## syntax.
  ///
  /// `<stmt:item>`.
  ///
  /// ## notes.
  ///
  /// @see [`Parser::parse_item`]
  fn parse_stmt_item(&mut self) -> Result<Stmt> {
    let item = self.parse_item()?;
    let span = item.span;

    Ok(Stmt {
      kind: StmtKind::Item(Box::new(item)),
      span,
    })
  }

  /// ## syntax.
  ///
  /// `<stmt:expr> ;?`.
  ///
  /// ## notes.
  ///
  /// @see [`Parser::parse_expr`]
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
      .unwrap();

    let hi = self.current_span();
    let span = Span::merge(lo, hi);

    Ok(Stmt {
      kind: StmtKind::Expr(Box::new(expr)),
      span,
    })
  }

  /// ## syntax.
  ///
  /// `<expr>`.
  ///
  /// ## notes.
  ///
  /// @see [`Parser::parse_prefix_fn`] and [`Parser::parse_infix_fn`]
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

  /// ## syntax.
  ///
  /// `<prefix>`.
  fn parse_prefix_fn(&self) -> Option<ParsePrefixFn> {
    let token = self.maybe_token_current.unwrap();

    match token.kind {
      TokenKind::Int(..) => Some(Self::parse_expr_lit_int),
      TokenKind::Float(_) => Some(Self::parse_expr_lit_float),
      TokenKind::Ident(_) => Some(Self::parse_expr_lit_ident),
      TokenKind::Char(_) => Some(Self::parse_expr_lit_char),
      TokenKind::Str(_) => Some(Self::parse_expr_lit_str),
      kind if kind.is_unop() => Some(Self::parse_expr_unop),
      TokenKind::Kw(Kw::Fn) => Some(Self::parse_expr_fn),
      TokenKind::Group(Group::ParenOpen) => Some(Self::parse_expr_group),
      TokenKind::Group(Group::BraceOpen) => Some(Self::parse_expr_block),
      TokenKind::Group(Group::BracketOpen) => Some(Self::parse_expr_array),
      TokenKind::Kw(Kw::Return) => Some(Self::parse_expr_return),
      TokenKind::Kw(Kw::Continue) => Some(Self::parse_expr_continue),
      TokenKind::Kw(Kw::Break) => Some(Self::parse_expr_break),
      TokenKind::Kw(Kw::If) => Some(Self::parse_expr_if_else),
      TokenKind::Kw(Kw::When) => Some(Self::parse_expr_when),
      TokenKind::Kw(Kw::Loop) => Some(Self::parse_expr_loop),
      TokenKind::Kw(Kw::While) => Some(Self::parse_expr_while),
      TokenKind::Punctuation(Punctuation::ColonColon) => {
        Some(Self::parse_expr_struct)
      }
      _ => None,
    }
  }

  /// ## syntax.
  ///
  /// `<lit:int>`.
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

  /// ## syntax.
  ///
  /// `<lit:float>`.
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

  /// ## syntax.
  ///
  /// `<lit:ident>`.
  fn parse_expr_lit_ident(parser: &mut Parser) -> Result<Expr> {
    parser
      .maybe_token_current
      .map(|token| match token.kind {
        TokenKind::Ident(symbol) => {
          // because `true` and `false` are not keywords, we need to detect
          // first if we have a match. in a future it should be noice to used
          // implement a trait to &str.
          let ident = parser.interner.lookup_ident(symbol);

          if ident == "true" || ident == "false" {
            // cannot borrow `*parser` as mutable because it is also borrowed as
            // immutable mutable borrow occurs here.
            // return Self::parse_expr_lit_bool(parser, ident);
          }

          Ok(Expr {
            kind: ExprKind::Lit(Lit {
              kind: LitKind::Ident(symbol),
              span: token.span,
            }),
            span: token.span,
          })
        }
        _ => Err(ReportError::Syntax(Syntax::ExpectedLitIdent(
          token.span,
          token.kind.to_string(),
        ))),
      })
      .unwrap()
  }

  #[allow(dead_code)]
  /// ## syntax.
  ///
  /// `<lit:bool>`.
  fn parse_expr_lit_bool(parser: &mut Parser, ident: &str) -> Result<Expr> {
    parser
      .maybe_token_current
      .map(|token| {
        let kind = match ident {
          "true" => LitKind::Bool(true),
          "false" => LitKind::Bool(false),
          _ => panic!("expected booleaan."),
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

  /// ## syntax.
  ///
  /// `<lit:char>`.
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

  /// ## syntax.
  ///
  /// `<lit:str>`.
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

  /// ## syntax.
  ///
  /// `<unop> <expr>`.
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

  /// ## syntax.
  ///
  /// `<infix>`.
  fn parse_infix_fn(&self) -> Option<ParseInfixFn> {
    let token = self.maybe_token_next.unwrap();

    match token.kind {
      kind if kind.is_binop() => Some(Self::parse_expr_binop),
      kind if kind.is_assignement() => Some(Self::parse_expr_assignment),
      kind if kind.is_calling() => Some(Self::parse_expr_call),
      kind if kind.is_index() => Some(Self::parse_expr_array_access),
      kind if kind.is_chaining() => Some(Self::parse_expr_chaining),
      _ => None,
    }
  }

  /// ## syntax.
  ///
  /// `<expr> <binop> <expr>`.
  ///
  /// ## notes.
  ///
  /// @see [`Precedence`].
  fn parse_expr_binop(parser: &mut Parser, lhs: Expr) -> Result<Expr> {
    let lo = parser.current_span();

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
          kind if kind.is_calling() => (Precedence::Calling, Some(binop)),
          kind if kind.is_index() => (Precedence::Index, Some(binop)),
          kind if kind.is_chaining() => (Precedence::Chaining, Some(binop)),
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

  /// ## syntax.
  ///
  ///  `<expr> = <expr>` | `<expr> <assignop> <expr>`.
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

  /// ## syntax.
  ///
  /// `<expr> <assignop> <expr>`.
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

  /// ## syntax.
  ///
  /// `<expr> <args>`.
  fn parse_expr_call(parser: &mut Parser, lhs: Expr) -> Result<Expr> {
    let args = parser.parse_args()?;
    let hi = parser.current_span();
    let span = Span::merge(lhs.span, hi);

    Ok(Expr {
      kind: ExprKind::Call(Box::new(lhs), args),
      span,
    })
  }

  /// ## syntax.
  ///
  /// `<arg*>`.
  fn parse_args(&mut self) -> Result<Args> {
    // no allocation.
    let mut args = Vec::with_capacity(0usize);

    while !self.ensure_peek(TokenKind::Group(Group::ParenClose)) {
      if self
        .expect_peek(TokenKind::Punctuation(Punctuation::Comma))
        .is_ok()
      {
        continue;
      }

      self.next();
      args.push(self.parse_arg()?);
    }

    self.expect_peek(TokenKind::Group(Group::ParenClose))?;

    Ok(Args(args))
  }

  /// ## syntax.
  ///
  /// `<pattern>`.
  fn parse_arg(&mut self) -> Result<Arg> {
    let lo = self.current_span();
    let pattern = self.parse_pattern()?;
    let hi = self.current_span();
    let span = Span::merge(lo, hi);

    Ok(Arg {
      pattern,
      ty: Ty::UNIT,
      span,
    })
  }

  /// ## syntax.
  ///
  /// `<pattern> [ <expr> ]`.
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

  /// ## syntax.
  ///
  /// `<expr> . <expr>`.
  fn parse_expr_chaining(parser: &mut Parser, lhs: Expr) -> Result<Expr> {
    let lo = lhs.span;

    parser.next();

    let access = parser.parse_expr(Precedence::Chaining)?;
    let hi = parser.current_span();

    Ok(Expr {
      kind: ExprKind::Chaining(Box::new(lhs), Box::new(access)),
      span: Span::merge(lo, hi),
    })
  }

  /// ## syntax.
  ///
  /// `{ <pattern> = <expr> , }`.
  fn parse_expr_struct(parser: &mut Parser) -> Result<Expr> {
    // no allocation.
    let mut props = Vec::with_capacity(0usize);

    let lo = parser.current_span();

    parser.expect_peek(TokenKind::Group(Group::BraceOpen))?;

    while !parser.ensure_peek(TokenKind::Group(Group::BraceClose)) {
      if parser
        .expect_peek(TokenKind::Punctuation(Punctuation::Comma))
        .is_ok()
      {
        continue;
      }

      parser.next();
      props.push(parser.parse_field()?);
    }

    parser.expect_peek(TokenKind::Group(Group::BraceClose))?;

    let hi = parser.current_span();
    let span = Span::merge(lo, hi);

    // todo!("odkeodke");
    Ok(Expr {
      kind: ExprKind::StructExpr(StructExpr { props, span }),
      span: Span::merge(lo, hi),
    })
  }

  fn parse_field(&mut self) -> Result<Prop> {
    let lo = self.current_span();
    let pattern = self.parse_pattern()?;

    self.expect_peek(TokenKind::Op(Op::Equal))?;
    self.next();

    let value = self.parse_expr(Precedence::Low)?;
    let hi = self.current_span();

    Ok(Prop {
      pattern,
      value,
      span: Span::merge(lo, hi),
    })
  }
  /// ## syntax.
  ///
  /// `fn <prototype> -> <expr>`.
  /// `fn <prototype> { <body> }`.
  fn parse_expr_fn(parser: &mut Parser) -> Result<Expr> {
    // todo (ivs) — needs improvements, not working as expected.

    parser
      .maybe_token_current
      .map(|token| {
        let lo = parser.current_span();
        let symbol = parser.interner.intern(&format!("anon_{}", 0usize)); // todo (ivs) — should be dynamic..

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

        let lop = pattern.span;
        let inputs = parser.parse_inputs()?;
        let output_ty = parser.parse_output_ty()?;
        let hip = output_ty.as_span();

        let prototype = Prototype {
          pattern,
          inputs,
          output_ty,
          span: Span::merge(lop, hip),
        };

        parser
          .expect_peek(TokenKind::Punctuation(Punctuation::MinusGreaterThan))?;

        parser.next();

        let stmt = parser.parse_stmt()?;
        let stmt_span = stmt.span;
        let hi = parser.current_span();
        let span = Span::merge(lo, hi);

        Ok(Expr {
          kind: ExprKind::Fn(
            prototype,
            Block {
              stmts: vec![stmt],
              span: stmt_span,
            },
          ),
          span,
        })
      })
      .unwrap()
  }

  /// ## syntax.
  ///
  /// `{ <stmt*> }`.
  fn parse_expr_block(parser: &mut Parser) -> Result<Expr> {
    let mut stmts = Vec::with_capacity(0usize);
    let lo = parser.current_span();

    while !parser.ensure_peek(TokenKind::Group(Group::BraceClose)) {
      parser.next();
      stmts.push(parser.parse_stmt()?);
    }

    parser.expect_peek(TokenKind::Group(Group::BraceClose))?;

    let hi = parser.current_span();
    let span = Span::merge(lo, hi);

    Ok(Expr {
      kind: ExprKind::Block(Block { stmts, span }),
      span,
    })
  }

  /// ## syntax.
  ///
  /// `( <expr*> )`.
  fn parse_expr_group(parser: &mut Parser) -> Result<Expr> {
    parser.next();

    let expr = parser.parse_expr(Precedence::Low)?;

    parser.expect_peek(TokenKind::Group(Group::ParenClose))?;

    Ok(expr)
  }

  /// ## syntax.
  ///
  /// `[ <expr*> , ]`.
  fn parse_expr_array(parser: &mut Parser) -> Result<Expr> {
    let lo = parser.current_span();
    let exprs = parser.parse_exprs()?;
    let hi = parser.current_span();

    Ok(Expr {
      kind: ExprKind::Array(exprs),
      span: Span::merge(lo, hi),
    })
  }

  /// ## syntax.
  ///
  /// `<expr*>`.
  fn parse_exprs(&mut self) -> Result<Vec<Expr>> {
    let mut exprs = Vec::with_capacity(0usize);

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

  /// ## syntax.
  ///
  /// `return <expr?>`.
  fn parse_expr_return(parser: &mut Parser) -> Result<Expr> {
    let lo = parser.current_span();

    parser.next();

    if parser.ensure(TokenKind::Punctuation(Punctuation::Semicolon)) {
      let hi = parser.current_span();

      return Ok(Expr {
        kind: ExprKind::Return(None),
        span: Span::merge(lo, hi),
      });
    }

    let expr = parser.parse_expr(Precedence::Low)?;
    let hi = parser.current_span();

    while parser.ensure(TokenKind::Punctuation(Punctuation::Semicolon)) {
      parser.next();
    }

    Ok(Expr {
      kind: ExprKind::Return(Some(Box::new(expr))),
      span: Span::merge(lo, hi),
    })
  }

  /// ## syntax.
  ///
  /// `continue`.
  fn parse_expr_continue(parser: &mut Parser) -> Result<Expr> {
    let lo = parser.current_span();
    let hi = parser.current_span();

    Ok(Expr {
      kind: ExprKind::Continue,
      span: Span::merge(lo, hi),
    })
  }

  /// ## syntax.
  ///
  /// `break <expr?>`.
  fn parse_expr_break(parser: &mut Parser) -> Result<Expr> {
    let lo = parser.current_span();

    parser.next();

    if parser.ensure(TokenKind::Punctuation(Punctuation::Semicolon)) {
      let hi = parser.current_span();
      let span = Span::merge(lo, hi);

      return Ok(Expr {
        kind: ExprKind::Break(None),
        span,
      });
    }

    let value = parser.parse_expr(Precedence::Low)?;
    let hi = parser.current_span();

    Ok(Expr {
      kind: ExprKind::Break(Some(Box::new(value))),
      span: Span::merge(lo, hi),
    })
  }

  /// ## syntax.
  ///
  /// `if <expr> { <block> }`.
  /// `if <expr> { <block> } else { <block> }`.
  /// `if <expr> { <block> } else if <expr> { <block> } else { <block> }`.
  fn parse_expr_if_else(parser: &mut Parser) -> Result<Expr> {
    let lo = parser.current_span();

    parser.next();

    let condition = parser.parse_expr(Precedence::Low)?;
    let consequence = parser.parse_block()?;

    parser.next();

    let alternative = if parser.expect(TokenKind::Kw(Kw::Else)).is_ok() {
      if parser.ensure_peek(TokenKind::Kw(Kw::If)) {
        Some(Box::new(Self::parse_expr_if_else(parser)?))
      } else {
        let expr = parser.parse_expr(Precedence::Low)?;

        Some(Box::new(expr))
      }
    } else {
      None
    };

    let hi = parser.current_span();

    Ok(Expr {
      kind: ExprKind::IfElse(Box::new(condition), consequence, alternative),
      span: Span::merge(lo, hi),
    })
  }

  /// ## syntax.
  ///
  /// `when <expr> ? <expr> : <expr>`.
  fn parse_expr_when(parser: &mut Parser) -> Result<Expr> {
    let lo = parser.current_span();

    parser.next();

    let condition = parser.parse_expr(Precedence::Low)?;

    parser.expect_peek(TokenKind::Op(Op::Question))?;
    parser.next();

    let consequence = parser.parse_expr(Precedence::Low)?;

    parser.expect_peek(TokenKind::Punctuation(Punctuation::Colon))?;
    parser.next();

    let alternative = parser.parse_expr(Precedence::Low)?;
    let hi = parser.current_span();

    Ok(Expr {
      kind: ExprKind::When(
        Box::new(condition),
        Box::new(consequence),
        Box::new(alternative),
      ),
      span: Span::merge(lo, hi),
    })
  }

  /// ## syntax.
  ///
  /// `loop { <block> }`.
  fn parse_expr_loop(parser: &mut Parser) -> Result<Expr> {
    let lo = parser.current_span();
    let body = parser.parse_block()?;
    let hi = parser.current_span();

    Ok(Expr {
      kind: ExprKind::Loop(body),
      span: Span::merge(lo, hi),
    })
  }

  /// ## syntax.
  ///
  /// `while <expr> { <block> }`.
  fn parse_expr_while(parser: &mut Parser) -> Result<Expr> {
    let lo = parser.current_span();

    parser.next();

    let condition = parser.parse_expr(Precedence::Low)?;
    let body = parser.parse_block()?;
    let hi = parser.current_span();

    Ok(Expr {
      kind: ExprKind::While(Box::new(condition), body),
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

/// Public function that parses a sequence of tokens.
///
/// ## arguments.
///
/// * `session` — a reference to the session.
/// * `tokens`  — a sequence of tokens to be parsed.
///
/// ## returns.
///
/// a [`Result`] containing an ast ([`Program`]) or an error [`ReportError`].
///
/// ## examples.
///
/// ```
/// ```
pub fn parse(session: &mut Session, tokens: &[Token]) -> Result<Program> {
  Parser::new(&mut session.interner, &session.reporter, tokens).parse()
}
