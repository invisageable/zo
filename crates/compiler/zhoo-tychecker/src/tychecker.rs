//! ...

#![allow(dead_code)]

use zhoo_ast::ast;
use zhoo_session::session::Session;
use zhoo_ty::ty::{AsTy, Ty, TyKind};

use zo_core::interner::symbol::Symbolize;
use zo_core::interner::Interner;
use zo_core::reporter::report::semantic::Semantic;
use zo_core::reporter::report::ReportError;
use zo_core::reporter::Reporter;
use zo_core::span::Span;
use zo_core::Result;

#[derive(Debug)]
struct Tychecker<'ast> {
  interner: &'ast Interner,
  reporter: &'ast Reporter,
  loops: usize,
  return_ty: Ty,
}

impl<'ast> Tychecker<'ast> {
  #[inline]
  fn new(interner: &'ast Interner, reporter: &'ast Reporter) -> Self {
    Self {
      interner,
      reporter,
      loops: 0usize,
      return_ty: Ty::UNIT,
    }
  }

  #[inline]
  fn ensure(&mut self, expr: &ast::Expr, t1: &Ty) {
    match self.tycheck_expr(expr) {
      Ok(t2) => {
        self.tycheck_eq(t1, &t2);
      }
      Err(error) => panic!("{error}"),
    }
  }

  #[inline]
  fn ensure_unit(&self, t1: &Ty) {
    self.tycheck_eq(t1, &Ty::unit(Span::ZERO));
  }

  #[inline]
  fn ensure_int(&self, t1: &Ty) {
    self.tycheck_eq(t1, &Ty::int(Span::ZERO));
  }

  #[inline]
  fn ensure_float(&self, t1: &Ty) {
    self.tycheck_eq(t1, &Ty::float(Span::ZERO));
  }

  #[inline]
  fn ensure_bool(&self, t1: &Ty) {
    self.tycheck_eq(t1, &Ty::bool(Span::ZERO));
  }

  #[inline]
  fn ensure_char(&self, t1: &Ty) {
    self.tycheck_eq(t1, &Ty::char(Span::ZERO));
  }

  #[inline]
  fn ensure_str(&self, t1: &Ty) {
    self.tycheck_eq(t1, &Ty::str(Span::ZERO));
  }

  #[inline]
  fn tycheck_eq(&self, t1: &Ty, t2: &Ty) -> bool {
    if t1.kind != t2.kind {
      self
        .reporter
        .add_report(ReportError::Semantic(Semantic::TypeMismatch(
          t1.span.to(t2.span),
          t1.to_string(),
          t2.to_string(),
        )));

      return false;
    }

    true
  }

  fn tycheck(&mut self, program: &ast::Program) -> Result<()> {
    for item in &program.items {
      self.tycheck_item(item)?;
    }

    self.reporter.abort_if_has_errors();

    Ok(())
  }

  fn tycheck_item(&mut self, item: &ast::Item) -> Result<()> {
    match &item.kind {
      ast::ItemKind::Load(load) => self.tycheck_item_load(load),
      ast::ItemKind::Pack(pack) => self.tycheck_item_pack(pack),
      ast::ItemKind::Var(var) => self.tycheck_item_var(var),
      ast::ItemKind::TyAlias(ty_alias) => self.tycheck_item_ty_alias(ty_alias),
      ast::ItemKind::Ext(ext) => self.tycheck_item_ext(ext),
      ast::ItemKind::Abstract(abstr) => self.tycheck_item_abstract(abstr),
      ast::ItemKind::Enum(enumeration) => self.tycheck_item_enum(enumeration),
      ast::ItemKind::Struct(structure) => self.tycheck_item_struct(structure),
      ast::ItemKind::Apply(apply) => self.tycheck_item_apply(apply),
      ast::ItemKind::Fun(fun) => self.tycheck_item_fun(fun),
    }
  }

  fn tycheck_item_load(&mut self, _load: &ast::Load) -> Result<()> {
    todo!()
  }

  fn tycheck_item_pack(&mut self, _pack: &ast::Pack) -> Result<()> {
    todo!()
  }

  fn tycheck_item_var(&mut self, var: &ast::Var) -> Result<()> {
    self.tycheck_var(var)
  }

  fn tycheck_var(&mut self, _var: &ast::Var) -> Result<()> {
    // todo:
    // get the type of the pattern expr.
    // get the type of the value expr.
    // check if the scope map already contains the pattern type.
    // if not, register the pattern type into it, if not handle a specific
    // error.

    todo!()
  }

  fn tycheck_item_ty_alias(&mut self, ty_alias: &ast::TyAlias) -> Result<()> {
    match &ty_alias.maybe_ty {
      Some(_ty) => {
        // register the type into a scope map.
        Ok(())
      }
      None => panic!(),
    }
  }

  fn tycheck_item_ext(&mut self, _ext: &ast::Ext) -> Result<()> {
    todo!()
  }

  fn tycheck_item_abstract(&mut self, _abstr: &ast::Abstract) -> Result<()> {
    todo!()
  }

  fn tycheck_item_enum(&mut self, _enumeration: &ast::Enum) -> Result<()> {
    todo!()
  }

  fn tycheck_item_struct(&mut self, _structure: &ast::Struct) -> Result<()> {
    todo!()
  }

  fn tycheck_item_apply(&mut self, _apply: &ast::Apply) -> Result<()> {
    todo!()
  }

  fn tycheck_item_fun(&mut self, fun: &ast::Fun) -> Result<()> {
    self.tycheck_prototype(&fun.prototype)?;
    self.tycheck_block(&fun.body)
  }

  fn tycheck_prototype(&mut self, prototype: &ast::Prototype) -> Result<()> {
    self.tycheck_inputs(&prototype.inputs)?;

    self.return_ty = prototype.as_ty();

    Ok(())
  }

  fn tycheck_inputs(&mut self, inputs: &ast::Inputs) -> Result<()> {
    for input in inputs.iter() {
      self.tycheck_input(input)?;
    }

    Ok(())
  }

  fn tycheck_input(&mut self, _input: &ast::Input) -> Result<()> {
    Ok(())
  }

  fn tycheck_block(&mut self, block: &ast::Block) -> Result<()> {
    for stmt in &block.stmts {
      self.tycheck_stmt(stmt)?;
    }

    Ok(())
  }

  fn tycheck_stmt(&mut self, stmt: &ast::Stmt) -> Result<()> {
    match &stmt.kind {
      ast::StmtKind::Var(var) => self.tycheck_stmt_var(var),
      ast::StmtKind::Item(item) => self.tycheck_stmt_item(item),
      ast::StmtKind::Expr(expr) => self.tycheck_stmt_expr(expr),
    }
  }

  fn tycheck_stmt_var(&mut self, var: &ast::Var) -> Result<()> {
    self.tycheck_var(var)
  }

  fn tycheck_stmt_item(&mut self, item: &ast::Item) -> Result<()> {
    self.tycheck_item(item)?;

    Ok(())
  }

  fn tycheck_stmt_expr(&mut self, expr: &ast::Expr) -> Result<()> {
    self.tycheck_expr(expr)?;

    Ok(())
  }

  fn tycheck_expr(&mut self, expr: &ast::Expr) -> Result<Ty> {
    match &expr.kind {
      ast::ExprKind::Lit(lit) => self.tycheck_expr_lit(lit),
      ast::ExprKind::UnOp(unop, rhs) => self.tycheck_expr_unop(unop, rhs),
      ast::ExprKind::BinOp(binop, lhs, rhs) => {
        self.tycheck_expr_binop(binop, lhs, rhs)
      }
      ast::ExprKind::Assign(lhs, rhs) => self.tycheck_expr_assign(lhs, rhs),
      ast::ExprKind::AssignOp(binop, lhs, rhs) => {
        self.tycheck_expr_assignop(binop, lhs, rhs)
      }
      ast::ExprKind::Array(elmts) => self.tycheck_expr_array(elmts),
      ast::ExprKind::Tuple(elmts) => self.tycheck_expr_tuple(elmts),
      ast::ExprKind::ArrayAccess(array, access) => {
        self.tycheck_expr_array_access(array, access)
      }
      ast::ExprKind::TupleAccess(tuple, access) => {
        self.tycheck_expr_tuple_access(tuple, access)
      }
      ast::ExprKind::Block(block) => self.tycheck_expr_block(block),
      ast::ExprKind::Fn(prototype, body) => {
        self.tycheck_expr_fn(prototype, body)
      }
      ast::ExprKind::Call(callee, args) => self.tycheck_expr_call(callee, args),
      ast::ExprKind::IfElse(condition, conseaquence, maybe_alternative) => {
        self.tycheck_expr_if_else(condition, conseaquence, maybe_alternative)
      }
      ast::ExprKind::When(condition, conseaquence, maybe_alternative) => {
        self.tycheck_expr_when(condition, conseaquence, maybe_alternative)
      }
      ast::ExprKind::Match(condition, arms) => {
        self.tycheck_expr_match(condition, arms)
      }
      ast::ExprKind::Loop(body) => self.tycheck_expr_loop(body),
      ast::ExprKind::While(condition, body) => {
        self.tycheck_expr_while(condition, body)
      }
      ast::ExprKind::For(for_loop) => self.tycheck_expr_for(for_loop),
      ast::ExprKind::Return(maybe_expr) => self.tycheck_expr_return(maybe_expr),
      ast::ExprKind::Break(maybe_expr) => self.tycheck_expr_break(maybe_expr),
      ast::ExprKind::Continue => self.tycheck_expr_continue(),
      ast::ExprKind::Var(var) => self.tycheck_expr_var(var),
      ast::ExprKind::StructExpr(struct_expr) => {
        self.tycheck_expr_struct_expr(struct_expr)
      }
      ast::ExprKind::Chaining(lhs, rhs) => self.tycheck_expr_chaining(lhs, rhs),
    }
  }

  fn tycheck_expr_lit(&mut self, lit: &ast::Lit) -> Result<Ty> {
    match &lit.kind {
      ast::LitKind::Int(_) => self.tycheck_expr_lit_int(lit),
      ast::LitKind::Float(_) => self.tycheck_expr_lit_float(lit),
      ast::LitKind::Ident(_) => self.tycheck_expr_lit_ident(lit),
      ast::LitKind::Bool(_) => self.tycheck_expr_lit_bool(lit),
      ast::LitKind::Char(_) => self.tycheck_expr_lit_char(lit),
      ast::LitKind::Str(_) => self.tycheck_expr_lit_str(lit),
    }
  }

  fn tycheck_expr_lit_int(&mut self, lit: &ast::Lit) -> Result<Ty> {
    Ok(Ty::int(lit.span))
  }

  fn tycheck_expr_lit_float(&mut self, lit: &ast::Lit) -> Result<Ty> {
    Ok(Ty::float(lit.span))
  }

  fn tycheck_expr_lit_ident(&mut self, _lit: &ast::Lit) -> Result<Ty> {
    todo!()
  }

  fn tycheck_expr_lit_bool(&mut self, lit: &ast::Lit) -> Result<Ty> {
    Ok(Ty::bool(lit.span))
  }

  fn tycheck_expr_lit_char(&mut self, lit: &ast::Lit) -> Result<Ty> {
    Ok(Ty::char(lit.span))
  }

  fn tycheck_expr_lit_str(&mut self, lit: &ast::Lit) -> Result<Ty> {
    Ok(Ty::str(lit.span))
  }

  fn tycheck_expr_unop(
    &mut self,
    unop: &ast::UnOp,
    rhs: &ast::Expr,
  ) -> Result<Ty> {
    let ty = self.tycheck_expr(rhs)?;

    match &unop.kind {
      ast::UnOpKind::Neg => self.tycheck_expr_unop_neg(ty),
      ast::UnOpKind::Not => self.tycheck_expr_unop_not(ty),
    }
  }

  fn tycheck_expr_unop_neg(&mut self, ty: Ty) -> Result<Ty> {
    match &ty.kind {
      TyKind::Int | TyKind::Float => Ok(ty),
      _ => panic!(),
    }
  }

  fn tycheck_expr_unop_not(&mut self, ty: Ty) -> Result<Ty> {
    self.ensure_bool(&ty);

    Ok(ty)
  }

  fn tycheck_expr_binop(
    &mut self,
    binop: &ast::BinOp,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
  ) -> Result<Ty> {
    let t1 = self.tycheck_expr(lhs)?;
    let t2 = self.tycheck_expr(rhs)?;

    match &binop.kind {
      ast::BinOpKind::Add => self.tycheck_expr_binop_add(t1, t2),
      ast::BinOpKind::Sub => self.tycheck_expr_binop_sub(t1, t2),
      ast::BinOpKind::Mul => self.tycheck_expr_binop_mul(t1, t2),
      ast::BinOpKind::Div => self.tycheck_expr_binop_div(t1, t2),
      ast::BinOpKind::Rem => self.tycheck_expr_binop_rem(t1, t2),
      ast::BinOpKind::And => self.tycheck_expr_binop_and(t1, t2),
      ast::BinOpKind::Or => self.tycheck_expr_binop_or(t1, t2),
      ast::BinOpKind::BitAnd => self.tycheck_expr_binop_bit_and(t1, t2),
      ast::BinOpKind::BitOr => self.tycheck_expr_binop_bit_or(t1, t2),
      ast::BinOpKind::BitXor => self.tycheck_expr_binop_bit_xor(t1, t2),
      ast::BinOpKind::Lt => self.tycheck_expr_binop_lt(t1, t2),
      ast::BinOpKind::Gt => self.tycheck_expr_binop_gt(t1, t2),
      ast::BinOpKind::Le => self.tycheck_expr_binop_le(t1, t2),
      ast::BinOpKind::Ge => self.tycheck_expr_binop_ge(t1, t2),
      ast::BinOpKind::Eq => self.tycheck_expr_binop_eq(t1, t2),
      ast::BinOpKind::Ne => self.tycheck_expr_binop_ne(t1, t2),
      ast::BinOpKind::Shl => self.tycheck_expr_binop_shl(t1, t2),
      ast::BinOpKind::Shr => self.tycheck_expr_binop_shr(t1, t2),
      _ => todo!(),
    }
  }

  fn tycheck_expr_binop_add(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn tycheck_expr_binop_sub(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn tycheck_expr_binop_mul(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn tycheck_expr_binop_div(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn tycheck_expr_binop_rem(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn tycheck_expr_binop_and(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Bool, TyKind::Bool) => Ok(t1),
      _ => panic!(),
    }
  }

  fn tycheck_expr_binop_or(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Bool, TyKind::Bool) => Ok(t1),
      _ => panic!(),
    }
  }

  fn tycheck_expr_binop_bit_and(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn tycheck_expr_binop_bit_or(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn tycheck_expr_binop_bit_xor(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn tycheck_expr_binop_lt(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn tycheck_expr_binop_gt(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn tycheck_expr_binop_le(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn tycheck_expr_binop_ge(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn tycheck_expr_binop_eq(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    self.tycheck_eq(&t1, &t2);

    Ok(Ty::bool(t1.span.to(t2.span)))
  }

  fn tycheck_expr_binop_ne(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    self.tycheck_eq(&t1, &t2);

    Ok(Ty::bool(t1.span.to(t2.span)))
  }

  fn tycheck_expr_binop_shl(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn tycheck_expr_binop_shr(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  // todo: unfinished, use `symbolise` instead.
  fn tycheck_expr_assign(
    &mut self,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
  ) -> Result<Ty> {
    match &lhs.kind {
      ast::ExprKind::Lit(ast::Lit {
        kind: ast::LitKind::Ident(_symbol),
        ..
      }) => {
        let t1 = self.tycheck_expr(lhs)?;

        self.ensure(rhs, &t1);

        Ok(t1)
      }
      _ => todo!(),
    }
  }

  fn tycheck_expr_assignop(
    &mut self,
    binop: &ast::BinOp,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
  ) -> Result<Ty> {
    let t1 = self.tycheck_expr(lhs)?;
    let t2 = self.tycheck_expr(rhs)?;

    match &binop.kind {
      ast::BinOpKind::Add => self.tycheck_expr_assignop_add(t1, t2),
      ast::BinOpKind::Sub => self.tycheck_expr_assignop_rem(t1, t2),
      ast::BinOpKind::Mul => self.tycheck_expr_assignop_mul(t1, t2),
      ast::BinOpKind::Div => self.tycheck_expr_assignop_div(t1, t2),
      ast::BinOpKind::Rem => self.tycheck_expr_assignop_rem(t1, t2),
      _ => todo!(),
    }
  }

  fn tycheck_expr_assignop_add(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn tycheck_expr_assignop_sub(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn tycheck_expr_assignop_mul(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn tycheck_expr_assignop_div(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn tycheck_expr_assignop_rem(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn tycheck_expr_array(&mut self, _elmts: &[ast::Expr]) -> Result<Ty> {
    todo!()
  }

  fn tycheck_expr_tuple(&mut self, _elmts: &[ast::Expr]) -> Result<Ty> {
    todo!()
  }

  fn tycheck_expr_array_access(
    &mut self,
    _array: &ast::Expr,
    _access: &ast::Expr,
  ) -> Result<Ty> {
    todo!()
  }

  fn tycheck_expr_tuple_access(
    &mut self,
    _tuple: &ast::Expr,
    _access: &ast::Expr,
  ) -> Result<Ty> {
    todo!()
  }

  fn tycheck_expr_block(&mut self, _block: &ast::Block) -> Result<Ty> {
    todo!()
  }

  fn tycheck_expr_fn(
    &mut self,
    _prototype: &ast::Prototype,
    _body: &ast::Block,
  ) -> Result<Ty> {
    todo!()
  }

  fn tycheck_expr_call(
    &mut self,
    callee: &ast::Expr,
    _args: &ast::Args,
  ) -> Result<Ty> {
    let _name = self.interner.lookup_ident(*callee.symbolize());

    todo!()
  }

  fn tycheck_args(&mut self, _args: &ast::Args) -> Result<Ty> {
    todo!()
  }

  fn tycheck_expr_if_else(
    &mut self,
    condition: &ast::Expr,
    consequence: &ast::Block,
    maybe_alternative: &Option<Box<ast::Expr>>,
  ) -> Result<Ty> {
    let t1 = self.tycheck_expr(condition)?;

    self.ensure_bool(&t1);

    let t2 = self.tycheck_expr_block(consequence)?;

    match &maybe_alternative {
      Some(alternative) => {
        self.ensure(&alternative, &t2);

        Ok(t2)
      }
      None => Ok(t2),
    }
  }

  fn tycheck_expr_when(
    &mut self,
    condition: &ast::Expr,
    consequence: &ast::Expr,
    alternative: &ast::Expr,
  ) -> Result<Ty> {
    self.ensure(condition, &Ty::bool(condition.span));

    let t1 = self.tycheck_expr(consequence)?;

    self.ensure(alternative, &t1);

    Ok(t1)
  }

  fn tycheck_expr_match(
    &mut self,
    _condition: &ast::Expr,
    _arms: &[ast::Arm],
  ) -> Result<Ty> {
    todo!()
  }

  fn tycheck_expr_loop(&mut self, body: &ast::Block) -> Result<Ty> {
    self.loops += 1;

    let ty = self.tycheck_expr_block(body)?;

    self.loops -= 1;

    Ok(ty)
  }

  fn tycheck_expr_while(
    &mut self,
    condition: &ast::Expr,
    body: &ast::Block,
  ) -> Result<Ty> {
    self.ensure(condition, &Ty::bool(condition.span));

    self.loops += 1;

    let ty = self.tycheck_expr_block(body)?;

    self.loops -= 1;

    Ok(ty)
  }

  fn tycheck_expr_for(&mut self, for_loop: &ast::For) -> Result<Ty> {
    self.loops += 1;

    let ty = self.tycheck_expr_block(&for_loop.body)?;

    self.loops -= 1;

    Ok(ty)
  }

  fn tycheck_expr_return(
    &mut self,
    maybe_expr: &Option<Box<ast::Expr>>,
  ) -> Result<Ty> {
    match maybe_expr {
      Some(expr) => {
        let ty = self.tycheck_expr(expr)?;

        self.tycheck_eq(&ty, &self.return_ty);

        Ok(ty)
      }
      None => Ok(Ty::UNIT),
    }
  }

  fn tycheck_expr_break(
    &mut self,
    maybe_expr: &Option<Box<ast::Expr>>,
  ) -> Result<Ty> {
    if self.loops == 0 {
      return Err(ReportError::Semantic(Semantic::OutOfLoop("unnamed".into()))); // todo: get the origin expression.
    }

    match maybe_expr {
      Some(expr) => {
        let ty = self.tycheck_expr(expr)?;

        self.tycheck_eq(&ty, &self.return_ty);

        Ok(ty)
      }
      None => Ok(Ty::UNIT),
    }
  }

  fn tycheck_expr_continue(&mut self) -> Result<Ty> {
    if self.loops == 0 {
      return Err(ReportError::Semantic(Semantic::OutOfLoop("unnamed".into()))); // todo: get the origin expression.
    }

    Ok(Ty::UNIT)
  }

  fn tycheck_expr_var(&mut self, var: &ast::Var) -> Result<Ty> {
    self.tycheck_var(var)?;

    Ok(Ty::UNIT)
  }

  fn tycheck_expr_struct_expr(
    &mut self,
    _struct_expr: &ast::StructExpr,
  ) -> Result<Ty> {
    todo!()
  }

  fn tycheck_expr_chaining(
    &mut self,
    _lhs: &ast::Expr,
    _rhs: &ast::Expr,
  ) -> Result<Ty> {
    todo!()
  }
}

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn tycheck(session: &mut Session, program: &ast::Program) -> Result<()> {
  Tychecker::new(&session.interner, &session.reporter).tycheck(program)
}
