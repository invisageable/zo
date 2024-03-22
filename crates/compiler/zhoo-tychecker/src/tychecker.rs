//! ...

#![allow(dead_code)]

use zhoo_ast::ast;
use zhoo_session::session::Session;
use zhoo_ty::ty::{Ty, TyKind};

use zo_core::reporter::report::semantic::Semantic;
use zo_core::reporter::report::ReportError;
use zo_core::reporter::Reporter;
use zo_core::span::Span;
use zo_core::Result;

#[derive(Debug)]
struct Tychecker<'ast> {
  reporter: &'ast Reporter,
}

impl<'ast> Tychecker<'ast> {
  #[inline]
  fn new(reporter: &'ast Reporter) -> Self {
    Self { reporter }
  }

  #[inline]
  fn ensure(&mut self, expr: &ast::Expr, t1: &Ty) {
    match self.check_expr(expr) {
      Ok(t2) => {
        self.check_eq(t1, &t2);
      }
      Err(error) => panic!("{error}"),
    }
  }

  #[inline]
  fn ensure_unit(&self, t1: &Ty) {
    self.check_eq(t1, &Ty::unit(Span::ZERO));
  }

  #[inline]
  fn ensure_int(&self, t1: &Ty) {
    self.check_eq(t1, &Ty::int(Span::ZERO));
  }

  #[inline]
  fn ensure_float(&self, t1: &Ty) {
    self.check_eq(t1, &Ty::float(Span::ZERO));
  }

  #[inline]
  fn ensure_bool(&self, t1: &Ty) {
    self.check_eq(t1, &Ty::bool(Span::ZERO));
  }

  #[inline]
  fn ensure_char(&self, t1: &Ty) {
    self.check_eq(t1, &Ty::char(Span::ZERO));
  }

  #[inline]
  fn ensure_str(&self, t1: &Ty) {
    self.check_eq(t1, &Ty::str(Span::ZERO));
  }

  #[inline]
  fn check_eq(&self, t1: &Ty, t2: &Ty) -> bool {
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

  fn check(&mut self, program: &ast::Program) -> Result<()> {
    for item in &program.items {
      self.check_item(item)?;
    }

    self.reporter.abort_if_has_errors();

    Ok(())
  }

  fn check_item(&mut self, item: &ast::Item) -> Result<()> {
    match &item.kind {
      ast::ItemKind::Load(load) => self.check_item_load(load),
      ast::ItemKind::Pack(pack) => self.check_item_pack(pack),
      ast::ItemKind::Var(var) => self.check_item_var(var),
      ast::ItemKind::TyAlias(ty_alias) => self.check_item_ty_alias(ty_alias),
      ast::ItemKind::Ext(ext) => self.check_item_ext(ext),
      ast::ItemKind::Abstract(abstr) => self.check_item_abstract(abstr),
      ast::ItemKind::Enum(enumeration) => self.check_item_enum(enumeration),
      ast::ItemKind::Struct(structure) => self.check_item_struct(structure),
      ast::ItemKind::Apply(apply) => self.check_item_apply(apply),
      ast::ItemKind::Fun(fun) => self.check_item_fun(fun),
    }
  }

  fn check_item_load(&mut self, _load: &ast::Load) -> Result<()> {
    todo!()
  }

  fn check_item_pack(&mut self, _pack: &ast::Pack) -> Result<()> {
    todo!()
  }

  fn check_item_var(&mut self, var: &ast::Var) -> Result<()> {
    self.check_var(var)
  }

  fn check_var(&mut self, _var: &ast::Var) -> Result<()> {
    // todo:
    // get the type of the pattern expr.
    // get the type of the value expr.
    // check if the scope map already contains the pattern type.
    // if not, register the pattern type into it, if not handle a specific
    // error.

    todo!()
  }

  fn check_item_ty_alias(&mut self, ty_alias: &ast::TyAlias) -> Result<()> {
    match &ty_alias.maybe_ty {
      Some(_ty) => {
        // register the type into a scope map.
        Ok(())
      }
      None => panic!(),
    }
  }

  fn check_item_ext(&mut self, _ext: &ast::Ext) -> Result<()> {
    todo!()
  }

  fn check_item_abstract(&mut self, _abstr: &ast::Abstract) -> Result<()> {
    todo!()
  }

  fn check_item_enum(&mut self, _enumeration: &ast::Enum) -> Result<()> {
    todo!()
  }

  fn check_item_struct(&mut self, _structure: &ast::Struct) -> Result<()> {
    todo!()
  }

  fn check_item_apply(&mut self, _apply: &ast::Apply) -> Result<()> {
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
      ast::ExprKind::UnOp(unop, rhs) => self.check_expr_unop(unop, rhs),
      ast::ExprKind::BinOp(binop, lhs, rhs) => {
        self.check_expr_binop(binop, lhs, rhs)
      }
      ast::ExprKind::Assign(lhs, rhs) => self.check_expr_assign(lhs, rhs),
      ast::ExprKind::AssignOp(binop, lhs, rhs) => {
        self.check_expr_assignop(binop, lhs, rhs)
      }
      ast::ExprKind::Array(elmts) => self.check_expr_array(elmts),
      ast::ExprKind::Tuple(elmts) => self.check_expr_tuple(elmts),
      ast::ExprKind::ArrayAccess(array, access) => {
        self.check_expr_array_access(array, access)
      }
      ast::ExprKind::TupleAccess(tuple, access) => {
        self.check_expr_tuple_access(tuple, access)
      }
      ast::ExprKind::Block(block) => self.check_expr_block(block),
      ast::ExprKind::Fn(prototype, body) => self.check_expr_fn(prototype, body),
      ast::ExprKind::Call(callee, args) => self.check_expr_call(callee, args),
      ast::ExprKind::IfElse(condition, conseaquence, maybe_alternative) => {
        self.check_expr_if_else(condition, conseaquence, maybe_alternative)
      }
      ast::ExprKind::When(condition, conseaquence, maybe_alternative) => {
        self.check_expr_when(condition, conseaquence, maybe_alternative)
      }
      ast::ExprKind::Match(condition, arms) => {
        self.check_expr_match(condition, arms)
      }
      ast::ExprKind::Loop(body) => self.check_expr_loop(body),
      ast::ExprKind::While(condition, body) => {
        self.check_expr_while(condition, body)
      }
      ast::ExprKind::For(for_loop) => self.check_expr_for(for_loop),
      ast::ExprKind::Return(maybe_expr) => self.check_expr_return(maybe_expr),
      ast::ExprKind::Break(maybe_expr) => self.check_expr_break(maybe_expr),
      ast::ExprKind::Continue => self.check_expr_continue(),
      ast::ExprKind::Var(var) => self.check_expr_var(var),
      ast::ExprKind::StructExpr(struct_expr) => {
        self.check_expr_struct_expr(struct_expr)
      }
      ast::ExprKind::Chaining(lhs, rhs) => self.check_expr_chaining(lhs, rhs),
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

  fn check_expr_unop(
    &mut self,
    unop: &ast::UnOp,
    rhs: &ast::Expr,
  ) -> Result<Ty> {
    let ty = self.check_expr(rhs)?;

    match &unop.kind {
      ast::UnOpKind::Neg => self.check_expr_unop_neg(ty),
      ast::UnOpKind::Not => self.check_expr_unop_not(ty),
    }
  }

  fn check_expr_unop_neg(&mut self, ty: Ty) -> Result<Ty> {
    match &ty.kind {
      TyKind::Int | TyKind::Float => Ok(ty),
      _ => panic!(),
    }
  }

  fn check_expr_unop_not(&mut self, ty: Ty) -> Result<Ty> {
    self.ensure_bool(&ty);

    Ok(ty)
  }

  fn check_expr_binop(
    &mut self,
    binop: &ast::BinOp,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
  ) -> Result<Ty> {
    let t1 = self.check_expr(lhs)?;
    let t2 = self.check_expr(rhs)?;

    match &binop.kind {
      ast::BinOpKind::Add => self.check_expr_binop_add(t1, t2),
      ast::BinOpKind::Sub => self.check_expr_binop_sub(t1, t2),
      ast::BinOpKind::Mul => self.check_expr_binop_mul(t1, t2),
      ast::BinOpKind::Div => self.check_expr_binop_div(t1, t2),
      ast::BinOpKind::Rem => self.check_expr_binop_rem(t1, t2),
      ast::BinOpKind::And => self.check_expr_binop_and(t1, t2),
      ast::BinOpKind::Or => self.check_expr_binop_or(t1, t2),
      ast::BinOpKind::BitAnd => self.check_expr_binop_bit_and(t1, t2),
      ast::BinOpKind::BitOr => self.check_expr_binop_bit_or(t1, t2),
      ast::BinOpKind::BitXor => self.check_expr_binop_bit_xor(t1, t2),
      ast::BinOpKind::Lt => self.check_expr_binop_lt(t1, t2),
      ast::BinOpKind::Gt => self.check_expr_binop_gt(t1, t2),
      ast::BinOpKind::Le => self.check_expr_binop_le(t1, t2),
      ast::BinOpKind::Ge => self.check_expr_binop_ge(t1, t2),
      ast::BinOpKind::Eq => self.check_expr_binop_eq(t1, t2),
      ast::BinOpKind::Ne => self.check_expr_binop_ne(t1, t2),
      ast::BinOpKind::Shl => self.check_expr_binop_shl(t1, t2),
      ast::BinOpKind::Shr => self.check_expr_binop_shr(t1, t2),
      _ => todo!(),
    }
  }

  fn check_expr_binop_add(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn check_expr_binop_sub(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn check_expr_binop_mul(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn check_expr_binop_div(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn check_expr_binop_rem(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn check_expr_binop_and(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Bool, TyKind::Bool) => Ok(t1),
      _ => panic!(),
    }
  }

  fn check_expr_binop_or(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Bool, TyKind::Bool) => Ok(t1),
      _ => panic!(),
    }
  }

  fn check_expr_binop_bit_and(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn check_expr_binop_bit_or(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn check_expr_binop_bit_xor(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn check_expr_binop_lt(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn check_expr_binop_gt(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn check_expr_binop_le(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn check_expr_binop_ge(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn check_expr_binop_eq(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    self.check_eq(&t1, &t2);

    Ok(Ty::bool(t1.span.to(t2.span)))
  }

  fn check_expr_binop_ne(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    self.check_eq(&t1, &t2);

    Ok(Ty::bool(t1.span.to(t2.span)))
  }

  fn check_expr_binop_shl(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn check_expr_binop_shr(&mut self, t1: Ty, t2: Ty) -> Result<Ty> {
    match (&t1.kind, &t2.kind) {
      (TyKind::Int, TyKind::Int) => Ok(t1),
      (TyKind::Float, TyKind::Float) => Ok(t1),
      _ => panic!(),
    }
  }

  fn check_expr_assign(
    &mut self,
    _lhs: &ast::Expr,
    _rhs: &ast::Expr,
  ) -> Result<Ty> {
    todo!()
  }

  fn check_expr_assignop(
    &mut self,
    _binop: &ast::BinOp,
    _lhs: &ast::Expr,
    _rhs: &ast::Expr,
  ) -> Result<Ty> {
    todo!()
  }

  fn check_expr_array(&mut self, _elmts: &[ast::Expr]) -> Result<Ty> {
    todo!()
  }

  fn check_expr_tuple(&mut self, _elmts: &[ast::Expr]) -> Result<Ty> {
    todo!()
  }

  fn check_expr_array_access(
    &mut self,
    _array: &ast::Expr,
    _access: &ast::Expr,
  ) -> Result<Ty> {
    todo!()
  }

  fn check_expr_tuple_access(
    &mut self,
    _tuple: &ast::Expr,
    _access: &ast::Expr,
  ) -> Result<Ty> {
    todo!()
  }

  fn check_expr_block(&mut self, _block: &ast::Block) -> Result<Ty> {
    todo!()
  }

  fn check_expr_fn(
    &mut self,
    _prototype: &ast::Prototype,
    _body: &ast::Block,
  ) -> Result<Ty> {
    todo!()
  }

  fn check_expr_call(
    &mut self,
    _callee: &ast::Expr,
    _args: &ast::Args,
  ) -> Result<Ty> {
    todo!()
  }

  fn check_expr_if_else(
    &mut self,
    _condition: &ast::Expr,
    _consequence: &ast::Block,
    _maybe_alternative: &Option<Box<ast::Expr>>,
  ) -> Result<Ty> {
    // todo:
    // gets the type of condition (t1).
    // check if the condition type is a boolean type.
    // type check consequence (t2) and maybe_alternative (t3).
    // check if t2 and t3 has the same type, if yes, return t2.

    todo!()
  }

  fn check_expr_when(
    &mut self,
    _condition: &ast::Expr,
    _consequence: &ast::Expr,
    _alternative: &ast::Expr,
  ) -> Result<Ty> {
    todo!()
  }

  fn check_expr_match(
    &mut self,
    _condition: &ast::Expr,
    _arms: &[ast::Arm],
  ) -> Result<Ty> {
    todo!()
  }

  fn check_expr_loop(&mut self, _body: &ast::Block) -> Result<Ty> {
    todo!()
  }

  fn check_expr_while(
    &mut self,
    _condition: &ast::Expr,
    _body: &ast::Block,
  ) -> Result<Ty> {
    // todo:
    // get condition type.
    // ensure bool, pass condition type.
    // we need a loop counter to increment it before doing any check of the body
    // and decrement it after.

    todo!()
  }

  fn check_expr_for(&mut self, _for_loop: &ast::For) -> Result<Ty> {
    todo!()
  }

  fn check_expr_return(
    &mut self,
    _maybe_expr: &Option<Box<ast::Expr>>,
  ) -> Result<Ty> {
    // todo:
    // we need to keep trace of a return from function, then we can gets this
    // value and compare it with the maybe_expr and our return ty in memory.
    todo!()
  }

  fn check_expr_break(
    &mut self,
    _maybe_expr: &Option<Box<ast::Expr>>,
  ) -> Result<Ty> {
    // todo:
    // we need to keep trace of loops, then we can handle an error, if we
    // encounter a length `zero`.

    todo!()
  }

  fn check_expr_continue(&mut self) -> Result<Ty> {
    // todo:
    // we need to keep trace of loops, then we can handle an error, if we
    // encounter a length `zero`.

    todo!()
  }

  fn check_expr_var(&mut self, _var: &ast::Var) -> Result<Ty> {
    todo!()
  }

  fn check_expr_struct_expr(
    &mut self,
    _struct_expr: &ast::StructExpr,
  ) -> Result<Ty> {
    todo!()
  }

  fn check_expr_chaining(
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
pub fn check(session: &mut Session, program: &ast::Program) -> Result<()> {
  Tychecker::new(&session.reporter).check(program)
}
