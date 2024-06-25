//! ...

use super::precedence::Precedence;

use zo_ast::ast::{
  Arg, Args, Ast, BinOp, BinOpKind, Block, Expr, ExprKind, Ext, Field, Fields,
  Fun, Ident, Input, Inputs, Item, ItemKind, Lit, LitKind, Load, Mutability,
  OutputTy, Pattern, PatternKind, Prototype, Pub, Stmt, StmtKind, Struct,
  StructExpr, TyAlias, UnOp, Var, VarKind,
};

use zo_session::session::Session;
use zo_tokenizer::token::group::Group;
use zo_tokenizer::token::kw::Kw;
use zo_tokenizer::token::op::Op;
use zo_tokenizer::token::punctuation::Punctuation;
use zo_tokenizer::token::{Token, TokenKind};
use zo_ty::ty::{FloatTy, IntTy, LitFloatTy, LitIntTy, SintTy, Ty, UintTy};

use zo_core::interner::symbol::Symbol;
use zo_core::interner::Interner;
use zo_core::reporter::report::syntax::Syntax;
use zo_core::reporter::report::ReportError;
use zo_core::reporter::Reporter;
use zo_core::span::{AsSpan, Span};
use zo_core::Result;

type ParsePrefixFn = fn(&mut Parser) -> Result<Expr>;
type ParseInfixFn = fn(&mut Parser, Expr) -> Result<Expr>;

struct Parser<'tokens> {
  interner: &'tokens mut Interner,
  reporter: &'tokens mut Reporter,
  tokens: &'tokens [Token],
  index: usize,
  maybe_token_current: Option<&'tokens Token>,
  maybe_token_next: Option<&'tokens Token>,
  span_current: Span,
}

impl<'tokens> Parser<'tokens> {
  fn new(
    interner: &'tokens mut Interner,
    reporter: &'tokens mut Reporter,
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
      match self.parse_stmt() {
        Ok(stmt) => ast.add_stmt(stmt),
        Err(report_error) => self.reporter.add_report(report_error),
      }

      self.next();
    }

    self.reporter.abort_if_has_errors();

    Ok(ast)
  }

  fn parse_ident(&mut self) -> Result<Ident> {
    self
      .maybe_token_current
      .map(|token| match token.kind {
        TokenKind::Ident(symbol) => Ok(Ident {
          name: symbol,
          span: token.span,
        }),
        _ => panic!(), // return reporter error message.
      })
      .unwrap()
  }

  fn parse_block(&mut self) -> Result<Block> {
    let mut block = Block::new();
    let lo = self.current_span();

    self.expect_peek(TokenKind::Group(Group::BraceOpen))?;

    while !self.ensure_peek(TokenKind::Group(Group::BraceClose))
      && self.has_tokens()
    {
      self.next();
      block.add_stmt(self.parse_stmt()?);
    }

    self.expect_peek(TokenKind::Group(Group::BraceClose))?;

    let hi = self.current_span();
    let span = Span::merge(lo, hi);

    block.span = span;

    Ok(block)
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

  fn parse_ty(&mut self) -> Result<Ty> {
    self.next();

    let ty = self
      .maybe_token_current
      .map(|token| match token.kind {
        TokenKind::Punctuation(Punctuation::Colon) => self.parse_ty_type(),
        TokenKind::Op(Op::ColonEqual) => self.parse_ty_infer(),
        _ => panic!(), // return reporter error —  unexpected token.
      })
      .unwrap()?;

    if self.ensure_peek(TokenKind::Op(Op::Equal)) {
      self.next();
    }

    Ok(ty)
  }

  fn parse_ty_type(&mut self) -> Result<Ty> {
    self.next();

    self
      .maybe_token_current
      .map(|token| match &token.kind {
        TokenKind::Ident(symbol) => self.parse_ty_ident(symbol, token.span),
        TokenKind::Group(Group::BracketOpen) => self.parse_ty_array(),
        TokenKind::Kw(Kw::Fn) => self.parse_ty_fn(),
        _ => panic!(), // return reporter error —  unexpected token.
      })
      .unwrap()
  }

  fn parse_ty_ident(&mut self, symbol: &Symbol, span: Span) -> Result<Ty> {
    let ident = self.interner.lookup_ident(symbol);

    Ok(match ident {
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
      "bool" => Ty::bool(span),
      "char" => Ty::char(span),
      "str" => Ty::str(span),
      _ => Ty::alias(*symbol, span),
    })
  }

  fn parse_ty_array(&mut self) -> Result<Ty> {
    let lo = self.current_span();

    self.expect(TokenKind::Group(Group::BracketOpen))?;
    self.expect(TokenKind::Group(Group::BracketClose))?;

    let ty = self
      .maybe_token_current
      .map(|token| match token.kind {
        TokenKind::Ident(symbol) => self.parse_ty_ident(&symbol, token.span),
        _ => panic!(), // return reporter error message.
      })
      .unwrap()?;

    let hi = self.current_span();
    let span = Span::merge(lo, hi);

    Ok(Ty::array(ty, span))
  }

  fn parse_ty_fn(&mut self) -> Result<Ty> {
    let lo = self.current_span();

    self.next();

    let inputs = self.parse_ty_fn_inputs()?;
    let output = self.parse_ty_fn_output()?;
    let hi = self.current_span();
    let span = Span::merge(lo, hi);

    Ok(Ty::closure(inputs, output, span))
  }

  fn parse_ty_fn_inputs(&mut self) -> Result<Vec<Ty>> {
    let mut tys = Vec::with_capacity(0usize);

    while !self.ensure_peek(TokenKind::Group(Group::ParenClose)) {
      if self
        .expect_peek(TokenKind::Punctuation(Punctuation::Comma))
        .is_ok()
      {
        continue;
      }

      tys.push(self.parse_ty_type()?);
    }

    self.expect_peek(TokenKind::Group(Group::ParenClose))?;

    Ok(tys)
  }

  fn parse_ty_fn_output(&mut self) -> Result<Ty> {
    let output = self.parse_ty()?;

    Ok(output)
  }

  fn parse_ty_infer(&mut self) -> Result<Ty> {
    let span = self.current_span();

    Ok(Ty::infer(span))
  }

  fn parse_item(&mut self) -> Result<Item> {
    self
      .maybe_token_current
      .map(|token| match token.kind {
        TokenKind::Kw(Kw::Load) => self.parse_item_load(),
        TokenKind::Kw(Kw::Val) => self.parse_item_val(),
        TokenKind::Kw(Kw::Type) => self.parse_item_ty_alias(),
        TokenKind::Kw(Kw::Ext) => self.parse_item_ext(),
        TokenKind::Kw(Kw::Struct) => self.parse_item_struct(),
        TokenKind::Kw(Kw::Fun) => self.parse_item_fun(),
        _ => Err(ReportError::Syntax(Syntax::ExpectedItem(
          token.span,
          token.kind.to_string(),
        ))),
      })
      .unwrap()
  }

  // needs work — it's just a proof of concept this implementation  must be
  // defined next time.
  fn parse_item_load(&mut self) -> Result<Item> {
    let lo = self.current_span();

    self.next();

    self
      .maybe_token_current
      .map(|token| match token.kind {
        TokenKind::Str(symbol) => {
          use zo_reader::reader::Reader;
          use zo_tokenizer::tokenizer::Tokenizer;

          let pathname = self.interner.lookup_ident(symbol);

          // println!("\n{}", pathname);

          let pathname =
            format!("{}/{}", cargo_ws_root(), pathname.replace("\"", ""));

          // println!("\n{}", pathname);

          let source_bytes = Reader::new(self.reporter).read(pathname).unwrap();

          // println!("\n{:?}", source_bytes);

          let tokens =
            Tokenizer::new(self.interner, self.reporter, &source_bytes)
              .tokenize()
              .unwrap();

          // println!("\n{:?}", tokens);

          let ast = Parser::new(self.interner, self.reporter, &tokens)
            .parse()
            .unwrap();

          // println!("\n{:?}", ast);

          let span = ast.as_span();
          let hi = self.current_span();

          Ok(Item {
            kind: ItemKind::Load(Load { ast, span }),
            span: Span::merge(lo, hi),
          })
        }
        _ => panic!(),
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
          maybe_ty: Some(ty),
          value: Box::new(value),
          span,
        }),
        span,
      }),
      _ => panic!(), // TODO(ivs): should returns a reporter error.
    }
  }

  fn parse_item_ty_alias(&mut self) -> Result<Item> {
    let lo = self.current_span();

    self.next();

    let ident = self.parse_ident()?;

    self.expect_peek(TokenKind::Op(Op::Equal))?;

    let ty = self.parse_ty_type()?;
    let hi = self.current_span();
    let span = Span::merge(lo, hi);

    self.next();

    Ok(Item {
      kind: ItemKind::TyAlias(TyAlias {
        pubness: Pub::No,
        ident,
        maybe_ty: Some(ty),
        span,
      }),
      span,
    })
  }

  fn parse_item_ext(&mut self) -> Result<Item> {
    let lo = self.current_span();

    self.next();

    let prototype = self.parse_prototype()?;
    let hi = self.current_span();
    let span = Span::merge(lo, hi);

    Ok(Item {
      kind: ItemKind::Ext(Ext {
        pubness: Pub::No,
        prototype,
        maybe_body: None,
        span,
      }),
      span,
    })
  }

  fn parse_item_struct(&mut self) -> Result<Item> {
    let lo = self.current_span();

    self.next();

    let ident = self.parse_ident()?;
    let fields = self.parse_fields()?;
    let hi = self.current_span();
    let span = Span::merge(lo, hi);

    Ok(Item {
      kind: ItemKind::Struct(Struct {
        ident,
        fields,
        span,
      }),
      span,
    })
  }

  fn parse_fields(&mut self) -> Result<Fields> {
    let mut fields = Fields::new();

    self.expect_peek(TokenKind::Group(Group::BraceOpen))?;

    while !self.ensure_peek(TokenKind::Group(Group::BraceClose)) {
      if self
        .expect_peek(TokenKind::Punctuation(Punctuation::Comma))
        .is_ok()
      {
        continue;
      }

      fields.add_field(self.parse_field()?);

      self.next();
    }

    self.next();

    Ok(fields)
  }

  fn parse_field(&mut self) -> Result<Field> {
    self.next();

    let lo = self.current_span();
    let ident = self.parse_ident()?;
    let ty = self.parse_ty()?;
    let hi = self.current_span();
    let span = Span::merge(lo, hi);

    Ok(Field { ident, ty, span })
  }

  fn parse_item_fun(&mut self) -> Result<Item> {
    let lo = self.current_span();

    self.next();

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

  fn parse_inputs(&mut self) -> Result<Inputs> {
    let mut inputs = Inputs::new();

    self.expect_peek(TokenKind::Group(Group::ParenOpen))?;

    while !self.ensure_peek(TokenKind::Group(Group::ParenClose)) {
      if self
        .expect_peek(TokenKind::Punctuation(Punctuation::Comma))
        .is_ok()
      {
        continue;
      }

      self.next();
      inputs.add_input(self.parse_input()?);
    }

    self.expect_peek(TokenKind::Group(Group::ParenClose))?;

    Ok(inputs)
  }

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

  fn parse_output_ty(&mut self) -> Result<OutputTy> {
    // should be removed.
    if self.ensure_peek(TokenKind::Punctuation(Punctuation::MinusGreaterThan)) {
      self.next();
    }

    println!("{:?}", self.maybe_token_current);
    println!("{:?}", self.maybe_token_next);

    self
      .maybe_token_next
      .map(|token| match token.kind {
        TokenKind::Punctuation(Punctuation::Colon) => {
          let ty = self.parse_ty_type()?;

          Ok(OutputTy::Ty(ty))
        }
        TokenKind::Group(Group::BraceOpen) => Ok(OutputTy::Default(token.span)),
        TokenKind::Punctuation(Punctuation::Semicolon) => {
          Ok(OutputTy::Default(token.span))
        }
        _ => panic!(),
      })
      .unwrap()
  }

  fn parse_stmt(&mut self) -> Result<Stmt> {
    let stmt = self
      .maybe_token_current
      .map(|token| match token.kind {
        kind if kind.is_var_local() => self.parse_stmt_var(),
        kind if kind.is_item() => self.parse_stmt_item(),
        _ => self.parse_stmt_expr(),
      })
      .unwrap()?;

    if self.ensure_peek(TokenKind::Punctuation(Punctuation::Semicolon)) {
      self.next();
    }

    Ok(stmt)
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
          pubness: Pub::No,
          pattern,
          value: Box::new(value),
          maybe_ty: Some(ty),
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
          value: Box::new(value),
          maybe_ty: Some(ty),
          span,
        }),
        span,
      }),
      _ => panic!("expected local var."), // returns reporter error.
    }
  }

  fn parse_stmt_item(&mut self) -> Result<Stmt> {
    let item = self.parse_item()?;
    let span = item.span;

    Ok(Stmt {
      kind: StmtKind::Item(Box::new(item)),
      span,
    })
  }

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
      TokenKind::Group(Group::BraceOpen) => Some(Self::parse_expr_struct_expr),
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

  fn parse_expr_group(parser: &mut Parser) -> Result<Expr> {
    parser.next();

    let expr = parser.parse_expr(Precedence::Low)?;

    parser.expect_peek(TokenKind::Group(Group::ParenClose))?;

    Ok(expr)
  }

  fn parse_expr_array(parser: &mut Parser) -> Result<Expr> {
    let lo = parser.current_span();
    let elmts = parser.parse_exprs()?;
    let hi = parser.current_span();

    Ok(Expr {
      kind: ExprKind::Array(elmts),
      span: Span::merge(lo, hi),
    })
  }

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

  fn parse_expr_struct_expr(parser: &mut Parser) -> Result<Expr> {
    let mut pairs = vec![];
    let lo = parser.current_span();

    while !parser.ensure_peek(TokenKind::Group(Group::BraceClose)) {
      parser.next();

      let key = Self::parse_expr_lit_ident(parser)?;

      parser.expect_peek(TokenKind::Op(Op::Equal))?;

      parser.next();

      let value = parser.parse_expr(Precedence::Low)?;

      pairs.push((key, value));

      if !parser.ensure_peek(TokenKind::Group(Group::BraceClose)) {
        parser.expect_peek(TokenKind::Punctuation(Punctuation::Comma))?;
      }
    }

    parser.next();

    let hi = parser.current_span();

    Ok(Expr {
      kind: ExprKind::Struct(StructExpr { pairs }),
      span: Span::merge(lo, hi),
    })
  }

  // TODO(ivs): needs to be improve and verify each egde cases, what's happen if
  // we pass a statement instead of an expression when the needed is `->` and
  // so on.
  //
  // TODO(ivs): does the pattern span is correct ?
  fn parse_expr_fn(parser: &mut Parser) -> Result<Expr> {
    let lo = parser.current_span();
    let symbol = parser.interner.intern(&format!("anon_{}", parser.index));
    let name = parser.interner.lookup_ident(symbol);
    let span = Span::of(lo.hi, lo.hi + name.len());

    let pattern = Pattern {
      kind: PatternKind::Ident(Box::new(Expr {
        kind: ExprKind::Lit(Lit {
          kind: LitKind::Ident(symbol),
          span,
        }),
        span,
      })),
      span,
    };

    let lop = pattern.span;
    let inputs = parser.parse_inputs()?;

    let output_ty = OutputTy::Ty(Ty::infer(Span::of(
      inputs.as_span().hi,
      inputs.as_span().hi + 2,
    )));

    let hip = output_ty.as_span();

    let prototype = Prototype {
      pattern,
      inputs,
      output_ty,
      span: Span::merge(lop, hip),
    };

    let block = parser
      .maybe_token_next
      .map(|token| match token.kind {
        TokenKind::Punctuation(Punctuation::MinusGreaterThan) => {
          parser.next();
          parser.next();

          let expr = parser.parse_expr(Precedence::Low)?;
          let span = expr.span;

          Ok(Block {
            stmts: vec![Stmt {
              kind: StmtKind::Expr(Box::new(expr)),
              span,
            }],
            span,
          })
        }
        TokenKind::Group(Group::BraceOpen) => parser.parse_block(),
        _ => panic!(), // should return reporter error.
      })
      .unwrap()?;

    let hi = parser.current_span();
    let span = Span::merge(lo, hi);

    Ok(Expr {
      kind: ExprKind::Fn(prototype, block),
      span,
    })
  }

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

  fn parse_expr_loop(parser: &mut Parser) -> Result<Expr> {
    let lo = parser.current_span();
    let body = parser.parse_block()?;
    let hi = parser.current_span();

    Ok(Expr {
      kind: ExprKind::Loop(body),
      span: Span::merge(lo, hi),
    })
  }

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

    while parser.ensure_peek(TokenKind::Punctuation(Punctuation::Semicolon)) {
      parser.next();
    }

    Ok(Expr {
      kind: ExprKind::Return(Some(Box::new(expr))),
      span: Span::merge(lo, hi),
    })
  }

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

  fn parse_expr_continue(parser: &mut Parser) -> Result<Expr> {
    let lo = parser.current_span();

    if parser.ensure_peek(TokenKind::Punctuation(Punctuation::Semicolon)) {
      let hi = parser.current_span();
      let span = Span::merge(lo, hi);

      parser.next();

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

  fn parse_infix_fn(&self) -> Option<ParseInfixFn> {
    let token = self.maybe_token_next.unwrap(); // should be unwrap properly.

    match token.kind {
      kind if kind.is_binop() => Some(Self::parse_expr_binop),
      kind if kind.is_assignement() => Some(Self::parse_expr_assignment),
      kind if kind.is_calling() => Some(Self::parse_expr_call),
      kind if kind.is_index() => Some(Self::parse_expr_array_access),
      kind if kind.is_chaining() => Some(Self::parse_expr_struct_access),
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

  fn parse_expr_struct_access(parser: &mut Parser, lhs: Expr) -> Result<Expr> {
    let lo = lhs.span;

    parser.next();

    let access = parser.parse_expr(Precedence::Chaining)?;
    let hi = parser.current_span();

    Ok(Expr {
      kind: ExprKind::StructAccess(Box::new(lhs), Box::new(access)),
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
  Parser::new(&mut session.interner, &mut session.reporter, tokens).parse()
}

pub fn cargo_ws_root() -> String {
  let program = env!("CARGO");

  let output = std::process::Command::new(program)
    .args(["locate-project", "--workspace", "--message-format=plain"])
    .output()
    .unwrap()
    .stdout;

  let cargo_path = std::path::Path::new(match std::str::from_utf8(&output) {
    Ok(path) => path.trim(),
    Err(error) => panic!("{error}"),
  });

  cargo_path
    .parent()
    .unwrap()
    .to_path_buf()
    .display()
    .to_string()
}
