//! ...

use zo_ast::ast::{
  Ast, BinOp, BinOpKind, Expr, ExprKind, Lit, LitKind, Pattern, PatternKind,
  Stmt, StmtKind, UnOp, UnOpKind, Var,
};

use zo_core::interner::symbol::Symbol;
use zo_core::interner::Interner;
use zo_core::reporter::Reporter;
use zo_core::writer::Writer;
use zo_core::{to, Result};

pub(crate) struct Translator<'ast> {
  interner: &'ast Interner,
  reporter: &'ast Reporter,
  writer: Writer,
}

impl<'ast> Translator<'ast> {
  #[inline]
  pub fn new(interner: &'ast Interner, reporter: &'ast Reporter) -> Self {
    Self {
      interner,
      reporter,
      writer: Writer::new(2usize),
    }
  }

  #[inline]
  pub fn output(&mut self) -> Result<Box<[u8]>> {
    Ok(self.writer.as_bytes())
  }

  pub fn translate(&mut self, ast: &Ast) -> Result<()> {
    self.writer.write_bytes(b"def main():")?;

    for stmt in &ast.stmts {
      self.writer.new_line()?;
      self.writer.indent();
      self.translate_stmt(stmt)?;
      self.writer.indent();
    }

    self.writer.new_line()?;
    self.writer.new_line()?;

    self.writer.writeln_bytes(
      br#"if __name__ == "__main__":
  main()"#,
    )?;

    self.reporter.abort_if_has_errors();

    Ok(())
  }

  fn translate_pattern(&mut self, pattern: &Pattern) -> Result<()> {
    match &pattern.kind {
      PatternKind::Underscore => self.writer.write_bytes(b"_"),
      PatternKind::Ident(ident) => self.translate_expr(ident),
      PatternKind::Lit(lit) => self.translate_expr_lit(lit),
      _ => todo!(),
    }
  }

  fn translate_stmt(&mut self, stmt: &Stmt) -> Result<()> {
    match &stmt.kind {
      StmtKind::Var(var) => self.translate_stmt_var(var),
      StmtKind::Expr(expr) => self.translate_stmt_expr(expr),
    }
  }

  fn translate_stmt_var(&mut self, var: &Var) -> Result<()> {
    self.translate_pattern(&var.pattern)?;
    self.writer.space()?;
    self.writer.write_bytes(b"=")?;
    self.writer.space()?;
    self.translate_expr(&var.value)
  }

  fn translate_stmt_expr(&mut self, expr: &Expr) -> Result<()> {
    self.translate_expr(expr)
  }

  fn translate_expr(&mut self, expr: &Expr) -> Result<()> {
    match &expr.kind {
      ExprKind::Lit(lit) => self.translate_expr_lit(lit),
      ExprKind::UnOp(unop, rhs) => self.translate_expr_unop(unop, rhs),
      ExprKind::BinOp(binop, lhs, rhs) => {
        self.translate_expr_binop(binop, lhs, rhs)
      }
      _ => todo!(),
    }
  }

  fn translate_expr_lit(&mut self, lit: &Lit) -> Result<()> {
    match &lit.kind {
      LitKind::Int(symbol) => self.translate_expr_lit_int(symbol),
      LitKind::Float(symbol) => self.translate_expr_lit_float(symbol),
      LitKind::Ident(symbol) => self.translate_expr_lit_ident(symbol),
      LitKind::Bool(boolean) => self.translate_expr_lit_bool(boolean),
      LitKind::Char(symbol) => self.translate_expr_lit_char(symbol),
      LitKind::Str(symbol) => self.translate_expr_lit_str(symbol),
    }
  }

  fn translate_expr_lit_int(&mut self, symbol: &Symbol) -> Result<()> {
    let int = self.interner.lookup_int(*symbol);

    self.writer.write_int(int)
  }

  fn translate_expr_lit_float(&mut self, symbol: &Symbol) -> Result<()> {
    let float = self.interner.lookup_float(*symbol);

    self.writer.write_float(float)
  }

  fn translate_expr_lit_ident(&mut self, symbol: &Symbol) -> Result<()> {
    let ident = self.interner.lookup_ident(*symbol);

    self.writer.write(ident)
  }

  fn translate_expr_lit_bool(&mut self, boolean: &bool) -> Result<()> {
    let boolean = to!(pascal format!("{boolean}"));

    self.writer.write(boolean)
  }

  fn translate_expr_lit_char(&mut self, _symbol: &Symbol) -> Result<()> {
    todo!()
  }

  fn translate_expr_lit_str(&mut self, symbol: &Symbol) -> Result<()> {
    let string = self.interner.lookup_str(*symbol);

    self.writer.write(string)
  }

  fn translate_expr_unop(&mut self, unop: &UnOp, rhs: &Expr) -> Result<()> {
    match unop.kind {
      UnOpKind::Neg => self.translate_expr_unop_neg(unop, rhs),
      UnOpKind::Not => self.translate_expr_unop_not(unop, rhs),
    }
  }

  fn translate_expr_unop_neg(
    &mut self,
    _unop: &UnOp,
    _rhs: &Expr,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_unop_not(
    &mut self,
    _unop: &UnOp,
    _rhs: &Expr,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_binop(
    &mut self,
    binop: &BinOp,
    lhs: &Expr,
    rhs: &Expr,
  ) -> Result<()> {
    match binop.kind {
      BinOpKind::Add => self.translate_expr_binop_add(binop, lhs, rhs),
      BinOpKind::Sub => self.translate_expr_binop_sub(binop, lhs, rhs),
      BinOpKind::Mul => self.translate_expr_binop_mul(binop, lhs, rhs),
      BinOpKind::Div => self.translate_expr_binop_div(binop, lhs, rhs),
      _ => todo!(),
    }
  }

  fn translate_expr_binop_add(
    &mut self,
    _binop: &BinOp,
    _lhs: &Expr,
    _rhs: &Expr,
  ) -> Result<()> {
    let lhs = 1;
    let rhs = 2;

    self.writer.write(format!("{lhs} + {rhs}"))
  }

  fn translate_expr_binop_sub(
    &mut self,
    _binop: &BinOp,
    _lhs: &Expr,
    _rhs: &Expr,
  ) -> Result<()> {
    let lhs = 1;
    let rhs = 2;

    self.writer.write(format!("{lhs} - {rhs}"))
  }

  fn translate_expr_binop_mul(
    &mut self,
    _binop: &BinOp,
    _lhs: &Expr,
    _rhs: &Expr,
  ) -> Result<()> {
    let lhs = 1;
    let rhs = 2;

    self.writer.write(format!("{lhs} * {rhs}"))
  }

  fn translate_expr_binop_div(
    &mut self,
    _binop: &BinOp,
    _lhs: &Expr,
    _rhs: &Expr,
  ) -> Result<()> {
    let lhs = 1;
    let rhs = 2;

    self.writer.write(format!("{lhs} / {rhs}"))
  }
}
