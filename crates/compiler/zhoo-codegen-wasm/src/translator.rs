//! ...

use super::wasm::Wat;

use zhoo_ast::ast;
use zhoo_ty::ty::Ty;

use zo_core::interner::symbol::{Symbol, Symbolize};
use zo_core::interner::Interner;
use zo_core::reporter::Reporter;
use zo_core::writer::Writer;
use zo_core::Result;

// todo #1 — the `mir` should have a `Ty`. Normally, this detail must be
// implements from the translateencer side.

pub(crate) struct Translator<'mir> {
  interner: &'mir Interner,
  reporter: &'mir Reporter,
  writer: Writer,
}

impl<'mir> Translator<'mir> {
  #[inline]
  pub(crate) fn new(
    interner: &'mir Interner,
    reporter: &'mir Reporter,
  ) -> Self {
    Self {
      interner,
      reporter,
      writer: Writer::new(),
    }
  }

  #[inline]
  pub(crate) fn output(&mut self) -> Result<Box<[u8]>> {
    Ok(self.writer.as_bytes())
  }

  pub(crate) fn translate(&mut self, program: &ast::Program) -> Result<()> {
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
      ast::ItemKind::TyAlias(ty_alias) => {
        self.translate_item_ty_alias(ty_alias)
      }
      ast::ItemKind::Ext(ext) => self.translate_item_ext(ext),
      ast::ItemKind::Abstract(abstr) => self.translate_item_abstract(abstr),
      ast::ItemKind::Fun(fun) => self.translate_item_fun(fun),
      _ => todo!(),
    }
  }

  fn translate_item_var(&mut self, var: &ast::Var) -> Result<()> {
    self.translate_var(var)
  }

  fn translate_var(&mut self, _var: &ast::Var) -> Result<()> {
    Ok(())
  }

  fn translate_item_ty_alias(
    &mut self,
    _ty_alias: &ast::TyAlias,
  ) -> Result<()> {
    todo!()
  }

  fn translate_item_ext(&mut self, _ext: &ast::Ext) -> Result<()> {
    todo!()
  }

  fn translate_item_abstract(&mut self, _abstr: &ast::Abstract) -> Result<()> {
    todo!()
  }

  fn translate_pattern(&mut self, pattern: &ast::Pattern) -> Result<()> {
    match &pattern.kind {
      ast::PatternKind::Underscore => todo!(),
      ast::PatternKind::Ident(ident) => self.translate_expr(ident),
      ast::PatternKind::Lit(lit) => self.translate_expr_lit(lit),
      ast::PatternKind::MeLower => todo!(),
    }
  }

  fn translate_item_fun(&mut self, fun: &ast::Fun) -> Result<()> {
    self.writer.write_bytes(b"(func")?;
    self.translate_prototype(&fun.prototype)?;
    self.translate_block(&fun.body)?;
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
    self.translate_output_ty(&prototype.output_ty)
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

  fn translate_block(&mut self, body: &ast::Block) -> Result<()> {
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

  fn translate_stmt_var(&mut self, var: &ast::Var) -> Result<()> {
    self.translate_var(var)
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
      ast::ExprKind::UnOp(unop, rhs) => self.translate_expr_unop(unop, rhs),
      ast::ExprKind::BinOp(binop, lhs, rhs) => {
        self.translate_expr_binop(binop, lhs, rhs)
      }
      ast::ExprKind::Assign(lhs, rhs) => self.translate_expr_assign(lhs, rhs),
      ast::ExprKind::AssignOp(binop, lhs, rhs) => {
        self.translate_expr_assignop(binop, lhs, rhs)
      }
      ast::ExprKind::Block(block) => self.translate_expr_block(block),
      ast::ExprKind::Array(exprs) => self.translate_expr_array(exprs),
      ast::ExprKind::Tuple(exprs) => self.translate_expr_tuple(exprs),
      ast::ExprKind::ArrayAccess(array, access) => {
        self.translate_expr_array_access(array, access)
      }
      ast::ExprKind::TupleAccess(array, access) => {
        self.translate_expr_tuple_access(array, access)
      }
      ast::ExprKind::Fn(prototype, body) => {
        self.translate_expr_fn(prototype, body)
      }
      ast::ExprKind::Call(callee, args) => {
        self.translate_expr_call(callee, args)
      }
      ast::ExprKind::Return(maybe_expr) => {
        self.translate_expr_return(maybe_expr)
      }
      ast::ExprKind::IfElse(condition, consequence, maybe_alternative) => {
        self.translate_expr_if_else(condition, consequence, maybe_alternative)
      }
      ast::ExprKind::When(condition, consequence, alternative) => {
        self.translate_expr_when(condition, consequence, alternative)
      }
      ast::ExprKind::Match(condition, arms) => {
        self.translate_expr_match(condition, arms)
      }
      ast::ExprKind::Loop(block) => self.translate_expr_loop(block),
      ast::ExprKind::While(condition, body) => {
        self.translate_expr_while(condition, body)
      }
      ast::ExprKind::For(for_loop) => self.translate_expr_for(for_loop),
      ast::ExprKind::Break(maybe_expr) => self.translate_expr_break(maybe_expr),
      ast::ExprKind::Continue => self.translate_expr_continue(),
      ast::ExprKind::Var(var) => self.translate_expr_var(var),
      ast::ExprKind::StructExpr(struct_expr) => {
        self.translate_expr_struct_expr(struct_expr)
      }
      ast::ExprKind::Chaining(lhs, rhs) => {
        self.translate_expr_chaining(lhs, rhs)
      }
    }
  }

  fn translate_expr_lit(&mut self, lit: &ast::Lit) -> Result<()> {
    match &lit.kind {
      ast::LitKind::Int(symbol) => self.translate_expr_lit_int(symbol),
      ast::LitKind::Float(symbol) => self.translate_expr_lit_float(symbol),
      ast::LitKind::Ident(symbol) => self.translate_expr_lit_ident(symbol),
      ast::LitKind::Bool(boolean) => self.translate_expr_lit_bool(boolean),
      ast::LitKind::Char(symbol) => self.translate_expr_lit_char(symbol),
      ast::LitKind::Str(symbol) => self.translate_expr_lit_str(symbol),
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

  fn translate_expr_lit_ident(&mut self, symbol: &Symbol) -> Result<()> {
    let ident = self.interner.lookup_ident(*symbol);

    self.writer.write(format!("${ident}"))
  }

  // todo #1
  fn translate_expr_lit_bool(&mut self, boolean: &bool) -> Result<()> {
    let boolean = if *boolean { 1 } else { 0 };

    self.writer.write(format!("(i64.const {boolean})"))
  }

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

  fn translate_expr_unop(
    &mut self,
    _unop: &ast::UnOp,
    _rhs: &ast::Expr,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_binop(
    &mut self,
    _binop: &ast::BinOp,
    _lhs: &ast::Expr,
    _rhs: &ast::Expr,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_assign(
    &mut self,
    _lhs: &ast::Expr,
    _rhs: &ast::Expr,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_assignop(
    &mut self,
    _binop: &ast::BinOp,
    _lhs: &ast::Expr,
    _rhs: &ast::Expr,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_block(&mut self, block: &ast::Block) -> Result<()> {
    self.translate_block(block)
  }

  fn translate_expr_call(
    &mut self,
    _lhs: &ast::Expr,
    _rhs: &ast::Args,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_array(&mut self, _array: &[ast::Expr]) -> Result<()> {
    todo!()
  }

  fn translate_expr_tuple(&mut self, _tuple: &[ast::Expr]) -> Result<()> {
    todo!()
  }

  fn translate_expr_array_access(
    &mut self,
    _array: &ast::Expr,
    _access: &ast::Expr,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_tuple_access(
    &mut self,
    _tuple: &ast::Expr,
    _access: &ast::Expr,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_fn(
    &mut self,
    _prototype: &ast::Prototype,
    _body: &ast::Block,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_return(
    &mut self,
    _maybe_expr: &Option<Box<ast::Expr>>,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_if_else(
    &mut self,
    _condition: &ast::Expr,
    _consequence: &ast::Block,
    _maybe_alternative: &Option<Box<ast::Expr>>,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_when(
    &mut self,
    _condition: &ast::Expr,
    _consequence: &ast::Expr,
    _alternative: &ast::Expr,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_match(
    &mut self,
    _condition: &ast::Expr,
    _arms: &[ast::Arm],
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_loop(&mut self, _block: &ast::Block) -> Result<()> {
    todo!()
  }

  fn translate_expr_while(
    &mut self,
    _condition: &ast::Expr,
    _body: &ast::Block,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_for(&mut self, _for_loop: &ast::For) -> Result<()> {
    todo!()
  }

  fn translate_expr_break(
    &mut self,
    _maybe_expr: &Option<Box<ast::Expr>>,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_continue(&mut self) -> Result<()> {
    todo!()
  }

  fn translate_expr_var(&mut self, var: &ast::Var) -> Result<()> {
    self.translate_var(var)
  }

  fn translate_expr_struct_expr(
    &mut self,
    _struct_expr: &ast::StructExpr,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_chaining(
    &mut self,
    _lhs: &ast::Expr,
    _rhs: &ast::Expr,
  ) -> Result<()> {
    todo!()
  }
}
