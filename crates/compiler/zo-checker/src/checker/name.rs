//! ...

use zo_ast::ast::{
  Args, Ast, BinOp, Block, Enum, Expr, ExprKind, Ext, Field, Fields, Fun,
  Inputs, Item, ItemKind, Lit, Load, OutputTy, Pattern, Prototype, Stmt,
  StmtKind, Struct, StructExpr, TyAlias, Var,
};

use zo_session::session::Session;

use zo_core::case::strcase::StrCase;
use zo_core::interner::symbol::Symbolize;
use zo_core::interner::Interner;
use zo_core::reporter::report::semantic::Semantic;
use zo_core::reporter::report::ReportError;
use zo_core::reporter::Reporter;
use zo_core::span::Span;
use zo_core::{is, to, Result};

struct NameChecker<'ast> {
  interner: &'ast mut Interner,
  reporter: &'ast Reporter,
}

impl<'ast> NameChecker<'ast> {
  #[inline]
  fn new(interner: &'ast mut Interner, reporter: &'ast Reporter) -> Self {
    Self { interner, reporter }
  }

  fn check(&mut self, ast: &Ast) -> Result<()> {
    for stmt in ast.iter() {
      if let Err(error) = self.check_stmt(stmt) {
        self.reporter.add_report(error);
      }
    }

    Ok(())
  }

  fn check_pattern(&mut self, pattern: &Pattern, case: StrCase) -> Result<()> {
    let ident = self.interner.lookup_ident(pattern.as_symbol());

    match case {
      StrCase::Pascal => self.verify_pascal_case(pattern.span, ident),
      StrCase::Snake => self.verify_snake_case(pattern.span, ident),
      StrCase::SnakeScreaming => {
        self.verify_snake_screaming_case(pattern.span, ident)
      }
      _ => panic!(), // should returns reporter error message.
    }
  }

  fn check_item(&mut self, item: &Item) -> Result<()> {
    match &item.kind {
      ItemKind::Load(load) => self.check_item_load(load),
      ItemKind::Var(var) => self.check_item_var(var),
      ItemKind::TyAlias(ty_alias) => self.check_item_ty_alias(ty_alias),
      ItemKind::Ext(ext) => self.check_item_ext(ext),
      ItemKind::Enum(enm) => self.check_item_enum(enm),
      ItemKind::Struct(structure) => self.check_item_struct(structure),
      ItemKind::Fun(fun) => self.check_item_fun(fun),
    }
  }

  fn check_item_load(&mut self, load: &Load) -> Result<()> {
    self.check(&load.ast)
  }

  fn check_item_var(&mut self, var: &Var) -> Result<()> {
    self.check_global_var(var)
  }

  fn check_global_var(&mut self, var: &Var) -> Result<()> {
    self.check_pattern(&var.pattern, StrCase::SnakeScreaming)?;
    self.check_expr(&var.value)
  }

  fn check_item_ty_alias(&mut self, ty_alias: &TyAlias) -> Result<()> {
    let name = self.interner.lookup_ident(ty_alias.ident.name);

    self.verify_pascal_case(ty_alias.ident.span, name)
  }

  fn check_item_ext(&mut self, ext: &Ext) -> Result<()> {
    self.check_prototype(&ext.prototype)
  }

  fn check_item_enum(&mut self, enm: &Enum) -> Result<()> {
    let name = self.interner.lookup_ident(enm.ident.name);

    self.verify_pascal_case(enm.ident.span, name)
  }

  fn check_item_struct(&mut self, strct: &Struct) -> Result<()> {
    let name = self.interner.lookup_ident(strct.ident.name);

    self.verify_pascal_case(strct.span, name)?;
    self.check_fields(&strct.fields)
  }

  fn check_fields(&mut self, fields: &Fields) -> Result<()> {
    for field in fields.iter() {
      self.check_field(field)?;
    }

    Ok(())
  }

  // field `ident` property should be renamed `key`.
  // field `ty` should be verify and impl the `Symbolize` trait.
  fn check_field(&mut self, field: &Field) -> Result<()> {
    let name = self.interner.lookup_ident(field.ident.name);

    self.verify_snake_case(field.ident.span, name)?;

    Ok(())
  }

  fn check_item_fun(&mut self, fun: &Fun) -> Result<()> {
    self.check_prototype(&fun.prototype)?;
    self.check_block(&fun.body)
  }

  fn check_stmt(&mut self, stmt: &Stmt) -> Result<()> {
    match &stmt.kind {
      StmtKind::Var(var) => self.check_stmt_var(var),
      StmtKind::Item(item) => self.check_stmt_item(item),
      StmtKind::Expr(expr) => self.check_stmt_expr(expr),
    }
  }

  fn check_stmt_var(&mut self, var: &Var) -> Result<()> {
    self.check_local_var(var)
  }

  fn check_local_var(&mut self, var: &Var) -> Result<()> {
    self.check_pattern(&var.pattern, StrCase::Snake)?;
    self.check_expr(&var.value)
  }

  fn check_stmt_item(&mut self, item: &Item) -> Result<()> {
    self.check_item(item)
  }

  fn check_stmt_expr(&mut self, expr: &Expr) -> Result<()> {
    self.check_expr(expr)
  }

  fn check_expr(&mut self, expr: &Expr) -> Result<()> {
    match &expr.kind {
      ExprKind::Lit(lit) => self.check_expr_lit(lit),
      ExprKind::UnOp(_, rhs) => self.check_expr_unop(rhs),
      ExprKind::BinOp(_, lhs, rhs) => self.check_expr_binop(lhs, rhs),
      ExprKind::Assign(assignee, value) => {
        self.check_expr_assgin(assignee, value)
      }
      ExprKind::AssignOp(binop, assignee, value) => {
        self.check_expr_assgin_op(binop, assignee, value)
      }
      ExprKind::Block(body) => self.check_expr_block(body),
      ExprKind::Fn(prototype, body) => self.check_expr_fn(prototype, body),
      ExprKind::Call(callee, args) => self.check_expr_call(callee, args),
      ExprKind::Array(elmts) => self.check_expr_array(elmts),
      ExprKind::ArrayAccess(indexed, index) => {
        self.check_expr_array_access(indexed, index)
      }
      ExprKind::Struct(strctr) => self.check_expr_struct(strctr),
      ExprKind::StructAccess(strctr, prop) => {
        self.check_expr_struct_access(strctr, prop)
      }
      ExprKind::IfElse(condition, consequence, maybe_alternative) => {
        self.check_expr_if_else(condition, consequence, maybe_alternative)
      }
      ExprKind::When(condition, consequence, maybe_alternative) => {
        self.check_expr_when(condition, consequence, maybe_alternative)
      }
      ExprKind::Loop(body) => self.check_expr_loop(body),
      ExprKind::While(condition, body) => {
        self.check_expr_while(condition, body)
      }
      ExprKind::Return(maybe_expr) => self.check_expr_return(maybe_expr),
      ExprKind::Break(maybe_expr) => self.check_expr_break(maybe_expr),
      ExprKind::Var(var) => self.check_expr_var(var),
      _ => Ok(()),
    }
  }

  fn check_expr_lit(&mut self, _lit: &Lit) -> Result<()> {
    Ok(())
  }

  fn check_expr_unop(&mut self, rhs: &Expr) -> Result<()> {
    self.check_expr(rhs)
  }

  fn check_expr_binop(&mut self, lhs: &Expr, rhs: &Expr) -> Result<()> {
    self.check_expr(lhs)?;
    self.check_expr(rhs)
  }

  fn check_expr_assgin(
    &mut self,
    _assignee: &Expr,
    _value: &Expr,
  ) -> Result<()> {
    todo!()
  }

  fn check_expr_assgin_op(
    &mut self,
    _binop: &BinOp,
    _assignee: &Expr,
    _value: &Expr,
  ) -> Result<()> {
    Ok(())
  }

  fn check_expr_block(&mut self, body: &Block) -> Result<()> {
    self.check_block(body)
  }

  fn check_block(&mut self, body: &Block) -> Result<()> {
    for stmt in &body.stmts {
      self.check_stmt(stmt)?;
    }

    Ok(())
  }

  fn check_expr_fn(
    &mut self,
    prototype: &Prototype,
    body: &Block,
  ) -> Result<()> {
    self.check_prototype(prototype)?;
    self.check_block(body)
  }

  fn check_prototype(&mut self, prototype: &Prototype) -> Result<()> {
    self.check_pattern(&prototype.pattern, StrCase::Snake)?;
    self.check_inputs(&prototype.inputs)?;
    self.check_output_ty(&prototype.output_ty)
  }

  fn check_inputs(&mut self, inputs: &Inputs) -> Result<()> {
    for input in inputs.iter() {
      self.check_pattern(&input.pattern, StrCase::Snake)?;
    }

    Ok(())
  }

  fn check_output_ty(&mut self, output_ty: &OutputTy) -> Result<()> {
    match output_ty {
      OutputTy::Default(_) => Ok(()),
      OutputTy::Ty(_) => Ok(()),
    }
  }

  fn check_expr_call(&mut self, _callee: &Expr, _args: &Args) -> Result<()> {
    Ok(()) // tmp.
  }

  fn check_expr_array(&mut self, _elmts: &[Expr]) -> Result<()> {
    Ok(()) // tmp.
  }

  fn check_expr_array_access(
    &mut self,
    _indexed: &Expr,
    _index: &Expr,
  ) -> Result<()> {
    Ok(()) // tmp.
  }

  fn check_expr_struct(&mut self, strctr: &StructExpr) -> Result<()> {
    for (key, _) in strctr.pairs.iter() {
      let name = self.interner.lookup_ident(key.as_symbol());

      self.verify_snake_case(key.span, name)?;
    }

    Ok(())
  }

  fn check_expr_struct_access(
    &mut self,
    strctr: &Expr,
    prop: &Expr,
  ) -> Result<()> {
    let name = self.interner.lookup_ident(strctr.as_symbol());

    self.verify_snake_case(strctr.span, name)?;

    let name = self.interner.lookup_ident(prop.as_symbol());

    self.verify_snake_case(prop.span, name)
  }

  fn check_expr_if_else(
    &mut self,
    _condition: &Expr,
    _consequence: &Block,
    _maybe_alternative: &Option<Box<Expr>>,
  ) -> Result<()> {
    Ok(()) // tmp.
  }

  fn check_expr_when(
    &mut self,
    _condition: &Expr,
    _consequence: &Expr,
    _alternative: &Expr,
  ) -> Result<()> {
    Ok(()) // tmp.
  }

  fn check_expr_loop(&mut self, body: &Block) -> Result<()> {
    self.check_block(body)
  }

  fn check_expr_while(
    &mut self,
    _condition: &Expr,
    _body: &Block,
  ) -> Result<()> {
    Ok(()) // tmp.
  }

  fn check_expr_return(
    &mut self,
    _maybe_expr: &Option<Box<Expr>>,
  ) -> Result<()> {
    Ok(()) // tmp.
  }

  fn check_expr_break(
    &mut self,
    _maybe_expr: &Option<Box<Expr>>,
  ) -> Result<()> {
    todo!()
  }

  fn check_expr_var(&mut self, _var: &Var) -> Result<()> {
    todo!()
  }

  fn verify_pascal_case(&self, span: Span, name: &str) -> Result<()> {
    if is!(pascal name) {
      return Ok(());
    }

    Err(self.error_naming_convention(name, span, StrCase::Pascal))
  }

  fn verify_snake_case(&self, span: Span, name: &str) -> Result<()> {
    if is!(snake name) {
      return Ok(());
    }

    Err(self.error_naming_convention(name, span, StrCase::Snake))
  }

  fn verify_snake_screaming_case(&self, span: Span, name: &str) -> Result<()> {
    if is!(snake_screaming name) {
      return Ok(());
    }

    Err(self.error_naming_convention(name, span, StrCase::SnakeScreaming))
  }

  fn error_naming_convention(
    &self,
    name: &str,
    span: Span,
    naming: StrCase,
  ) -> ReportError {
    let naming = match naming {
      StrCase::Pascal => to!(pascal name),
      StrCase::Snake => to!(snake name),
      StrCase::SnakeScreaming => to!(snake_screaming name),
      _ => unreachable!(),
    };

    ReportError::Semantic(Semantic::NamingConvention(span, name.into(), naming))
  }
}

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn check(session: &mut Session, ast: &Ast) -> Result<()> {
  NameChecker::new(&mut session.interner, &session.reporter).check(ast)
}
