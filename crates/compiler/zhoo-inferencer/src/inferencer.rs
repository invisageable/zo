use zhoo_ast::ast;
use zhoo_session::session::Session;
use zhoo_ty::ty;

use zo_core::interner::Interner;
use zo_core::reporter::Reporter;
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

  fn infer(&mut self, program: &ast::Program) -> Result<ty::Ty> {
    let mut ty = ty::Ty::UNIT;

    for item in program.items.iter() {
      ty = self.infer_item(item)?;
    }

    Ok(ty)
  }

  fn infer_item(&mut self, item: &ast::Item) -> Result<ty::Ty> {
    match &item.kind {
      ast::ItemKind::Fun(fun) => self.infer_item_fun(fun),
    }
  }

  fn infer_item_fun(&mut self, fun: &ast::Fun) -> Result<ty::Ty> {
    let mut ty = ty::Ty::UNIT;

    for stmt in fun.body.stmts.iter() {
      ty = self.infer_stmt(stmt)?;
    }

    println!("ITEM_FUN: {ty:?}");
    Ok(ty::Ty::fun())
  }

  fn infer_stmt(&mut self, stmt: &ast::Stmt) -> Result<ty::Ty> {
    match &stmt.kind {
      ast::StmtKind::Expr(expr) => self.infer_stmt_expr(expr),
    }
  }

  fn infer_stmt_expr(&mut self, expr: &ast::Expr) -> Result<ty::Ty> {
    match &expr.kind {
      ast::ExprKind::Int(_) => self.infer_item_expr_int(),
      ast::ExprKind::Float(_) => self.infer_item_expr_float(),
      ast::ExprKind::Ident(ident) => self.infer_item_expr_ident(ident),
    }
  }

  fn infer_item_expr_int(&mut self) -> Result<ty::Ty> {
    Ok(ty::Ty::int())
  }

  fn infer_item_expr_float(&mut self) -> Result<ty::Ty> {
    Ok(ty::Ty::float())
  }

  fn infer_item_expr_ident(&mut self, ident: &String) -> Result<ty::Ty> {
    Ok(ty::Ty::ident(ident.into()))
  }
}

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn infer(session: &mut Session, program: &ast::Program) -> Result<ty::Ty> {
  Inferencer::new(&mut session.interner, &session.reporter).infer(program)
}
