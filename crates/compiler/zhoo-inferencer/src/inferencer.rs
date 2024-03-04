//! ...

use zhoo_ast::ast;
use zhoo_session::session::Session;
use zhoo_ty::ty::Ty;

use zo_core::interner::symbol::Symbol;
use zo_core::interner::Interner;
use zo_core::reporter::Reporter;
use zo_core::span::Span;
use zo_core::Result;

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
      ast::ItemKind::Fun(fun) => self.infer_item_fun(fun),
      _ => todo!(),
    }
  }

  fn infer_item_fun(&mut self, fun: &ast::Fun) -> Result<Ty> {
    for stmt in &fun.body.stmts {
      self.infer_stmt(stmt)?;
    }

    Ok(Ty::fun(fun.span))
  }

  fn infer_stmt(&mut self, stmt: &ast::Stmt) -> Result<Ty> {
    match &stmt.kind {
      ast::StmtKind::Var(var) => self.infer_stmt_var(var),
      ast::StmtKind::Item(fun) => self.infer_item(fun),
      ast::StmtKind::Expr(expr) => self.infer_expr(expr),
    }
  }

  fn infer_stmt_var(&mut self, _var: &ast::Var) -> Result<Ty> {
    Ok(Ty::UNIT)
  }

  fn infer_expr(&mut self, expr: &ast::Expr) -> Result<Ty> {
    match &expr.kind {
      ast::ExprKind::Lit(lit) => self.infer_expr_lit(lit),
      ast::ExprKind::UnOp(unop, rhs) => self.infer_expr_unop(unop, rhs),
      ast::ExprKind::BinOp(binop, lhs, rhs) => {
        self.infer_expr_binop(binop, lhs, rhs)
      }
      ast::ExprKind::Return(maybe_expr) => self.infer_expr_return(maybe_expr),
      ast::ExprKind::Break(maybe_expr) => self.infer_expr_break(maybe_expr),
      ast::ExprKind::Continue => self.infer_expr_continue(),
      _ => todo!(),
    }
  }

  #[allow(dead_code)]
  fn infer_stmt_expr(&mut self, expr: &ast::Expr) -> Result<Ty> {
    match &expr.kind {
      ast::ExprKind::Lit(lit) => self.infer_expr_lit(lit),
      _ => todo!(),
    }
  }

  fn infer_expr_lit(&mut self, lit: &ast::Lit) -> Result<Ty> {
    match &lit.kind {
      ast::LitKind::Int(int) => self.infer_expr_lit_int(int, lit.span),
      ast::LitKind::Float(float) => self.infer_expr_lit_float(float, lit.span),
      ast::LitKind::Ident(ident) => self.infer_expr_lit_ident(ident, lit.span),
      _ => todo!(),
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
