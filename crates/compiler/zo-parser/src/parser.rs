use super::precedence::Precedence;

use zo_ast::ast::{
  Ast, BinOp, BinOpKind, Block, Expr, ExprKind, Input, Item, ItemKind, Lit,
  LitKind, Mutability, OutputTy, Pattern, PatternKind, Prototype, Pub, Stmt,
  StmtKind, UnOp, UnOpKind, Var, VarKind,
};

use zo_interner::interner::symbol::Symbol;
use zo_interner::interner::Interner;
use zo_reporter::reporter::Reporter;
use zo_reporter::{error, Result};
use zo_session::session::Session;
use zo_tokenizer::token::group::Group;
use zo_tokenizer::token::kw::Kw;
use zo_tokenizer::token::punctuation::Punctuation;
use zo_tokenizer::token::{Token, TokenKind};
use zo_ty::ty::{FloatTy, IntTy, LitFloatTy, LitIntTy, SintTy, Ty, UintTy};

use swisskit::span::{AsSpan, Span};

/// A type that defines a prefix function.
type PrefixFn = Box<dyn FnOnce(&mut Parser) -> Result<Expr>>;
/// A type that defines an infix function.
type InfixFn = Box<dyn FnOnce(&mut Parser, Expr) -> Result<Expr>>;

/// The representation of a parser.
struct Parser<'tokens> {
  /// The index of the token slice.
  index: usize,
  /// An optional current token.
  maybe_token_current: Option<&'tokens Token>,
  /// An optional next token.
  maybe_token_next: Option<&'tokens Token>,
  /// The current span.
  span_current: Span,
  /// The closure counter.
  counter_fn: usize,
  /// A group of tokens — see also [`Token`] for more information.
  tokens: &'tokens [Token],
  /// An interner — see also [`Interner`] for more information.
  interner: &'tokens mut Interner,
  /// A reporter — see also [`Reporter`] for more information.
  reporter: &'tokens Reporter,
}

impl<'tokens> Parser<'tokens> {
  /// Creates a new parser instance from tokens, interner and reporter.
  #[inline]
  fn new(
    interner: &'tokens mut Interner,
    reporter: &'tokens Reporter,
    tokens: &'tokens [Token],
  ) -> Self {
    Self {
      index: 0usize,
      maybe_token_current: None,
      maybe_token_next: None,
      span_current: Span::ZERO,
      counter_fn: 0usize,
      tokens,
      interner,
      reporter,
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

        Err(error::syntax::unexpected_token(token.span, *token))
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

        Err(error::syntax::unexpected_token(token.span, *token))
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

  /// Parses an item.
  fn parse_item(&mut self) -> Result<Item> {
    self
      .maybe_token_current
      .map(|token| match token.kind {
        TokenKind::Kw(Kw::Val) => self.parse_item_val(),
        _ => Err(error::syntax::unexpected_token(token.span, *token)),
      })
      .unwrap()
  }

  /// Parses an global variable item.
  fn parse_item_val(&mut self) -> Result<Item> {
    self.parse_global_var()
  }

  /// Parses an gloabl variable item.
  fn parse_global_var(&mut self) -> Result<Item> {
    let lo = self.current_span();

    let kind = self
      .maybe_token_current
      .map(|token| {
        if let TokenKind::Kw(Kw::Val) = token.kind {
          return Ok(VarKind::Val);
        }

        Err(error::syntax::expected_global_var(token.span, *token))
      })
      .unwrap()?;

    self.next();

    let pattern = self.parse_pattern()?;
    let ty = self.parse_ty()?;

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
          ty,
          value: Box::new(value),
          span,
        }),
        span,
      }),
      _ => Err(error::syntax::expected_local_var(span, kind)),
    }
  }

  /// Parses a statement.
  fn parse_stmt(&mut self) -> Result<Stmt> {
    let stmt = self
      .maybe_token_current
      .map(|token| match token.kind {
        k if k.is_var_local() => self.parse_stmt_var(),
        k if k.is_item() => self.parse_stmt_item(),
        _ => self.parse_stmt_expr(),
      })
      .unwrap()?;

    if self.ensure_peek(TokenKind::Punctuation(Punctuation::Semicolon)) {
      self.next();
    }

    Ok(stmt)
  }

  /// Parses a variable statement.
  fn parse_stmt_var(&mut self) -> Result<Stmt> {
    self.parse_local_var()
  }

  /// Parses a local variable statement.
  fn parse_local_var(&mut self) -> Result<Stmt> {
    let lo = self.current_span();

    let kind = self
      .maybe_token_current
      .map(|token| {
        Ok(match token.kind {
          TokenKind::Kw(Kw::Imu) => VarKind::Imu,
          TokenKind::Kw(Kw::Mut) => VarKind::Mut,
          _ => {
            return Err(error::syntax::expected_local_var(token.span, *token))
          }
        })
      })
      .unwrap()?;

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
          value: Box::new(value),
          ty,
          span,
        }),
        span,
      }),
      VarKind::Mut => Ok(Stmt {
        kind: StmtKind::Var(Var {
          kind,
          mutability: Mutability::Yes,
          pubness: Pub::No,
          pattern,
          value: Box::new(value),
          ty,
          span,
        }),
        span,
      }),
      _ => Err(error::syntax::expected_local_var(span, kind)),
    }
  }

  /// Parses a type.
  fn parse_ty(&mut self) -> Result<Ty> {
    self.next();

    let ty = self
      .maybe_token_current
      .map(|token| match token.kind {
        TokenKind::Punctuation(Punctuation::Colon) => self.parse_ty_type(),
        TokenKind::Punctuation(Punctuation::ColonEqual) => {
          Ok(Ty::infer(token.span))
        }
        _ => Err(error::syntax::expected_ty(token.span, *token)),
      })
      .unwrap()?;

    if self.ensure_peek(TokenKind::Punctuation(Punctuation::Equal)) {
      self.next();
    }

    Ok(ty)
  }

  /// Parses a type.
  fn parse_ty_type(&mut self) -> Result<Ty> {
    self.next();

    self
      .maybe_token_current
      .map(|token| match &token.kind {
        TokenKind::Ident(symbol) => {
          self.parse_ty_ident_or_array(symbol, token.span)
        }
        TokenKind::Group(Group::ParenOpen) => self.parse_ty_tuple(token.span),
        TokenKind::Kw(Kw::FnUpper) => self.parse_ty_closure(token.span),
        _ => Err(error::syntax::unexpected_token(token.span, *token)),
      })
      .unwrap()
  }

  /// Parses a type or array type.
  fn parse_ty_ident_or_array(
    &mut self,
    sym: &Symbol,
    span: Span,
  ) -> Result<Ty> {
    let ident = self.interner.lookup(**sym);

    let ty = match ident {
      "int" => Ty::int(LitIntTy::Int(IntTy::Int), span),
      "s8" => Ty::int(LitIntTy::Signed(SintTy::S8), span),
      "s16" => Ty::int(LitIntTy::Signed(SintTy::S8), span),
      "s32" => Ty::int(LitIntTy::Signed(SintTy::S8), span),
      "s64" => Ty::int(LitIntTy::Signed(SintTy::S8), span),
      "s128" => Ty::int(LitIntTy::Signed(SintTy::S128), span),
      "u8" => Ty::int(LitIntTy::Unsigned(UintTy::U8), span),
      "u16" => Ty::int(LitIntTy::Unsigned(UintTy::U16), span),
      "u32" => Ty::int(LitIntTy::Unsigned(UintTy::U32), span),
      "u64" => Ty::int(LitIntTy::Unsigned(UintTy::U64), span),
      "u128" => Ty::int(LitIntTy::Unsigned(UintTy::U128), span),
      "float" => Ty::float(LitFloatTy::Suffixed(FloatTy::Float), span),
      "f32" => Ty::float(LitFloatTy::Suffixed(FloatTy::F32), span),
      "f64" => Ty::float(LitFloatTy::Suffixed(FloatTy::F64), span),
      _ => Ty::unit(span),
    };

    if self
      .expect_peek(TokenKind::Group(Group::BracketOpen))
      .is_ok()
    {
      let maybe_size = self
        .maybe_token_next
        .map(|token| match &token.kind {
          TokenKind::Int(sym, _) => {
            self.next();

            let int = self.interner.lookup_int(**sym as usize);

            Some(int as usize)
          }
          _ => None,
        })
        .unwrap();

      self.expect_peek(TokenKind::Group(Group::BracketClose))?;

      let span = Span::merge(span, self.current_span());

      return Ok(Ty::array(ty, maybe_size, span));
    }

    Ok(ty)
  }

  /// Parses tuple type.
  fn parse_ty_tuple(&mut self, span: Span) -> Result<Ty> {
    let mut tys = Vec::with_capacity(0usize);

    while self
      .expect_peek(TokenKind::Group(Group::ParenClose))
      .is_err()
    {
      if self
        .expect_peek(TokenKind::Punctuation(Punctuation::Comma))
        .is_ok()
      {
        continue;
      }

      tys.push(self.parse_ty_type()?);
    }

    self.next();

    let span = Span::merge(span, self.current_span());

    Ok(Ty::tuple(tys, span))
  }

  /// Parses closure type.
  fn parse_ty_closure(&mut self, span: Span) -> Result<Ty> {
    self.next();

    let inputs = self.parse_ty_closure_inputs()?;
    let output = self.parse_ty_closure_output()?;
    let span = Span::merge(span, self.current_span());

    Ok(Ty::closure(inputs, output, span))
  }

  /// Parses inputs closure type.
  fn parse_ty_closure_inputs(&mut self) -> Result<Vec<Ty>> {
    let mut inputs = Vec::with_capacity(0usize);

    while self
      .expect_peek(TokenKind::Group(Group::ParenClose))
      .is_err()
    {
      if self
        .expect_peek(TokenKind::Punctuation(Punctuation::Comma))
        .is_ok()
      {
        continue;
      }

      inputs.push(self.parse_ty_type()?);
    }

    Ok(inputs)
  }

  /// Parses output closure type.
  fn parse_ty_closure_output(&mut self) -> Result<Ty> {
    self.parse_ty()
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

  /// Parses a pattern.
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
        _ => Err(error::syntax::unexpected_token(token.span, *token)),
      })
      .unwrap()
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

    Ok(match token.kind {
      TokenKind::Int(..) => Box::new(Self::parse_expr_lit_int),
      TokenKind::Float(_) => Box::new(Self::parse_expr_lit_float),
      TokenKind::Ident(_) => Box::new(Self::parse_expr_lit_ident),
      TokenKind::Kw(Kw::False) | TokenKind::Kw(Kw::True) => {
        Box::new(Self::parse_expr_lit_bool)
      }
      TokenKind::Char(_) => Box::new(Self::parse_expr_lit_char),
      TokenKind::Str(_) => Box::new(Self::parse_expr_lit_str),
      TokenKind::Punctuation(Punctuation::Minus)
      | TokenKind::Punctuation(Punctuation::Exclamation) => {
        Box::new(Self::parse_expr_unop)
      }
      TokenKind::Group(Group::ParenOpen) => {
        Box::new(Self::parse_expr_group_or_tuple)
      }
      TokenKind::Group(Group::BracketOpen) => Box::new(Self::parse_expr_array),
      TokenKind::Kw(Kw::If) => Box::new(Self::parse_expr_if_else),
      TokenKind::Kw(Kw::When) => Box::new(Self::parse_expr_when),
      TokenKind::Kw(Kw::Loop) => Box::new(Self::parse_expr_loop),
      TokenKind::Kw(Kw::While) => Box::new(Self::parse_expr_while),
      TokenKind::Kw(Kw::Return) => Box::new(Self::parse_expr_return),
      TokenKind::Kw(Kw::Break) => Box::new(Self::parse_expr_break),
      TokenKind::Kw(Kw::Continue) => Box::new(Self::parse_expr_continue),
      TokenKind::Kw(Kw::FnLower) => Box::new(Self::parse_expr_fn),
      _ => return Err(error::syntax::invalid_prefix(token.span, *token)),
    })
  }

  /// Parses an integer literal.
  fn parse_expr_lit_int(parser: &mut Parser) -> Result<Expr> {
    parser
      .maybe_token_current
      .map(|token| {
        let TokenKind::Int(sym, base) = token.kind else {
          return Err(error::syntax::expected_int(token.span, *token));
        };

        Ok(Expr {
          kind: ExprKind::Lit(Lit {
            kind: LitKind::Int(sym, base),
            span: token.span,
          }),
          span: token.span,
        })
      })
      .unwrap()
  }

  /// Parses a floating-point literal.
  fn parse_expr_lit_float(parser: &mut Parser) -> Result<Expr> {
    parser
      .maybe_token_current
      .map(|token| {
        let TokenKind::Float(sym) = token.kind else {
          return Err(error::syntax::expected_float(token.span, *token));
        };

        Ok(Expr {
          kind: ExprKind::Lit(Lit {
            kind: LitKind::Float(sym),
            span: token.span,
          }),
          span: token.span,
        })
      })
      .unwrap()
  }

  /// Parses an identifier expression.
  fn parse_expr_lit_ident(parser: &mut Parser) -> Result<Expr> {
    parser
      .maybe_token_current
      .map(|token| {
        let TokenKind::Ident(sym) = token.kind else {
          return Err(error::syntax::expected_ident(token.span, *token));
        };

        Ok(Expr {
          kind: ExprKind::Lit(Lit {
            kind: LitKind::Ident(sym),
            span: token.span,
          }),
          span: token.span,
        })
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
                return Err(error::syntax::expected_bool(token.span, *token))
              }
            },
            span: token.span,
          }),
          span: token.span,
        })
      })
      .unwrap()
  }

  /// Parses a char expression.
  fn parse_expr_lit_char(parser: &mut Parser) -> Result<Expr> {
    parser
      .maybe_token_current
      .map(|token| {
        let TokenKind::Char(sym) = token.kind else {
          panic!();
        };

        Ok(Expr {
          kind: ExprKind::Lit(Lit {
            kind: LitKind::Char(sym),
            span: token.span,
          }),
          span: token.span,
        })
      })
      .unwrap()
  }

  /// Parses a str expression.
  fn parse_expr_lit_str(parser: &mut Parser) -> Result<Expr> {
    parser
      .maybe_token_current
      .map(|token| {
        let TokenKind::Str(sym) = token.kind else {
          panic!();
        };

        Ok(Expr {
          kind: ExprKind::Lit(Lit {
            kind: LitKind::Str(sym),
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
          _ => return Err(error::syntax::expected_unop(token.span, *token)),
        };

        parser.next();

        let expr = parser.parse_expr(precedence)?;
        let span = Span::merge(unop.span, expr.span);

        Ok(Expr {
          kind: ExprKind::UnOp(unop, Box::new(expr)),
          span,
        })
      })
      .unwrap()
  }

  /// Parses a group expression.
  fn parse_expr_group_or_tuple(parser: &mut Parser) -> Result<Expr> {
    parser.next();

    let lo = parser.current_span();
    let expr = parser.parse_expr(Precedence::Low)?;

    // checks if the group is a tuple.
    if let TokenKind::Punctuation(Punctuation::Comma) =
      parser.maybe_token_next.unwrap().kind
    {
      let mut tuples = Vec::with_capacity(0usize);

      while !parser.ensure(TokenKind::Group(Group::ParenClose)) {
        if parser
          .expect(TokenKind::Punctuation(Punctuation::Comma))
          .is_ok()
        {
          continue;
        }

        tuples.push(parser.parse_expr(Precedence::Low)?);
        parser.next();
      }

      let hi = parser.current_span();
      let span = Span::merge(lo, hi);

      return Ok(Expr {
        kind: ExprKind::Tuple(tuples),
        span,
      });
    }

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

  /// Parses a if else condition expression.
  fn parse_expr_if_else(parser: &mut Parser) -> Result<Expr> {
    let lo = parser.current_span();

    parser.next();

    let condition = parser.parse_expr(Precedence::Low)?;
    let consequence = parser.parse_block()?;

    parser.next();

    let alternative = if parser.expect(TokenKind::Kw(Kw::Else)).is_ok() {
      if parser.ensure(TokenKind::Kw(Kw::If)) {
        Some(Box::new(Self::parse_expr_if_else(parser)?))
      } else {
        parser.next();

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

  /// Parses a block.
  fn parse_block(&mut self) -> Result<Block> {
    let lo = self.current_span();

    self
      .maybe_token_next
      .map(|token| match token.kind {
        TokenKind::Punctuation(Punctuation::MinusGreaterThan) => {
          self.next();
          self.next();

          let mut stmts = Vec::with_capacity(1usize);
          let expr = self.parse_expr(Precedence::Low)?;
          let span = expr.span;

          stmts.push(Stmt {
            kind: StmtKind::Expr(Box::new(expr)),
            span,
          });

          let span = Span::merge(lo, span);

          Ok(Block { stmts, span })
        }
        TokenKind::Group(Group::BraceOpen) => {
          let mut stmts = Vec::with_capacity(1usize);

          self.expect_peek(TokenKind::Group(Group::BraceOpen))?;

          while !self.ensure_peek(TokenKind::Group(Group::BraceClose))
            && self.has_tokens()
          {
            self.next();
            stmts.push(self.parse_stmt()?);
          }

          self.expect_peek(TokenKind::Group(Group::BraceClose))?;
          // self.next();

          let hi = self.current_span();
          let span = Span::merge(lo, hi);

          Ok(Block { stmts, span })
        }
        _ => Err(error::syntax::unexpected_token(token.span, *token)),
      })
      .unwrap()
  }

  /// Parses a ternary condition expression.
  fn parse_expr_when(parser: &mut Parser) -> Result<Expr> {
    let lo = parser.current_span();

    parser.next();

    let condition = parser.parse_expr(Precedence::Low)?;

    parser.expect_peek(TokenKind::Punctuation(Punctuation::Question))?;
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

  /// Parses a loop expression.
  fn parse_expr_loop(parser: &mut Parser) -> Result<Expr> {
    let lo = parser.current_span();
    let body = parser.parse_block()?;
    let hi = parser.current_span();

    Ok(Expr {
      kind: ExprKind::Loop(body),
      span: Span::merge(lo, hi),
    })
  }

  /// Parses a while loop expression.
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

  /// Parses a return expression.
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

    Ok(Expr {
      kind: ExprKind::Return(Some(Box::new(expr))),
      span: Span::merge(lo, hi),
    })
  }

  /// Parses a break expression.
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

  /// Parses a continue expression.
  fn parse_expr_continue(parser: &mut Parser) -> Result<Expr> {
    let lo = parser.current_span();

    parser.next();

    if parser.ensure_peek(TokenKind::Punctuation(Punctuation::Semicolon)) {
      let hi = parser.current_span();
      let span = Span::merge(lo, hi);

      return Ok(Expr {
        kind: ExprKind::Continue,
        span,
      });
    }

    let hi = parser.current_span();

    Ok(Expr {
      kind: ExprKind::Continue,
      span: Span::merge(lo, hi),
    })
  }

  /// Parses a closure expression.
  fn parse_expr_fn(parser: &mut Parser) -> Result<Expr> {
    let lo = parser.current_span();
    let sym = parser.interner.intern(&format!("fn_{}", parser.counter_fn));
    let name = parser.interner.lookup(*sym);
    let span = Span::of(lo.hi, lo.hi + name.len());

    let pattern = Pattern {
      kind: PatternKind::Ident(Box::new(Expr {
        kind: ExprKind::Lit(Lit {
          kind: LitKind::Ident(sym),
          span,
        }),
        span,
      })),
      span,
    };

    let inputs = parser.parse_inputs()?;
    let output_ty = OutputTy::Ty(Ty::infer(Span::ZERO));
    let span = Span::merge(pattern.span, output_ty.as_span());

    let prototype = Prototype {
      pattern,
      inputs,
      output_ty,
      span,
    };

    let block = parser.parse_block()?;
    let hi = parser.current_span();
    let span = Span::merge(lo, hi);

    Ok(Expr {
      kind: ExprKind::Closure(prototype, block),
      span,
    })
  }

  /// Parses a list of inputs for function and closure.
  fn parse_inputs(&mut self) -> Result<Vec<Input>> {
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

    Ok(inputs)
  }

  /// Parses an input.
  fn parse_input(&mut self) -> Result<Input> {
    let lo = self.current_span();
    let pattern = self.parse_pattern()?;
    let hi = self.current_span();
    let span = Span::merge(lo, hi);

    Ok(Input {
      pattern,
      ty: Ty::UNIT,
      span,
    })
  }

  /// Gets an infix function.
  fn parse_infix_fn(&self) -> Result<InfixFn> {
    let token = self.maybe_token_next.unwrap();

    Ok(match token.kind {
      k if k.is_binop() => Box::new(Self::parse_expr_infix),
      k if k.is_assignment() => Box::new(Self::parse_expr_assignment),
      k if k.is_index() => Box::new(Self::parse_expr_array_access),
      k if k.is_chaining() => Box::new(Self::parse_expr_tuple_access),
      k if k.is_calling() => Box::new(Self::parse_expr_call),
      _ => return Err(error::syntax::invalid_infix(token.span, *token)),
    })
  }

  /// Parses an infix expression.
  fn parse_expr_infix(parser: &mut Parser, lhs: Expr) -> Result<Expr> {
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
    let span = Span::merge(lhs.span, hi);

    Ok(Expr {
      kind: ExprKind::BinOp(binop, Box::new(lhs), Box::new(rhs)),
      span,
    })
  }

  /// Parses an assignment expression.
  fn parse_expr_assignment(parser: &mut Parser, lhs: Expr) -> Result<Expr> {
    if parser.ensure(TokenKind::Punctuation(Punctuation::Equal)) {
      parser.next();

      let rhs = parser.parse_expr(Precedence::Assignement)?;
      let hi = parser.current_span();
      let span = Span::merge(lhs.span, hi);

      return Ok(Expr {
        kind: ExprKind::Assign(Box::new(lhs), Box::new(rhs)),
        span,
      });
    }

    Self::parse_expr_assignop(parser, lhs)
  }

  /// Parses an assignment operator expression.
  fn parse_expr_assignop(parser: &mut Parser, lhs: Expr) -> Result<Expr> {
    let binop = parser
      .maybe_token_current
      .map(|token| {
        Ok(match token.kind {
          TokenKind::Punctuation(Punctuation::PlusEqual) => {
            (BinOpKind::Add, token.span)
          }
          TokenKind::Punctuation(Punctuation::MinusEqual) => {
            (BinOpKind::Sub, token.span)
          }
          TokenKind::Punctuation(Punctuation::AsteriskEqual) => {
            (BinOpKind::Mul, token.span)
          }
          TokenKind::Punctuation(Punctuation::SlashEqual) => {
            (BinOpKind::Div, token.span)
          }
          TokenKind::Punctuation(Punctuation::PercentEqual) => {
            (BinOpKind::Rem, token.span)
          }
          TokenKind::Punctuation(Punctuation::CircumflexEqual) => {
            (BinOpKind::BitXor, token.span)
          }
          TokenKind::Punctuation(Punctuation::AmspersandEqual) => {
            (BinOpKind::BitAnd, token.span)
          }
          TokenKind::Punctuation(Punctuation::PipeEqual) => {
            (BinOpKind::BitOr, token.span)
          }
          TokenKind::Punctuation(Punctuation::LessThanLessThanEqual) => {
            (BinOpKind::Shl, token.span)
          }
          TokenKind::Punctuation(Punctuation::GreaterThanGreaterThanEqual) => {
            (BinOpKind::Shr, token.span)
          }
          _ => return Err(error::syntax::expected_binop(token.span, *token)),
        })
      })
      .unwrap()
      .map(|(kind, span)| BinOp { kind, span });

    parser.next();

    let rhs = parser.parse_expr(Precedence::Low)?;
    let hi = parser.current_span();
    let span = Span::merge(lhs.span, hi);

    Ok(Expr {
      kind: ExprKind::AssignOp(binop?, Box::new(lhs), Box::new(rhs)),
      span,
    })
  }

  /// Parses an array access expression.
  fn parse_expr_array_access(parser: &mut Parser, lhs: Expr) -> Result<Expr> {
    parser.next();

    let access = parser.parse_expr(Precedence::Index)?;

    parser.expect_peek(TokenKind::Group(Group::BracketClose))?;

    let hi = parser.current_span();
    let span = Span::merge(lhs.span, hi);

    Ok(Expr {
      kind: ExprKind::ArrayAccess(Box::new(lhs), Box::new(access)),
      span,
    })
  }

  /// Parses a tuple access expression.
  fn parse_expr_tuple_access(parser: &mut Parser, lhs: Expr) -> Result<Expr> {
    parser.next();

    let access = parser.parse_expr(Precedence::Chaining)?;
    let hi = parser.current_span();
    let span = Span::merge(lhs.span, hi);

    Ok(Expr {
      kind: ExprKind::TupleAccess(Box::new(lhs), Box::new(access)),
      span,
    })
  }

  /// Parses a call expression.
  fn parse_expr_call(parser: &mut Parser, lhs: Expr) -> Result<Expr> {
    let args = parser.parse_args()?;
    let hi = parser.current_span();
    let span = Span::merge(lhs.span, hi);

    Ok(Expr {
      kind: ExprKind::Call(Box::new(lhs), args),
      span,
    })
  }

  /// Parses args.
  fn parse_args(&mut self) -> Result<Vec<Expr>> {
    let mut args = Vec::with_capacity(0usize);

    while !self.ensure_peek(TokenKind::Group(Group::ParenClose)) {
      if self
        .expect_peek(TokenKind::Punctuation(Punctuation::Comma))
        .is_ok()
      {
        continue;
      }

      self.next();
      args.push(self.parse_expr(Precedence::Low)?);
    }

    self.expect_peek(TokenKind::Group(Group::ParenClose))?;

    Ok(args)
  }

  /// Parses an item statement.
  fn parse_stmt_item(&mut self) -> Result<Stmt> {
    let item = self.parse_item()?;
    let span = item.span;

    Ok(Stmt {
      kind: StmtKind::Item(item),
      span,
    })
  }
}

impl<'tokens> Iterator for Parser<'tokens> {
  type Item = &'tokens Token;

  /// Moves to the next token.
  fn next(&mut self) -> Option<Self::Item> {
    std::mem::swap(&mut self.maybe_token_current, &mut self.maybe_token_next);

    self.peek().inspect(|token| {
      self.index += 1;
      self.span_current = token.span;
      self.maybe_token_next = Some(token);
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
  Parser::new(&mut session.interner, &session.reporter, tokens).parse()
}
