//! ...

use super::interface::Wat;

use zhoo_ast::ast;
use zhoo_ty::ty::Ty;

use zo_core::interner::symbol::{Symbol, Symbolize};
use zo_core::interner::Interner;
use zo_core::reporter::Reporter;
use zo_core::writer::Writer;
use zo_core::Result;

// todo #1 — the `mir` should have a `Ty`. Normally, this detail must be
// implements from the inferencer side.

pub(crate) struct Translator<'mir> {
  interner: &'mir Interner,
  reporter: &'mir Reporter,
  writer: Writer,
}

impl<'mir> Translator<'mir> {
  #[inline]
  pub fn new(interner: &'mir Interner, reporter: &'mir Reporter) -> Self {
    Self {
      interner,
      reporter,
      writer: Writer::new(),
    }
  }

  pub fn output(&mut self) -> Result<Box<[u8]>> {
    Ok(self.writer.as_bytes())
  }

  pub fn translate(&mut self, program: &ast::Program) -> Result<()> {
    self.writer.write_bytes(b"(module")?;

    for item in &program.items {
      self.writer.indent();

      if let Err(report_error) = self.translate_item(item) {
        self.reporter.add_report(report_error)
      }

      self.writer.dedent();
    }

    self.writer.writeln(')')?;
    self.reporter.abort_if_has_errors();

    Ok(())
  }

  fn translate_item(&mut self, item: &ast::Item) -> Result<()> {
    match &item.kind {
      ast::ItemKind::Var(var) => self.translate_item_var(var),
      ast::ItemKind::Fun(fun) => self.translate_item_fun(fun),
      _ => Ok(()),
    }
  }

  fn translate_item_var(&mut self, var: &ast::Var) -> Result<()> {
    self.translate_var(var)
  }

  fn translate_var(&mut self, _var: &ast::Var) -> Result<()> {
    Ok(())
  }

  fn translate_pattern(&mut self, pattern: &ast::Pattern) -> Result<()> {
    match &pattern.kind {
      ast::PatternKind::Ident(ident) => self.translate_expr(ident),
      ast::PatternKind::Lit(lit) => self.translate_expr_lit(lit),
      ast::PatternKind::MeLower => todo!(),
      _ => todo!(),
    }
  }

  fn translate_item_fun(&mut self, fun: &ast::Fun) -> Result<()> {
    self.writer.write_bytes(b"(func")?;
    self.translate_prototype(&fun.prototype)?;
    self.translate_body(&fun.body)?;
    self.writer.write(')')?;

    Ok(())
  }

  fn translate_prototype(&mut self, prototype: &ast::Prototype) -> Result<()> {
    let name = self.interner.lookup_ident(*prototype.pattern.symbolize());

    if name == "main" {
      self.writer.write("$main")?;
    } else {
      self.translate_pattern(&prototype.pattern)?;
    }

    self.translate_inputs(&prototype.inputs)?;
    self.translate_output_ty(&prototype.output)
  }

  fn translate_inputs(&mut self, inputs: &ast::Inputs) -> Result<()> {
    if inputs.is_empty() {
      return Ok(());
    }

    self.writer.write(' ')?;

    for (x, input) in inputs.0.iter().enumerate() {
      self.writer.write_bytes(b"(param ")?;
      self.translate_input(input)?;
      self.writer.write(')')?;

      if x != inputs.len() - 1 {
        self.writer.write(' ')?;
      }
    }

    Ok(())
  }

  fn translate_input(&mut self, input: &ast::Input) -> Result<()> {
    self.translate_pattern(&input.pattern)?;
    self.writer.write(' ')?;
    self.translate_ty(&input.ty)
  }

  fn translate_ty(&mut self, ty: &Ty) -> Result<()> {
    self.writer.write(ty.as_wat())
  }

  fn translate_output_ty(&mut self, output_ty: &ast::OutputTy) -> Result<()> {
    match output_ty {
      ast::OutputTy::Default(_) => Ok(()),
      ast::OutputTy::Ty(ty) => {
        let output_ty = format!(" (result {})", ty.as_wat());

        self.writer.write_bytes(output_ty.as_bytes())
      }
    }
  }

  fn translate_body(&mut self, body: &ast::Block) -> Result<()> {
    for stmt in &body.stmts {
      self.writer.indent();
      self.translate_stmt(stmt)?;
      self.writer.dedent();
    }

    Ok(())
  }

  fn translate_stmt(&mut self, stmt: &ast::Stmt) -> Result<()> {
    match &stmt.kind {
      ast::StmtKind::Var(var) => self.translate_stmt_var(var),
      ast::StmtKind::Item(item) => self.translate_stmt_item(item),
      ast::StmtKind::Expr(expr) => self.translate_stmt_expr(expr),
    }
  }

  fn translate_stmt_var(&mut self, _item: &ast::Var) -> Result<()> {
    Ok(())
  }

  fn translate_stmt_item(&mut self, item: &ast::Item) -> Result<()> {
    self.translate_item(item)
  }

  fn translate_stmt_expr(&mut self, expr: &ast::Expr) -> Result<()> {
    self.translate_expr(expr)
  }

  fn translate_expr(&mut self, expr: &ast::Expr) -> Result<()> {
    match &expr.kind {
      ast::ExprKind::Lit(lit) => self.translate_expr_lit(lit),
      _ => todo!(),
    }
  }

  fn translate_expr_lit(&mut self, lit: &ast::Lit) -> Result<()> {
    match &lit.kind {
      ast::LitKind::Bool(boolean) => self.translate_expr_lit_bool(boolean),
      ast::LitKind::Int(symbol) => self.translate_expr_lit_int(symbol),
      ast::LitKind::Float(symbol) => self.translate_expr_lit_float(symbol),
      ast::LitKind::Char(symbol) => self.translate_expr_lit_char(symbol),
      ast::LitKind::Str(symbol) => self.translate_expr_lit_str(symbol),
      ast::LitKind::Ident(symbol) => self.translate_expr_lit_ident(symbol),
    }
  }

  // todo #1
  fn translate_expr_lit_int(&mut self, symbol: &Symbol) -> Result<()> {
    let int = self.interner.lookup_int(*symbol);

    self.writer.write(format!("(i32.const {int})"))
  }

  // todo #1
  fn translate_expr_lit_float(&mut self, symbol: &Symbol) -> Result<()> {
    let float = self.interner.lookup_float(*symbol);

    self.writer.write(format!("(f64.const {float})"))
  }

  // todo #1
  fn translate_expr_lit_ident(&mut self, symbol: &Symbol) -> Result<()> {
    let ident = self.interner.lookup_ident(*symbol);

    self.writer.write(format!("${ident}"))
  }

  // todo #1
  fn translate_expr_lit_bool(&mut self, boolean: &bool) -> Result<()> {
    let boolean = if *boolean { 1 } else { 0 };

    self.writer.write(format!("(i64.const {boolean})"))
  }

  // todo #1
  fn translate_expr_lit_char(&mut self, symbol: &Symbol) -> Result<()> {
    let ch = self.interner.lookup_char(*symbol);

    self.writer.write(ch)
  }

  // todo #1
  fn translate_expr_lit_str(&mut self, symbol: &Symbol) -> Result<()> {
    let string = self.interner.lookup_str(*symbol);
    let value = format!("(data (i32.const 0) {string})");

    self.writer.write(&value)
  }
}
