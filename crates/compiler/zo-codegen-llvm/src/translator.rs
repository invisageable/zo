use zo_ast::ast;
use zo_interner::interner::symbol::Symbol;
use zo_interner::interner::Interner;
use zo_reporter::reporter::Reporter;
use zo_reporter::Result;

use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::values::IntValue;

/// The representation of a `llvm` translator.
pub struct Translator<'a, 'ctx> {
  pub ctx: &'ctx Context,
  pub builder: &'a Builder<'ctx>,
  pub module: &'a Module<'ctx>,
  pub interner: &'a mut Interner,
  pub reporter: &'a mut Reporter,
}

impl<'a, 'ctx> Translator<'a, 'ctx> {
  /// Translates an AST.
  pub fn translate(&mut self, ast: &ast::Ast) -> Result<()> {
    for stmt in ast.iter() {
      let value = self.translate_stmt(stmt)?;
      println!("Value = {value}");
    }

    self.reporter.abort_if_has_errors();

    Ok(())
  }

  /// Translates a statement.
  fn translate_stmt(&mut self, stmt: &ast::Stmt) -> Result<IntValue> {
    match &stmt.kind {
      ast::StmtKind::Expr(expr) => self.translate_expr(expr),
      _ => todo!(),
    }
  }

  /// Translates an expression.
  fn translate_expr(&self, expr: &ast::Expr) -> Result<IntValue> {
    match &expr.kind {
      ast::ExprKind::Lit(lit) => self.translate_expr_lit(lit),
      ast::ExprKind::UnOp(unop, lhs) => self.translate_expr_unop(unop, lhs),
      ast::ExprKind::BinOp(binop, lhs, rhs) => {
        self.translate_expr_binop(binop, lhs, rhs)
      }
      _ => todo!(),
    }
  }

  /// Translates a literal expression.
  fn translate_expr_lit(&self, lit: &ast::Lit) -> Result<IntValue> {
    match &lit.kind {
      ast::LitKind::Int(sym, _) => self.translate_expr_lit_int(sym),
      _ => todo!(),
    }
  }

  /// Translates a integer literal expression.
  fn translate_expr_lit_int(&self, sym: &Symbol) -> Result<IntValue> {
    let int = self.interner.lookup_int(**sym as usize);

    Ok(self.ctx.i64_type().const_int(int as u64, false))
  }

  /// Translates an unary operation expression.
  fn translate_expr_unop(
    &self,
    unop: &ast::UnOp,
    rhs: &ast::Expr,
  ) -> Result<IntValue> {
    let rhs = self.translate_expr(rhs)?;

    match unop.kind {
      ast::UnOpKind::Neg => Ok(self.builder.build_int_neg(rhs, "neg")),
      ast::UnOpKind::Not => Ok(self.builder.build_not(rhs, "not")),
    }
  }

  /// Translates an binary operation expression.
  fn translate_expr_binop(
    &self,
    binop: &ast::BinOp,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
  ) -> Result<IntValue> {
    let lhs = self.translate_expr(lhs)?;
    let rhs = self.translate_expr(rhs)?;

    match binop.kind {
      ast::BinOpKind::Add => Ok(self.builder.build_int_add(lhs, rhs, "add")),
      ast::BinOpKind::Sub => Ok(self.builder.build_int_sub(lhs, rhs, "sub")),
      ast::BinOpKind::Mul => Ok(self.builder.build_int_mul(lhs, rhs, "sub")),
      ast::BinOpKind::Div => {
        Ok(self.builder.build_int_signed_div(lhs, rhs, "div"))
      }
      _ => todo!(),
    }
  }
}
