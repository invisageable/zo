use super::py::{AsBuiltin, AsOp};

use zhoo_ast::ast;

use zo_core::interner::symbol::Symbol;
use zo_core::interner::symbol::Symbolize;
use zo_core::interner::Interner;
use zo_core::reporter::Reporter;
use zo_core::writer::Writer;
use zo_core::{to, Result};

const PY_HEADER: &[u8] = b"#!/usr/bin/python\n# -*- coding: utf-8 -*-\n";
const PY_ENTRY: &[u8] = b"if __name__ == \"__main__\":\n  main()";

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
    self.writer.write_bytes(PY_HEADER)?;

    for stmt in &program.items {
      self.writer.new_line()?;
      self.translate_item(stmt)?;
      self.writer.new_line()?;
    }

    self.writer.new_line()?;
    self.writer.write_bytes(PY_ENTRY)?;
    self.writer.new_line()?;
    self.reporter.abort_if_has_errors();

    Ok(())
  }

  /// ## syntax.
  ///
  /// `<item>`.
  fn translate_item(&mut self, item: &ast::Item) -> Result<()> {
    match &item.kind {
      ast::ItemKind::Var(var) => self.translate_item_var(var),
      ast::ItemKind::TyAlias(ty_alias) => {
        self.translate_item_ty_alias(ty_alias)
      }
      ast::ItemKind::Ext(ext) => self.translate_item_ext(ext),
      ast::ItemKind::Fun(fun) => self.translate_item_fun(fun),
      _ => todo!(),
    }
  }

  /// ## notes.
  ///
  /// @see [`Translator::translate_var`].
  fn translate_item_var(&mut self, var: &ast::Var) -> Result<()> {
    self.translate_var(var)
  }

  /// ## syntax.
  ///
  /// `<expr> = <expr>`.
  fn translate_var(&mut self, var: &ast::Var) -> Result<()> {
    self.translate_pattern(&var.pattern)?;
    self.writer.space()?;
    self.writer.write_bytes(b"=")?;
    self.writer.space()?;
    self.translate_expr(&var.value)
  }

  /// ## syntax.
  ///
  /// `<pattern>`.
  fn translate_pattern(&mut self, pattern: &ast::Pattern) -> Result<()> {
    match &pattern.kind {
      ast::PatternKind::Ident(ident) => {
        let symbol = *ident.symbolize();
        let ident = self.interner.lookup_ident(symbol);

        self.writer.write(ident)
      }
      _ => todo!(),
    }
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

  /// ## syntax.
  ///
  /// `def <prototype> <block>`.
  fn translate_item_fun(&mut self, fun: &ast::Fun) -> Result<()> {
    self.writer.write_bytes(b"def")?;
    self.writer.space()?;
    self.translate_prototype(&fun.prototype)?;
    self.translate_block(&fun.body)
  }

  /// ## syntax.
  ///
  /// `<pattern> <inputs> :`.
  fn translate_prototype(&mut self, prototype: &ast::Prototype) -> Result<()> {
    self.translate_pattern(&prototype.pattern)?;
    self.translate_inputs(&prototype.inputs)?;
    self.writer.write_bytes(b":")
  }

  /// ## syntax.
  ///
  /// `( <input*> )`.
  fn translate_inputs(&mut self, inputs: &ast::Inputs) -> Result<()> {
    self.writer.write_bytes(b"(")?;

    for input in inputs.iter() {
      self.translate_input(input)?;
    }

    self.writer.write_bytes(b")")
  }

  /// ## syntax.
  ///
  /// `( <input:pattern> )`.
  fn translate_input(&mut self, input: &ast::Input) -> Result<()> {
    self.translate_pattern(&input.pattern)
  }

  /// ## syntax.
  ///
  /// `{ <stmt*> }`.
  /// `{ <new_line> <indent> <stmt*> <dedent> }`.
  fn translate_block(&mut self, block: &ast::Block) -> Result<()> {
    self.writer.new_line()?;

    for (idx, stmt) in block.stmts.iter().enumerate() {
      self.writer.indent();
      self.translate_stmt(stmt)?;
      self.writer.dedent();

      if idx < block.stmts.len() - 1 {
        self.writer.new_line()?;
      }
    }

    Ok(())
  }

  /// ## syntax.
  ///
  /// `<stmt>`.
  fn translate_stmt(&mut self, stmt: &ast::Stmt) -> Result<()> {
    match &stmt.kind {
      ast::StmtKind::Var(var) => self.translate_stmt_var(var),
      ast::StmtKind::Item(item) => self.translate_stmt_item(item),
      ast::StmtKind::Expr(expr) => self.translate_stmt_expr(expr),
    }
  }

  /// ## notes.
  ///
  /// @see [`Translator::translate_var`].
  fn translate_stmt_var(&mut self, var: &ast::Var) -> Result<()> {
    self.translate_var(var)
  }

  /// ## notes.
  ///
  /// @see [`Translator::translate_item`].
  fn translate_stmt_item(&mut self, item: &ast::Item) -> Result<()> {
    self.translate_item(item)
  }

  /// ## notes.
  ///
  /// @see [`Translator::translate_expr`].
  fn translate_stmt_expr(&mut self, expr: &ast::Expr) -> Result<()> {
    self.translate_expr(expr)
  }

  /// ## syntax.
  ///
  /// `<expr>`.
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

  /// ## syntax.
  ///
  /// `<lit>`.
  fn translate_expr_lit(&mut self, lit: &ast::Lit) -> Result<()> {
    match &lit.kind {
      ast::LitKind::Int(symbol) => self.translate_lit_int(symbol),
      ast::LitKind::Float(symbol) => self.translate_lit_float(symbol),
      ast::LitKind::Ident(symbol) => self.translate_lit_ident(symbol),
      ast::LitKind::Bool(boolean) => self.translate_lit_bool(boolean),
      ast::LitKind::Char(symbol) => self.translate_lit_char(symbol),
      ast::LitKind::Str(symbol) => self.translate_lit_str(symbol),
    }
  }

  /// ## syntax.
  ///
  /// `[0-9_]`.
  fn translate_lit_int(&mut self, symbol: &Symbol) -> Result<()> {
    let int = self.interner.lookup_int(*symbol);

    self.writer.write_int(int)
  }

  /// ## syntax.
  ///
  /// `[0-9.0-9]`.
  fn translate_lit_float(&mut self, symbol: &Symbol) -> Result<()> {
    let float = self.interner.lookup_float(*symbol);

    self.writer.write_float(float)
  }

  /// ## syntax.
  ///
  /// `[a-z_A-Z]`.
  fn translate_lit_ident(&mut self, symbol: &Symbol) -> Result<()> {
    let ident = self.interner.lookup_ident(*symbol);

    self.writer.dedent();
    self.writer.write(ident)
  }

  /// ## syntax.
  ///
  /// `[false|true]`.
  fn translate_lit_bool(&mut self, boolean: &bool) -> Result<()> {
    let boolean = to!(pascal format!("{boolean}"));

    self.writer.write(boolean)
  }
  /// ## syntax.
  ///
  /// `' '`.
  fn translate_lit_char(&mut self, symbol: &Symbol) -> Result<()> {
    let ch = self.interner.lookup_char(*symbol);

    self.writer.write(ch)
  }

  /// ## syntax.
  ///
  /// `" "`
  fn translate_lit_str(&mut self, symbol: &Symbol) -> Result<()> {
    let string = self.interner.lookup_str(*symbol);

    self.writer.write(string)
  }

  /// ## syntax.
  ///
  /// `<unop> <expr>`.
  fn translate_expr_unop(
    &mut self,
    unop: &ast::UnOp,
    rhs: &ast::Expr,
  ) -> Result<()> {
    self.writer.write_bytes(unop.as_op().as_bytes())?;

    match &unop.kind {
      ast::UnOpKind::Not => self.writer.space(),
      ast::UnOpKind::Neg => Ok(()),
    }?;

    self.translate_expr(rhs)
  }

  /// ## syntax.
  ///
  /// `<expr> <binop> <expr>`.
  fn translate_expr_binop(
    &mut self,
    binop: &ast::BinOp,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
  ) -> Result<()> {
    self.translate_expr(lhs)?;
    self.writer.space()?;
    self.writer.write_bytes(binop.as_op().as_bytes())?;
    self.writer.space()?;
    self.translate_expr(rhs)
  }

  /// ## syntax.
  ///
  /// `<expr> = <expr>`.
  fn translate_expr_assign(
    &mut self,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
  ) -> Result<()> {
    self.translate_expr(lhs)?;
    self.writer.space()?;
    self.writer.write_bytes(b"=")?;
    self.writer.space()?;
    self.translate_expr(rhs)
  }

  /// ## syntax.
  ///
  /// `<expr> <binop> <expr>`.
  fn translate_expr_assignop(
    &mut self,
    binop: &ast::BinOp,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
  ) -> Result<()> {
    self.translate_expr(lhs)?;
    self.writer.space()?;
    self.writer.write_bytes(binop.as_op().as_bytes())?;
    self.writer.space()?;
    self.translate_expr(rhs)
  }

  /// ## notes.
  ///
  /// @see [`Translator::translate_expr_block`].
  fn translate_expr_block(&mut self, block: &ast::Block) -> Result<()> {
    self.translate_block(block)
  }

  /// ## syntax.
  ///
  /// `<expr> ( <args> )`.
  fn translate_expr_call(
    &mut self,
    callee: &ast::Expr,
    args: &ast::Args,
  ) -> Result<()> {
    let ident = self.interner.lookup_ident(*callee.symbolize());
    let ident = ident.as_builtin();

    self.writer.write(ident)?;
    self.writer.write_bytes(b"(")?;
    self.translate_args(args)?;
    self.writer.write_bytes(b")")
  }

  /// ## syntax.
  ///
  /// `<arg*> ,?`.
  fn translate_args(&mut self, args: &ast::Args) -> Result<()> {
    for (idx, arg) in args.iter().enumerate() {
      self.translate_pattern(&arg.pattern)?;

      if idx < args.len() - 1 {
        self.writer.comma()?;
        self.writer.space()?;
      }
    }

    Ok(())
  }

  /// ## syntax.
  ///
  /// `[ <expr> , ]`.
  fn translate_expr_array(&mut self, array: &[ast::Expr]) -> Result<()> {
    self.writer.write_bytes(b"[")?;

    for expr in array {
      self.translate_expr(expr)?;
    }

    self.writer.write_bytes(b"]")
  }

  /// ## syntax.
  ///
  /// `...`
  fn translate_expr_tuple(&mut self, _tuple: &[ast::Expr]) -> Result<()> {
    todo!()
  }

  /// ## syntax.
  ///
  /// `<expr> [ <expr> ]`.
  fn translate_expr_array_access(
    &mut self,
    array: &ast::Expr,
    access: &ast::Expr,
  ) -> Result<()> {
    self.translate_expr(array)?;
    self.writer.write_bytes(b"[")?;
    self.translate_expr(access)?;
    self.writer.write_bytes(b"]")
  }

  /// ## syntax.
  ///
  /// `...`
  fn translate_expr_tuple_access(
    &mut self,
    _tuple: &ast::Expr,
    _access: &ast::Expr,
  ) -> Result<()> {
    todo!()
  }

  /// ## syntax.
  ///
  /// `lambda <expr>`.
  fn translate_expr_fn(
    &mut self,
    prototype: &ast::Prototype,
    body: &ast::Block,
  ) -> Result<()> {
    self.writer.write_bytes(b"lambda")?;
    self.writer.space()?;
    self.translate_inputs(&prototype.inputs)?;
    self.writer.colon()?;
    self.translate_block(body)
  }

  /// ## syntax.
  ///
  /// `return <expr>`.
  fn translate_expr_return(
    &mut self,
    maybe_expr: &Option<Box<ast::Expr>>,
  ) -> Result<()> {
    self.writer.write_bytes(b"return")?;

    match maybe_expr {
      Some(expr) => {
        self.writer.space()?;
        self.translate_expr(expr)
      }
      None => Ok(()),
    }
  }

  /// ## syntax.
  ///
  /// `if <expr>: <new_line> <indent> <block> <dedent> <new_line>`.
  /// `if <expr>: <new_line> <indent> <block> <dedent> <new_line> else:
  /// <new_line> <indent> <expr> <dedent> <new_line>`.
  fn translate_expr_if_else(
    &mut self,
    condition: &ast::Expr,
    consequence: &ast::Block,
    maybe_alternative: &Option<Box<ast::Expr>>,
  ) -> Result<()> {
    self.writer.write_bytes(b"if")?;
    self.writer.space()?;
    self.translate_expr(condition)?;
    self.writer.colon()?;

    match maybe_alternative {
      Some(expr) => {
        self.writer.write_bytes(b"else")?;
        self.writer.space()?;
        self.translate_expr(expr)
      }
      None => self.translate_block(consequence),
    }
  }

  /// ## syntax.
  ///
  /// `if <expr> else <expr>`.
  fn translate_expr_when(
    &mut self,
    condition: &ast::Expr,
    consequence: &ast::Expr,
    alternative: &ast::Expr,
  ) -> Result<()> {
    self.writer.write_bytes(b"if")?;
    self.writer.space()?;
    self.translate_expr(condition)?;
    self.writer.space()?;
    self.translate_expr(consequence)?;
    self.writer.space()?;
    self.writer.write_bytes(b"else")?;
    self.writer.space()?;
    self.translate_expr(alternative)
  }

  /// ## syntax.
  ///
  /// `...`
  fn translate_expr_match(
    &mut self,
    _condition: &ast::Expr,
    _arms: &[ast::Arm],
  ) -> Result<()> {
    todo!()
  }

  /// ## syntax.
  ///
  /// `while True: <new_line> <indent> <block> <new_line> <indent>`.
  fn translate_expr_loop(&mut self, block: &ast::Block) -> Result<()> {
    self.writer.write_bytes(b"while True:")?;
    self.translate_block(block)
  }

  /// ## syntax.
  ///
  /// `while <expr> : <block> <block?>`.
  fn translate_expr_while(
    &mut self,
    condition: &ast::Expr,
    body: &ast::Block,
  ) -> Result<()> {
    self.writer.write_bytes(b"while")?;
    self.writer.space()?;
    self.translate_expr(condition)?;
    self.writer.write_bytes(b":")?;
    self.translate_block(body)
  }

  /// ## syntax.
  ///
  /// `...`
  fn translate_expr_for(&mut self, _for_loop: &ast::For) -> Result<()> {
    todo!()
  }

  /// ## syntax.
  ///
  /// `break <expr?>`.
  fn translate_expr_break(
    &mut self,
    maybe_expr: &Option<Box<ast::Expr>>,
  ) -> Result<()> {
    self.writer.write_bytes(b"break")?;

    match maybe_expr {
      Some(expr) => {
        self.writer.space()?;
        self.translate_expr(expr)
      }
      None => Ok(()),
    }
  }

  /// ## syntax.
  ///
  /// `continue`.
  fn translate_expr_continue(&mut self) -> Result<()> {
    self.writer.write_bytes(b"continue")
  }

  /// ## notes.
  ///
  /// @see [`Translator::translate_var`].
  fn translate_expr_var(&mut self, var: &ast::Var) -> Result<()> {
    self.translate_var(var)
  }

  /// ## syntax.
  ///
  /// `...`
  fn translate_expr_struct_expr(
    &mut self,
    _struct_expr: &ast::StructExpr,
  ) -> Result<()> {
    todo!()
  }

  /// ## syntax.
  ///
  /// `...`
  fn translate_expr_chaining(
    &mut self,
    _lhs: &ast::Expr,
    _rhs: &ast::Expr,
  ) -> Result<()> {
    todo!()
  }
}
