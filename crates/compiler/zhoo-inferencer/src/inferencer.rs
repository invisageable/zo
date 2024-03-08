//! ...wip.

use zhoo_ast::ast;
use zhoo_session::session::Session;
use zhoo_ty::ty::Ty;

use zo_core::interner::symbol::Symbol;
use zo_core::interner::Interner;
use zo_core::reporter::Reporter;
use zo_core::span::Span;
use zo_core::Result;

// note #1 — safe, globals cannot be declared without type.
//
// note #2 — in case of block should we return an `unit` type of the last
// statement type of a block.

#[derive(Debug)]
struct Inferencer<'program> {
  #[allow(dead_code)]
  interner: &'program mut Interner,
  #[allow(dead_code)]
  reporter: &'program Reporter,
}

impl<'program> Inferencer<'program> {
  #[inline]
  fn new(
    interner: &'program mut Interner,
    reporter: &'program Reporter,
  ) -> Self {
    Self { interner, reporter }
  }

  fn infer(&mut self, program: &ast::Program) -> Result<Ty> {
    let mut ty = Ty::UNIT;

    for item in program.items.iter() {
      ty = self.infer_item(item)?;
    }

    Ok(ty)
  }

  fn infer_item(&mut self, item: &ast::Item) -> Result<Ty> {
    match &item.kind {
      ast::ItemKind::Ext(ext) => self.infer_item_ext(ext),
      ast::ItemKind::Var(var) => self.infer_item_var(var),
      ast::ItemKind::TyAlias(ty_alias) => self.infer_item_ty_alias(ty_alias),
      ast::ItemKind::Fun(fun) => self.infer_item_fun(fun),
    }
  }

  fn infer_item_ext(&mut self, ext: &ast::Ext) -> Result<Ty> {
    let t1 = self.infer_prototype(&ext.prototype)?;

    let _t2 = match &ext.maybe_body {
      Some(body) => self.infer_block(body),
      None => unreachable!(),
    };

    Ok(t1)
  }

  fn infer_prototype(&mut self, prototype: &ast::Prototype) -> Result<Ty> {
    match &prototype.output {
      ast::OutputTy::Ty(ty) => Ok(Ty::from(ty)),
      ast::OutputTy::Default(span) => Ok(Ty::unit(*span)),
    }
  }

  fn infer_item_ty_alias(&mut self, ty_alias: &ast::TyAlias) -> Result<Ty> {
    match &ty_alias.maybe_ty {
      Some(ty) => Ok(Ty::from(ty)),
      None => unreachable!(), // note #1.
    }
  }

  fn infer_item_var(&mut self, var: &ast::Var) -> Result<Ty> {
    self.infer_var(var)
  }

  fn infer_item_fun(&mut self, fun: &ast::Fun) -> Result<Ty> {
    let t1 = self.infer_prototype(&fun.prototype)?;
    let _t2 = self.infer_block(&fun.body)?;

    Ok(t1)
  }

  // note #2.
  fn infer_block(&mut self, block: &ast::Block) -> Result<Ty> {
    let mut ty = Ty::UNIT;

    for stmt in &block.stmts {
      ty = self.infer_stmt(stmt)?;
    }

    Ok(ty)
  }

  fn infer_stmt(&mut self, stmt: &ast::Stmt) -> Result<Ty> {
    match &stmt.kind {
      ast::StmtKind::Var(var) => self.infer_stmt_var(var),
      ast::StmtKind::Item(fun) => self.infer_item(fun),
      ast::StmtKind::Expr(expr) => self.infer_expr(expr),
    }
  }

  fn infer_stmt_var(&mut self, var: &ast::Var) -> Result<Ty> {
    self.infer_var(var)
  }

  fn infer_var(&mut self, var: &ast::Var) -> Result<Ty> {
    match &var.maybe_ty {
      Some(ty) => Ok(Ty::from(ty)),
      None => self.infer_expr(&var.value),
    }
  }

  fn infer_expr(&mut self, expr: &ast::Expr) -> Result<Ty> {
    match &expr.kind {
      ast::ExprKind::Lit(lit) => self.infer_expr_lit(lit),
      ast::ExprKind::UnOp(unop, rhs) => self.infer_expr_unop(unop, rhs),
      ast::ExprKind::BinOp(binop, lhs, rhs) => {
        self.infer_expr_binop(binop, lhs, rhs)
      }
      ast::ExprKind::Assign(_, rhs) => self.infer_expr_assign(rhs),
      ast::ExprKind::AssignOp(_, lhs, rhs) => {
        self.infer_expr_assignop(lhs, rhs)
      }
      ast::ExprKind::Block(block) => self.infer_expr_block(block),
      ast::ExprKind::Array(exprs) => self.infer_expr_array(exprs),
      ast::ExprKind::Tuple(exprs) => self.infer_expr_tuple(exprs),
      ast::ExprKind::ArrayAccess(array, access) => {
        self.infer_expr_array_access(array, access)
      }
      ast::ExprKind::TupleAccess(array, access) => {
        self.infer_expr_tuple_access(array, access)
      }
      ast::ExprKind::Fn(prototype, body) => self.infer_expr_fn(prototype, body),
      ast::ExprKind::Call(callee, args) => self.infer_expr_call(callee, args),
      ast::ExprKind::Return(maybe_expr) => self.infer_expr_return(maybe_expr),
      ast::ExprKind::IfElse(condition, consequence, alternative) => {
        self.infer_expr_if_else(condition, consequence, alternative)
      }
      ast::ExprKind::When(condition, consequence, alternative) => {
        self.infer_expr_when(condition, consequence, alternative)
      }
      ast::ExprKind::Match(condition, arms) => {
        self.infer_expr_match(condition, arms)
      }
      ast::ExprKind::Loop(block) => self.infer_expr_loop(block),
      ast::ExprKind::While(condition, body) => {
        self.infer_expr_while(condition, body)
      }
      ast::ExprKind::For(for_loop) => self.infer_expr_for(for_loop),
      ast::ExprKind::Break(maybe_expr) => self.infer_expr_break(maybe_expr),
      ast::ExprKind::Continue => self.infer_expr_continue(),
      ast::ExprKind::Var(var) => self.infer_expr_var(var),
      ast::ExprKind::StructExpr(struct_expr) => {
        self.infer_expr_struct_expr(struct_expr)
      }
      ast::ExprKind::Chaining(lhs, rhs) => self.infer_expr_chaining(lhs, rhs),
    }
  }

  fn infer_expr_lit(&mut self, lit: &ast::Lit) -> Result<Ty> {
    match &lit.kind {
      ast::LitKind::Int(int) => self.infer_expr_lit_int(int, lit.span),
      ast::LitKind::Float(float) => self.infer_expr_lit_float(float, lit.span),
      ast::LitKind::Ident(ident) => self.infer_expr_lit_ident(ident, lit.span),
      ast::LitKind::Bool(_) => self.infer_expr_lit_bool(lit.span),
      ast::LitKind::Char(_) => self.infer_expr_lit_char(lit.span),
      ast::LitKind::Str(_) => self.infer_expr_lit_str(lit.span),
    }
  }

  fn infer_expr_lit_int(&mut self, _int: &Symbol, span: Span) -> Result<Ty> {
    Ok(Ty::int(span))
  }

  fn infer_expr_lit_float(
    &mut self,
    _symbol: &Symbol,
    span: Span,
  ) -> Result<Ty> {
    Ok(Ty::float(span))
  }

  fn infer_expr_lit_ident(&mut self, ident: &Symbol, span: Span) -> Result<Ty> {
    Ok(Ty::ident(*ident, span))
  }

  fn infer_expr_lit_bool(&mut self, span: Span) -> Result<Ty> {
    Ok(Ty::bool(span))
  }

  fn infer_expr_lit_char(&mut self, span: Span) -> Result<Ty> {
    Ok(Ty::char(span))
  }

  fn infer_expr_lit_str(&mut self, span: Span) -> Result<Ty> {
    Ok(Ty::str(span))
  }

  fn infer_expr_unop(
    &mut self,
    _unop: &ast::UnOp,
    _rhs: &ast::Expr,
  ) -> Result<Ty> {
    Ok(Ty::UNIT)
  }

  fn infer_expr_binop(
    &mut self,
    _binop: &ast::BinOp,
    _lhs: &ast::Expr,
    _rhs: &ast::Expr,
  ) -> Result<Ty> {
    Ok(Ty::UNIT)
  }

  fn infer_expr_return(
    &mut self,
    maybe_expr: &Option<Box<ast::Expr>>,
  ) -> Result<Ty> {
    match maybe_expr {
      Some(expr) => self.infer_expr(expr),
      None => Ok(Ty::UNIT),
    }
  }

  fn infer_expr_if_else(
    &mut self,
    condition: &ast::Expr,
    consequence: &ast::Block,
    maybe_alternative: &Option<Box<ast::Expr>>,
  ) -> Result<Ty> {
    let _t1 = self.infer_expr(condition)?;
    let t2 = self.infer_block(consequence)?;

    match maybe_alternative {
      Some(alternative) => {
        let t3 = self.infer_expr(alternative)?;

        if !t2.is(t3.kind) {
          panic!("mismatch types.");
        }

        Ok(t2)
      }
      None => Ok(t2),
    }
  }

  fn infer_expr_when(
    &mut self,
    condition: &ast::Expr,
    consequence: &ast::Expr,
    alternative: &ast::Expr,
  ) -> Result<Ty> {
    let _t1 = self.infer_expr(condition)?;
    let t2 = self.infer_expr(consequence)?;
    let t3 = self.infer_expr(alternative)?;

    if !t2.is(t3.kind) {
      panic!("mismatch types.");
    }

    Ok(t2)
  }

  fn infer_expr_match(
    &mut self,
    _condition: &ast::Expr,
    _arms: &[ast::Arm],
  ) -> Result<Ty> {
    todo!()
  }

  fn infer_expr_break(
    &mut self,
    maybe_expr: &Option<Box<ast::Expr>>,
  ) -> Result<Ty> {
    match maybe_expr {
      Some(expr) => self.infer_expr(expr),
      None => Ok(Ty::UNIT),
    }
  }

  fn infer_expr_continue(&mut self) -> Result<Ty> {
    Ok(Ty::UNIT)
  }

  fn infer_expr_assign(&mut self, rhs: &ast::Expr) -> Result<Ty> {
    let ty = self.infer_expr(rhs)?;

    Ok(ty)
  }

  // note #2.
  fn infer_expr_block(&mut self, block: &ast::Block) -> Result<Ty> {
    let _ty = self.infer_block(block)?;

    Ok(Ty::UNIT)
  }

  fn infer_expr_array(&mut self, exprs: &[ast::Expr]) -> Result<Ty> {
    let mut exprs = exprs.iter();
    let maybe_expr = exprs.next();

    match maybe_expr {
      Some(expr) => {
        let t1 = self.infer_expr(expr)?;

        for expr in exprs {
          let t2 = self.infer_expr(expr)?;

          if !t1.is(t2.kind) {
            panic!("mismatch types");
          }
        }

        Ok(t1)
      }
      None => unreachable!(),
    }
  }

  fn infer_expr_tuple(&mut self, _exprs: &[ast::Expr]) -> Result<Ty> {
    todo!()
  }

  fn infer_expr_array_access(
    &mut self,
    _array: &ast::Expr,
    _access: &ast::Expr,
  ) -> Result<Ty> {
    todo!()
  }

  fn infer_expr_tuple_access(
    &mut self,
    _tuple: &ast::Expr,
    _access: &ast::Expr,
  ) -> Result<Ty> {
    todo!()
  }

  // note #2.
  fn infer_expr_fn(
    &mut self,
    prototype: &ast::Prototype,
    body: &ast::Block,
  ) -> Result<Ty> {
    let t1 = self.infer_prototype(prototype)?;
    let _t2 = self.infer_block(body)?;

    Ok(t1)
  }

  fn infer_expr_call(
    &mut self,
    _callee: &ast::Expr,
    _args: &ast::Args,
  ) -> Result<Ty> {
    todo!()
  }

  fn infer_expr_assignop(
    &mut self,
    _lhs: &ast::Expr,
    rhs: &ast::Expr,
  ) -> Result<Ty> {
    let ty = self.infer_expr(rhs)?;

    Ok(ty)
  }

  fn infer_expr_loop(&mut self, block: &ast::Block) -> Result<Ty> {
    let _ty = self.infer_block(block)?;

    Ok(Ty::UNIT)
  }

  fn infer_expr_while(
    &mut self,
    condition: &ast::Expr,
    body: &ast::Block,
  ) -> Result<Ty> {
    let _t1 = self.infer_expr(condition)?;
    let _t2 = self.infer_block(body)?;

    Ok(Ty::UNIT)
  }

  fn infer_expr_for(&mut self, _for_loop: &ast::For) -> Result<Ty> {
    todo!()
  }

  fn infer_expr_var(&mut self, var: &ast::Var) -> Result<Ty> {
    self.infer_var(var)
  }

  fn infer_expr_struct_expr(
    &mut self,
    _struct_expr: &ast::StructExpr,
  ) -> Result<Ty> {
    todo!()
  }

  fn infer_expr_chaining(
    &mut self,
    _lhs: &ast::Expr,
    _rhs: &ast::Expr,
  ) -> Result<Ty> {
    todo!()
  }
}

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn infer(session: &mut Session, program: &ast::Program) -> Result<Ty> {
  Inferencer::new(&mut session.interner, &session.reporter).infer(program)
}
