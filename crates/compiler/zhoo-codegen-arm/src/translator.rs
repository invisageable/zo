#![allow(dead_code)]

use zhoo_ast::ast;

use zo_core::interner::symbol::Symbol;
use zo_core::interner::Interner;
use zo_core::reporter::Reporter;
use zo_core::Result;

pub(crate) struct Translator<'mir> {
  interner: &'mir Interner,
  reporter: &'mir Reporter,
}

impl<'mir> Translator<'mir> {
  #[inline]
  pub fn new(interner: &'mir Interner, reporter: &'mir Reporter) -> Self {
    Self { interner, reporter }
  }

  pub fn translate(&mut self, program: &ast::Program) -> Result<()> {
    for stmt in &program.items {
      self.translate_item(stmt)?;
    }

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
    todo!()
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

  fn translate_item_fun(&mut self, fun: &ast::Fun) -> Result<()> {
    self.translate_block(&fun.body)?;

    todo!()
  }

  fn translate_block(&mut self, block: &ast::Block) -> Result<()> {
    for stmt in &block.stmts {
      self.translate_stmt(stmt)?;
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

  fn translate_expr_lit(&mut self, lit: &ast::Lit) -> Result<()> {
    match &lit.kind {
      ast::LitKind::Int(int) => self.translate_lit_int(int),
      ast::LitKind::Float(float) => self.translate_lit_float(float),
      ast::LitKind::Ident(ident) => self.translate_lit_ident(ident),
      ast::LitKind::Bool(boolean) => self.translate_lit_bool(boolean),
      ast::LitKind::Char(char) => self.translate_lit_char(char),
      ast::LitKind::Str(string) => self.translate_lit_str(string),
    }
  }

  fn translate_lit_int(&mut self, _int: &Symbol) -> Result<()> {
    todo!()
  }

  fn translate_lit_float(&mut self, _float: &Symbol) -> Result<()> {
    todo!()
  }

  fn translate_lit_ident(&mut self, _ident: &Symbol) -> Result<()> {
    todo!()
  }

  fn translate_lit_bool(&mut self, _boolean: &bool) -> Result<()> {
    todo!()
  }

  fn translate_lit_char(&mut self, _char: &Symbol) -> Result<()> {
    todo!()
  }

  fn translate_lit_str(&mut self, _str: &Symbol) -> Result<()> {
    todo!()
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

  fn infer_expr_return(
    &mut self,
    _maybe_expr: &Option<Box<ast::Expr>>,
  ) -> Result<()> {
    todo!()
  }

  fn infer_expr_if_else(
    &mut self,
    _condition: &ast::Expr,
    _consequence: &ast::Block,
    _maybe_alternative: &Option<Box<ast::Expr>>,
  ) -> Result<()> {
    todo!()
  }

  fn infer_expr_when(
    &mut self,
    _condition: &ast::Expr,
    _consequence: &ast::Expr,
    _maybe_alternative: &ast::Expr,
  ) -> Result<()> {
    todo!()
  }

  fn infer_expr_match(
    &mut self,
    _condition: &ast::Expr,
    _arms: &[ast::Arm],
  ) -> Result<()> {
    todo!()
  }

  fn infer_expr_loop(&mut self, _block: &ast::Block) -> Result<()> {
    todo!()
  }

  fn infer_expr_while(
    &mut self,
    _condition: &ast::Expr,
    _body: &ast::Block,
  ) -> Result<()> {
    todo!()
  }

  fn infer_expr_for(&mut self, _for_loop: &ast::For) -> Result<()> {
    todo!()
  }

  fn infer_expr_break(
    &mut self,
    _maybe_expr: &Option<Box<ast::Expr>>,
  ) -> Result<()> {
    todo!()
  }

  fn infer_expr_continue(&mut self) -> Result<()> {
    todo!()
  }

  fn infer_expr_var(&mut self, var: &ast::Var) -> Result<()> {
    self.translate_var(var)
  }

  fn infer_expr_struct_expr(
    &mut self,
    _struct_expr: &ast::StructExpr,
  ) -> Result<()> {
    todo!()
  }

  fn infer_expr_chaining(
    &mut self,
    _lhs: &ast::Expr,
    _rhs: &ast::Expr,
  ) -> Result<()> {
    todo!()
  }
}
