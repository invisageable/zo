//! ...

#![allow(dead_code)]

use zhoo_ast::ast;
use zhoo_session::session::Session;
use zhoo_ty::ty::Ty;

use zo_core::{span::Span, Result};

#[derive(Debug)]
struct Tychecker {}

impl Tychecker {
  #[inline]
  fn new() -> Self {
    Self {}
  }

  fn check(&mut self) -> Result<()> {
    Ok(())
  }

  fn check_item(&mut self, item: &ast::Item) -> Result<()> {
    match &item.kind {
      ast::ItemKind::Var(item) => self.check_item_var(item),
      ast::ItemKind::Fun(expr) => self.check_item_fun(expr),
      _ => todo!(),
    }
  }

  fn check_item_var(&mut self, var: &ast::Var) -> Result<()> {
    self.check_var(var)
  }

  fn check_var(&mut self, _var: &ast::Var) -> Result<()> {
    todo!()
  }

  fn check_item_fun(&mut self, _fun: &ast::Fun) -> Result<()> {
    todo!()
  }

  fn check_stmt(&mut self, stmt: &ast::Stmt) -> Result<()> {
    match &stmt.kind {
      ast::StmtKind::Var(var) => self.check_stmt_var(var),
      ast::StmtKind::Item(item) => self.check_stmt_item(item),
      ast::StmtKind::Expr(expr) => self.check_stmt_expr(expr),
    }
  }

  fn check_stmt_var(&mut self, var: &ast::Var) -> Result<()> {
    self.check_var(var)
  }

  fn check_stmt_item(&mut self, item: &ast::Item) -> Result<()> {
    self.check_item(item)?;

    Ok(())
  }

  fn check_stmt_expr(&mut self, expr: &ast::Expr) -> Result<()> {
    self.check_expr(expr)?;

    Ok(())
  }

  fn check_expr(&mut self, expr: &ast::Expr) -> Result<Ty> {
    match &expr.kind {
      ast::ExprKind::Lit(lit) => self.check_expr_lit(lit),
      _ => todo!(),
    }
  }

  fn check_expr_lit(&mut self, lit: &ast::Lit) -> Result<Ty> {
    Ok(match &lit.kind {
      ast::LitKind::Int(_) => Ty::int(lit.span),
      ast::LitKind::Float(_) => Ty::float(lit.span),
      ast::LitKind::Ident(_) => todo!(),
      ast::LitKind::Bool(_) => Ty::bool(lit.span),
      ast::LitKind::Char(_) => Ty::char(lit.span),
      ast::LitKind::Str(_) => Ty::str(lit.span),
    })
  }

  fn ensure(&self, t1: &Ty, t2: &Ty) {
    self.check_eq(t1, t2);
  }

  fn ensure_unit(&self, t1: &Ty) {
    self.check_eq(t1, &Ty::unit(Span::ZERO));
  }

  fn ensure_int(&self, t1: &Ty) {
    self.check_eq(t1, &Ty::int(Span::ZERO));
  }

  fn ensure_float(&self, t1: &Ty) {
    self.check_eq(t1, &Ty::float(Span::ZERO));
  }

  fn ensure_bool(&self, t1: &Ty) {
    self.check_eq(t1, &Ty::bool(Span::ZERO));
  }

  fn ensure_char(&self, t1: &Ty) {
    self.check_eq(t1, &Ty::char(Span::ZERO));
  }

  fn ensure_str(&self, t1: &Ty) {
    self.check_eq(t1, &Ty::str(Span::ZERO));
  }

  fn check_eq(&self, t1: &Ty, t2: &Ty) -> bool {
    if t1.kind != t2.kind {
      return false;
    }

    true
  }
}

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn check(_session: &mut Session) -> Result<()> {
  Tychecker::new().check()
}
