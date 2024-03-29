//! ...wip.

// note #1 — safety: cannot be declared without type.
//
// note #2 — in case of block should we return an `unit` type of the last
// statement type of a block.

use zhoo_ast::ast;
use zhoo_session::session::Session;
use zhoo_ty::ty::Ty;
use zhoo_ty::tyctx::TyCtx;

use zo_core::interner::symbol::Symbol;
use zo_core::interner::Interner;
use zo_core::reporter::Reporter;
use zo_core::span::Span;
use zo_core::Result;

#[derive(Debug)]
struct Inferencer<'ast> {
  #[allow(dead_code)]
  interner: &'ast mut Interner,
  reporter: &'ast Reporter,
  #[allow(dead_code)]
  tyctx: TyCtx<'ast>,
}

impl<'ast> Inferencer<'ast> {
  #[inline]
  fn new(interner: &'ast mut Interner, reporter: &'ast Reporter) -> Self {
    Self {
      interner,
      reporter,
      tyctx: TyCtx::new(),
    }
  }

  fn infer(&mut self, program: &'ast ast::Program) -> Result<Ty> {
    let mut ty = Ty::UNIT;

    for item in program.items.iter() {
      ty = self.infer_item(item)?;
    }

    self.reporter.abort_if_has_errors();

    Ok(ty)
  }

  fn infer_item(&mut self, item: &'ast ast::Item) -> Result<Ty> {
    match &item.kind {
      ast::ItemKind::Pack(pack) => self.infer_item_pack(pack),
      ast::ItemKind::Load(load) => self.infer_item_load(load),
      ast::ItemKind::Var(var) => self.infer_item_var(var),
      ast::ItemKind::TyAlias(ty_alias) => self.infer_item_ty_alias(ty_alias),
      ast::ItemKind::Ext(ext) => self.infer_item_ext(ext),
      ast::ItemKind::Abstract(abstr) => self.infer_item_abstract(abstr),
      ast::ItemKind::Enum(enumeration) => self.infer_item_enum(enumeration),
      ast::ItemKind::Struct(structure) => self.infer_item_struct(structure),
      ast::ItemKind::Apply(apply) => self.infer_item_apply(apply),
      ast::ItemKind::Fun(fun) => self.infer_item_fun(fun),
    }
  }

  fn infer_item_pack(&mut self, _pack: &'ast ast::Pack) -> Result<Ty> {
    todo!()
  }

  fn infer_item_load(&mut self, _load: &'ast ast::Load) -> Result<Ty> {
    todo!()
  }

  fn infer_item_var(&mut self, var: &'ast ast::Var) -> Result<Ty> {
    self.infer_var(var)
  }

  fn infer_var(&mut self, var: &'ast ast::Var) -> Result<Ty> {
    match &var.maybe_ty {
      Some(ty) => Ok(Ty::from(ty)),
      None => self.infer_expr(&var.value),
    }
  }

  fn infer_item_ty_alias(
    &mut self,
    ty_alias: &'ast ast::TyAlias,
  ) -> Result<Ty> {
    match &ty_alias.maybe_ty {
      Some(ty) => Ok(Ty::from(ty)),
      None => unreachable!(), // note #1.
    }
  }

  fn infer_item_ext(&mut self, ext: &'ast ast::Ext) -> Result<Ty> {
    let t1 = self.infer_prototype(&ext.prototype)?;

    let _t2 = match &ext.maybe_body {
      Some(body) => self.infer_block(body),
      None => unreachable!(),
    };

    Ok(t1)
  }

  fn infer_prototype(&mut self, prototype: &'ast ast::Prototype) -> Result<Ty> {
    match &prototype.output_ty {
      ast::OutputTy::Ty(ty) => Ok(Ty::from(ty)),
      ast::OutputTy::Default(span) => Ok(Ty::unit(*span)),
    }
  }

  fn infer_item_abstract(&mut self, _abstr: &'ast ast::Abstract) -> Result<Ty> {
    todo!()
  }

  fn infer_item_enum(&mut self, _enumeration: &'ast ast::Enum) -> Result<Ty> {
    todo!()
  }

  fn infer_item_struct(&mut self, _structure: &'ast ast::Struct) -> Result<Ty> {
    todo!()
  }

  fn infer_item_apply(&mut self, _apply: &'ast ast::Apply) -> Result<Ty> {
    todo!()
  }

  fn infer_item_fun(&mut self, fun: &'ast ast::Fun) -> Result<Ty> {
    let t1 = self.infer_prototype(&fun.prototype)?;
    let _t2 = self.infer_block(&fun.body)?;

    Ok(t1)
  }

  // note #2.
  fn infer_block(&mut self, block: &'ast ast::Block) -> Result<Ty> {
    let mut ty = Ty::UNIT;

    for stmt in &block.stmts {
      ty = self.infer_stmt(stmt)?;
    }

    Ok(ty)
  }

  fn infer_stmt(&mut self, stmt: &'ast ast::Stmt) -> Result<Ty> {
    match &stmt.kind {
      ast::StmtKind::Var(var) => self.infer_stmt_var(var),
      ast::StmtKind::Item(fun) => self.infer_item(fun),
      ast::StmtKind::Expr(expr) => self.infer_expr(expr),
    }
  }

  fn infer_stmt_var(&mut self, var: &'ast ast::Var) -> Result<Ty> {
    self.infer_var(var)
  }

  fn infer_expr(&mut self, expr: &'ast ast::Expr) -> Result<Ty> {
    match &expr.kind {
      ast::ExprKind::Lit(lit) => self.infer_expr_lit(lit),
      ast::ExprKind::UnOp(unop, rhs) => self.infer_expr_unop(unop, rhs),
      ast::ExprKind::BinOp(binop, lhs, rhs) => {
        self.infer_expr_binop(binop, lhs, rhs)
      }
      ast::ExprKind::Assign(_, rhs) => self.infer_expr_assign(rhs),
      ast::ExprKind::AssignOp(_, lhs, rhs) => {
        self.infer_expr_assignop(lhs, rhs)
      }
      ast::ExprKind::Block(block) => self.infer_expr_block(block),
      ast::ExprKind::Array(elmts) => self.infer_expr_array(elmts),
      ast::ExprKind::Tuple(elmts) => self.infer_expr_tuple(elmts),
      ast::ExprKind::ArrayAccess(array, access) => {
        self.infer_expr_array_access(array, access)
      }
      ast::ExprKind::TupleAccess(array, access) => {
        self.infer_expr_tuple_access(array, access)
      }
      ast::ExprKind::Fn(prototype, body) => self.infer_expr_fn(prototype, body),
      ast::ExprKind::Call(callee, args) => self.infer_expr_call(callee, args),
      ast::ExprKind::Return(maybe_expr) => self.infer_expr_return(maybe_expr),
      ast::ExprKind::IfElse(condition, consequence, maybe_alternative) => {
        self.infer_expr_if_else(condition, consequence, maybe_alternative)
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

  fn infer_expr_lit(&mut self, lit: &'ast ast::Lit) -> Result<Ty> {
    match &lit.kind {
      ast::LitKind::Int(symbol) => self.infer_expr_lit_int(symbol, lit.span),
      ast::LitKind::Float(symbol) => {
        self.infer_expr_lit_float(symbol, lit.span)
      }
      ast::LitKind::Ident(symbol) => {
        self.infer_expr_lit_ident(symbol, lit.span)
      }
      ast::LitKind::Bool(_) => self.infer_expr_lit_bool(lit.span),
      ast::LitKind::Char(_) => self.infer_expr_lit_char(lit.span),
      ast::LitKind::Str(_) => self.infer_expr_lit_str(lit.span),
    }
  }

  fn infer_expr_lit_int(
    &mut self,
    _int: &'ast Symbol,
    span: Span,
  ) -> Result<Ty> {
    Ok(Ty::int(span))
  }

  fn infer_expr_lit_float(
    &mut self,
    _symbol: &Symbol,
    span: Span,
  ) -> Result<Ty> {
    Ok(Ty::float(span))
  }

  fn infer_expr_lit_ident(
    &mut self,
    ident: &'ast Symbol,
    span: Span,
  ) -> Result<Ty> {
    Ok(Ty::ident(*ident, span))
  }

  fn infer_expr_lit_bool(&mut self, span: Span) -> Result<Ty> {
    Ok(Ty::bool(span))
  }

  fn infer_expr_lit_char(&mut self, span: Span) -> Result<Ty> {
    Ok(Ty::char(span))
  }

  fn infer_expr_lit_str(&mut self, span: Span) -> Result<Ty> {
    Ok(Ty::str(span))
  }

  fn infer_expr_unop(
    &mut self,
    _unop: &'ast ast::UnOp,
    _rhs: &'ast ast::Expr,
  ) -> Result<Ty> {
    Ok(Ty::UNIT)
  }

  fn infer_expr_binop(
    &mut self,
    _binop: &'ast ast::BinOp,
    _lhs: &'ast ast::Expr,
    _rhs: &'ast ast::Expr,
  ) -> Result<Ty> {
    Ok(Ty::UNIT)
  }

  fn infer_expr_return(
    &mut self,
    maybe_expr: &'ast Option<Box<ast::Expr>>,
  ) -> Result<Ty> {
    match maybe_expr {
      Some(expr) => self.infer_expr(expr),
      None => Ok(Ty::UNIT),
    }
  }

  fn infer_expr_if_else(
    &mut self,
    condition: &'ast ast::Expr,
    consequence: &'ast ast::Block,
    maybe_alternative: &'ast Option<Box<ast::Expr>>,
  ) -> Result<Ty> {
    let _t1 = self.infer_expr(condition)?;
    let t2 = self.infer_block(consequence)?;

    match maybe_alternative {
      Some(alternative) => {
        let t3 = self.infer_expr(alternative)?;

        if !t2.is(t3.kind) {
          panic!("mismatch types.");
        }

        Ok(t2)
      }
      None => Ok(t2),
    }
  }

  fn infer_expr_when(
    &mut self,
    condition: &'ast ast::Expr,
    consequence: &'ast ast::Expr,
    alternative: &'ast ast::Expr,
  ) -> Result<Ty> {
    let _t1 = self.infer_expr(condition)?;
    let t2 = self.infer_expr(consequence)?;
    let t3 = self.infer_expr(alternative)?;

    if !t2.is(t3.kind) {
      panic!("mismatch types.");
    }

    Ok(t2)
  }

  fn infer_expr_match(
    &mut self,
    _condition: &'ast ast::Expr,
    _arms: &'ast [ast::Arm],
  ) -> Result<Ty> {
    todo!()
  }

  fn infer_expr_break(
    &mut self,
    maybe_expr: &'ast Option<Box<ast::Expr>>,
  ) -> Result<Ty> {
    match maybe_expr {
      Some(expr) => self.infer_expr(expr),
      None => Ok(Ty::UNIT),
    }
  }

  fn infer_expr_continue(&mut self) -> Result<Ty> {
    Ok(Ty::UNIT)
  }

  fn infer_expr_assign(&mut self, rhs: &'ast ast::Expr) -> Result<Ty> {
    let ty = self.infer_expr(rhs)?;

    Ok(ty)
  }

  // note #2.
  fn infer_expr_block(&mut self, block: &'ast ast::Block) -> Result<Ty> {
    let _ty = self.infer_block(block)?;

    Ok(Ty::UNIT)
  }

  fn infer_expr_array(&mut self, exprs: &'ast [ast::Expr]) -> Result<Ty> {
    let mut exprs = exprs.iter();
    let maybe_expr = exprs.next();

    match maybe_expr {
      Some(expr) => {
        let t1 = self.infer_expr(expr)?;

        for expr in exprs {
          let t2 = self.infer_expr(expr)?;

          if !t1.is(t2.kind) {
            panic!("mismatch types");
          }
        }

        Ok(t1)
      }
      None => unreachable!(),
    }
  }

  fn infer_expr_tuple(&mut self, _exprs: &'ast [ast::Expr]) -> Result<Ty> {
    todo!()
  }

  fn infer_expr_array_access(
    &mut self,
    _array: &'ast ast::Expr,
    _access: &'ast ast::Expr,
  ) -> Result<Ty> {
    todo!()
  }

  fn infer_expr_tuple_access(
    &mut self,
    _tuple: &'ast ast::Expr,
    _access: &'ast ast::Expr,
  ) -> Result<Ty> {
    todo!()
  }

  // note #2.
  fn infer_expr_fn(
    &mut self,
    prototype: &'ast ast::Prototype,
    body: &'ast ast::Block,
  ) -> Result<Ty> {
    let t1 = self.infer_prototype(prototype)?;
    let _t2 = self.infer_block(body)?;

    Ok(t1)
  }

  fn infer_expr_call(
    &mut self,
    _callee: &'ast ast::Expr,
    _args: &'ast ast::Args,
  ) -> Result<Ty> {
    todo!()
  }

  fn infer_expr_assignop(
    &mut self,
    _lhs: &'ast ast::Expr,
    rhs: &'ast ast::Expr,
  ) -> Result<Ty> {
    let ty = self.infer_expr(rhs)?;

    Ok(ty)
  }

  fn infer_expr_loop(&mut self, block: &'ast ast::Block) -> Result<Ty> {
    let _ty = self.infer_block(block)?;

    Ok(Ty::UNIT)
  }

  fn infer_expr_while(
    &mut self,
    condition: &'ast ast::Expr,
    body: &'ast ast::Block,
  ) -> Result<Ty> {
    let _t1 = self.infer_expr(condition)?;
    let _t2 = self.infer_block(body)?;

    Ok(Ty::UNIT)
  }

  fn infer_expr_for(&mut self, _for_loop: &'ast ast::For) -> Result<Ty> {
    todo!()
  }

  fn infer_expr_var(&mut self, var: &'ast ast::Var) -> Result<Ty> {
    self.infer_var(var)
  }

  fn infer_expr_struct_expr(
    &mut self,
    _struct_expr: &'ast ast::StructExpr,
  ) -> Result<Ty> {
    todo!()
  }

  fn infer_expr_chaining(
    &mut self,
    _lhs: &'ast ast::Expr,
    _rhs: &'ast ast::Expr,
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
pub fn infer(session: &mut Session, program: &ast::Program) -> Result<Ty> {
  Inferencer::new(&mut session.interner, &session.reporter).infer(program)
}
