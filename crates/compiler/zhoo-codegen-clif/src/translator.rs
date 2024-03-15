#![allow(dead_code)]

use zhoo_ast::ast;

use zo_core::interner::symbol::Symbol;
use zo_core::interner::Interner;
use zo_core::reporter::Reporter;
use zo_core::Result;

use cranelift::frontend::{FunctionBuilder, FunctionBuilderContext};
// use cranelift_codegen::ir::AbiParam;
// use cranelift_codegen::ir::Function;
use cranelift_codegen::{entity::EntityRef, ir::Value, Context};
use cranelift_object::ObjectModule;

// use cranelift_codegen::settings::Configurable;
// use cranelift_codegen::settings::Flags;
// use cranelift_module::Module;
// use cranelift_native::builder;
// use cranelift_object::ObjectBuilder;

pub(crate) struct Translator<'mir> {
  interner: &'mir Interner,
  reporter: &'mir Reporter,
  module: &'mir mut ObjectModule,
  context: &'mir mut Context,
  // builder: &'mir mut FunctionBuilder<'mir>,
}

impl<'mir> Translator<'mir> {
  #[inline]
  pub(crate) fn new(
    interner: &'mir Interner,
    reporter: &'mir Reporter,
    module: &'mir mut ObjectModule,
    context: &'mir mut Context,
    // builder: &'mir mut FunctionBuilder<'mir>,
  ) -> Self {
    Self {
      interner,
      reporter,
      module,
      context,
      // builder: &mut builder,
    }
  }

  pub(crate) fn translate(&mut self, program: &ast::Program) -> Result<Value> {
    let mut function_builder_context = FunctionBuilderContext::new();

    let mut _builder = FunctionBuilder::new(
      &mut self.context.func,
      &mut function_builder_context,
    );

    let mut value = Value::new(0usize);

    for stmt in &program.items {
      value = self.translate_item(stmt)?;
    }

    self.reporter.abort_if_has_errors();

    Ok(value)
  }

  fn translate_item(&mut self, item: &ast::Item) -> Result<Value> {
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

  fn translate_item_var(&mut self, var: &ast::Var) -> Result<Value> {
    self.translate_var(var)
  }

  fn translate_var(&mut self, _var: &ast::Var) -> Result<Value> {
    todo!()
  }

  fn translate_item_ty_alias(
    &mut self,
    _ty_alias: &ast::TyAlias,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_item_ext(&mut self, _ext: &ast::Ext) -> Result<Value> {
    todo!()
  }

  fn translate_item_abstract(
    &mut self,
    _abstr: &ast::Abstract,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_item_fun(&mut self, fun: &ast::Fun) -> Result<Value> {
    let inputs = &fun.prototype.inputs;
    let _signature = &mut self.context.func.signature;

    for _input in &inputs.0 {
      // let clif_type = TypeBuilder::from(&mut self.module, &input.ty);

      // signature.params.push(AbiParam::new(clif_type));
    }

    self.translate_block(&fun.body)?;

    todo!()
  }

  fn translate_block(&mut self, block: &ast::Block) -> Result<Value> {
    let mut value = Value::new(0usize);

    for stmt in &block.stmts {
      value = self.translate_stmt(stmt)?;
    }

    Ok(value)
  }

  fn translate_stmt(&mut self, stmt: &ast::Stmt) -> Result<Value> {
    match &stmt.kind {
      ast::StmtKind::Var(var) => self.translate_stmt_var(var),
      ast::StmtKind::Item(item) => self.translate_stmt_item(item),
      ast::StmtKind::Expr(expr) => self.translate_stmt_expr(expr),
    }
  }

  fn translate_stmt_var(&mut self, var: &ast::Var) -> Result<Value> {
    self.translate_var(var)
  }

  fn translate_stmt_item(&mut self, item: &ast::Item) -> Result<Value> {
    self.translate_item(item)
  }

  fn translate_stmt_expr(&mut self, expr: &ast::Expr) -> Result<Value> {
    self.translate_expr(expr)
  }

  fn translate_expr(&mut self, expr: &ast::Expr) -> Result<Value> {
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
      ast::ExprKind::Call(callee, args) => {
        self.translate_expr_call(callee, args)
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

  fn translate_expr_lit(&mut self, lit: &ast::Lit) -> Result<Value> {
    self.translate_lit(lit)
  }

  fn translate_lit(&mut self, lit: &ast::Lit) -> Result<Value> {
    match &lit.kind {
      ast::LitKind::Int(symbol) => self.translate_lit_int(symbol),
      ast::LitKind::Float(symbol) => self.translate_lit_float(symbol),
      ast::LitKind::Ident(symbol) => self.translate_lit_ident(symbol),
      ast::LitKind::Bool(boolean) => self.translate_lit_bool(boolean),
      ast::LitKind::Char(symbol) => self.translate_lit_char(symbol),
      ast::LitKind::Str(symbol) => self.translate_lit_str(symbol),
    }
  }

  fn translate_lit_int(&mut self, symbol: &Symbol) -> Result<Value> {
    let _int = self.interner.lookup_int(*symbol);

    todo!()
  }

  fn translate_lit_float(&mut self, symbol: &Symbol) -> Result<Value> {
    let _float = self.interner.lookup_float(*symbol);

    todo!()
  }

  fn translate_lit_ident(&mut self, symbol: &Symbol) -> Result<Value> {
    let _ident = self.interner.lookup_ident(*symbol);

    todo!()
  }

  fn translate_lit_bool(&mut self, _boolean: &bool) -> Result<Value> {
    todo!()
  }

  fn translate_lit_char(&mut self, symbol: &Symbol) -> Result<Value> {
    let _char = self.interner.lookup_char(*symbol);

    todo!()
  }

  fn translate_lit_str(&mut self, symbol: &Symbol) -> Result<Value> {
    let _string = self.interner.lookup_str(*symbol);

    todo!()
  }

  fn translate_expr_unop(
    &mut self,
    unop: &ast::UnOp,
    rhs: &ast::Expr,
  ) -> Result<Value> {
    match &unop.kind {
      ast::UnOpKind::Neg => self.translate_expr_unop_neg(rhs),
      ast::UnOpKind::Not => self.translate_expr_unop_not(rhs),
    }
  }

  fn translate_expr_unop_neg(&mut self, rhs: &ast::Expr) -> Result<Value> {
    let _value = self.translate_expr(rhs)?;

    match &rhs.kind {
      ast::ExprKind::Lit(lit) => match &lit.kind {
        ast::LitKind::Int(_) => todo!("{lit}"),
        ast::LitKind::Float(_) => todo!("{lit}"),
        _ => panic!(),
      },
      _ => panic!(),
    }
  }

  fn translate_expr_unop_not(&mut self, rhs: &ast::Expr) -> Result<Value> {
    let _value = self.translate_expr(rhs)?;

    todo!()
  }

  fn translate_expr_binop(
    &mut self,
    binop: &ast::BinOp,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
  ) -> Result<Value> {
    let vlhs = self.translate_expr(lhs)?;
    let vrhs = self.translate_expr(rhs)?;

    match &binop.kind {
      ast::BinOpKind::Add => self.translate_expr_binop_add(vlhs, vrhs),
      ast::BinOpKind::Sub => self.translate_expr_binop_sub(vlhs, vrhs),
      ast::BinOpKind::Mul => self.translate_expr_binop_mul(vlhs, vrhs),
      ast::BinOpKind::Div => self.translate_expr_binop_div(vlhs, vrhs),
      ast::BinOpKind::Rem => self.translate_expr_binop_rem(vlhs, vrhs),
      ast::BinOpKind::Lt => self.translate_expr_binop_lt(vlhs, vrhs),
      ast::BinOpKind::Gt => self.translate_expr_binop_gt(vlhs, vrhs),
      ast::BinOpKind::Le => self.translate_expr_binop_le(vlhs, vrhs),
      ast::BinOpKind::Ge => self.translate_expr_binop_ge(vlhs, vrhs),
      ast::BinOpKind::Eq => self.translate_expr_binop_eq(vlhs, vrhs),
      ast::BinOpKind::Ne => self.translate_expr_binop_ne(vlhs, vrhs),
      ast::BinOpKind::And => self.translate_expr_binop_and(vlhs, vrhs),
      ast::BinOpKind::Or => self.translate_expr_binop_or(vlhs, vrhs),
      ast::BinOpKind::BitAnd => self.translate_expr_binop_bit_and(vlhs, vrhs),
      ast::BinOpKind::BitXor => self.translate_expr_binop_bit_xor(vlhs, vrhs),
      ast::BinOpKind::BitOr => self.translate_expr_binop_bit_or(vlhs, vrhs),
      ast::BinOpKind::Shl => self.translate_expr_binop_shl(vlhs, vrhs),
      ast::BinOpKind::Shr => self.translate_expr_binop_shr(vlhs, vrhs),
    }
  }

  fn translate_expr_binop_add(
    &mut self,
    _lhs: Value,
    _rhs: Value,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_expr_binop_sub(
    &mut self,
    _lhs: Value,
    _rhs: Value,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_expr_binop_mul(
    &mut self,
    _lhs: Value,
    _rhs: Value,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_expr_binop_div(
    &mut self,
    _lhs: Value,
    _rhs: Value,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_expr_binop_rem(
    &mut self,
    _lhs: Value,
    _rhs: Value,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_expr_binop_lt(
    &mut self,
    _lhs: Value,
    _rhs: Value,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_expr_binop_gt(
    &mut self,
    _lhs: Value,
    _rhs: Value,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_expr_binop_le(
    &mut self,
    _lhs: Value,
    _rhs: Value,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_expr_binop_ge(
    &mut self,
    _lhs: Value,
    _rhs: Value,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_expr_binop_eq(
    &mut self,
    _lhs: Value,
    _rhs: Value,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_expr_binop_ne(
    &mut self,
    _lhs: Value,
    _rhs: Value,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_expr_binop_and(
    &mut self,
    _lhs: Value,
    _rhs: Value,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_expr_binop_or(
    &mut self,
    _lhs: Value,
    _rhs: Value,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_expr_binop_bit_and(
    &mut self,
    _lhs: Value,
    _rhs: Value,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_expr_binop_bit_xor(
    &mut self,
    _lhs: Value,
    _rhs: Value,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_expr_binop_bit_or(
    &mut self,
    _lhs: Value,
    _rhs: Value,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_expr_binop_shl(
    &mut self,
    _lhs: Value,
    _rhs: Value,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_expr_binop_shr(
    &mut self,
    _lhs: Value,
    _rhs: Value,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_expr_assign(
    &mut self,
    _lhs: &ast::Expr,
    _rhs: &ast::Expr,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_expr_assignop(
    &mut self,
    _binop: &ast::BinOp,
    _lhs: &ast::Expr,
    _rhs: &ast::Expr,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_expr_call(
    &mut self,
    _lhs: &ast::Expr,
    _rhs: &ast::Args,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_expr_block(&mut self, block: &ast::Block) -> Result<Value> {
    self.translate_block(block)
  }

  fn translate_expr_array(&mut self, _array: &[ast::Expr]) -> Result<Value> {
    todo!()
  }

  fn translate_expr_tuple(&mut self, _tuple: &[ast::Expr]) -> Result<Value> {
    todo!()
  }

  fn translate_expr_array_access(
    &mut self,
    _array: &ast::Expr,
    _access: &ast::Expr,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_expr_tuple_access(
    &mut self,
    _tuple: &ast::Expr,
    _access: &ast::Expr,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_expr_fn(
    &mut self,
    _prototype: &ast::Prototype,
    _body: &ast::Block,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_expr_return(
    &mut self,
    _maybe_expr: &Option<Box<ast::Expr>>,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_expr_if_else(
    &mut self,
    _condition: &ast::Expr,
    _consequence: &ast::Block,
    _maybe_alternative: &Option<Box<ast::Expr>>,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_expr_when(
    &mut self,
    _condition: &ast::Expr,
    _consequence: &ast::Expr,
    _alternative: &ast::Expr,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_expr_match(
    &mut self,
    _condition: &ast::Expr,
    _arms: &[ast::Arm],
  ) -> Result<Value> {
    todo!()
  }

  fn translate_expr_loop(&mut self, _block: &ast::Block) -> Result<Value> {
    todo!()
  }

  fn translate_expr_while(
    &mut self,
    _condition: &ast::Expr,
    _body: &ast::Block,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_expr_for(&mut self, _for_loop: &ast::For) -> Result<Value> {
    todo!()
  }

  fn translate_expr_break(
    &mut self,
    _maybe_expr: &Option<Box<ast::Expr>>,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_expr_continue(&mut self) -> Result<Value> {
    todo!()
  }

  fn translate_expr_var(&mut self, var: &ast::Var) -> Result<Value> {
    self.translate_var(var)
  }

  fn translate_expr_struct_expr(
    &mut self,
    _struct_expr: &ast::StructExpr,
  ) -> Result<Value> {
    todo!()
  }

  fn translate_expr_chaining(
    &mut self,
    _lhs: &ast::Expr,
    _rhs: &ast::Expr,
  ) -> Result<Value> {
    todo!()
  }
}
