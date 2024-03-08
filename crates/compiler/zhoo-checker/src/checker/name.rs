//! ...

use zhoo_ast::ast;
use zhoo_session::session::Session;

use zo_core::case::strcase::StrCase;
use zo_core::interner::symbol::{Symbol, Symbolize};
use zo_core::interner::Interner;
use zo_core::reporter::report::semantic::Semantic;
use zo_core::reporter::report::ReportError;
use zo_core::reporter::Reporter;
use zo_core::span::Span;
use zo_core::{is, to, Result};

#[derive(Debug)]
struct NameChecker<'program> {
  interner: &'program mut Interner,
  reporter: &'program Reporter,
}

impl<'program> NameChecker<'program> {
  fn new(
    interner: &'program mut Interner,
    reporter: &'program Reporter,
  ) -> Self {
    Self { interner, reporter }
  }

  fn check(&mut self, program: &ast::Program) -> Result<()> {
    for item in &program.items {
      if let Err(error) = self.check_item(item) {
        self.reporter.add_report(error);
      }
    }

    self.reporter.abort_if_has_errors();

    Ok(())
  }

  fn check_item(&mut self, item: &ast::Item) -> Result<()> {
    match &item.kind {
      ast::ItemKind::Ext(ext) => self.check_item_ext(ext),
      ast::ItemKind::TyAlias(ty_alias) => self.check_item_ty_alias(ty_alias),
      ast::ItemKind::Var(var) => self.check_item_var(var),
      ast::ItemKind::Fun(fun) => self.check_item_fun(fun),
    }
  }

  fn check_item_ext(&mut self, ext: &ast::Ext) -> Result<()> {
    self.check_prototype(&ext.prototype)?;

    if let Some(body) = &ext.maybe_body {
      self.check_block(body)?;
    };

    Ok(())
  }

  fn check_item_ty_alias(&mut self, _ty_alias: &ast::TyAlias) -> Result<()> {
    Ok(())
  }

  fn check_item_var(&mut self, _var: &ast::Var) -> Result<()> {
    Ok(())
  }

  fn check_item_fun(&mut self, fun: &ast::Fun) -> Result<()> {
    self.check_prototype(&fun.prototype)?;
    self.check_block(&fun.body)
  }

  fn check_prototype(&mut self, prototype: &ast::Prototype) -> Result<()> {
    self.check_pattern(&prototype.pattern)?;
    self.check_inputs(&prototype.inputs)?;
    self.check_output_ty(&prototype.output)
  }

  // todo (ivs) — strcase should be passed as argument because a pattern are not
  // snake screaming only.
  fn check_pattern(&mut self, pattern: &ast::Pattern) -> Result<()> {
    let ident = self.interner.lookup_ident(*pattern.symbolize());

    self.verify_snake_case(pattern.span, ident)
  }

  fn check_inputs(&mut self, _inputs: &ast::Inputs) -> Result<()> {
    Ok(())
  }

  fn check_output_ty(&mut self, _output_ty: &ast::OutputTy) -> Result<()> {
    Ok(())
  }

  fn check_block(&mut self, block: &ast::Block) -> Result<()> {
    for stmt in &block.stmts {
      self.check_stmt(stmt)?;
    }

    Ok(())
  }

  fn check_stmt(&mut self, stmt: &ast::Stmt) -> Result<()> {
    match &stmt.kind {
      ast::StmtKind::Var(var) => self.check_stmt_var(var),
      ast::StmtKind::Item(item) => self.check_stmt_item(item),
      ast::StmtKind::Expr(expr) => self.check_stmt_expr(expr),
    }
  }

  fn check_stmt_var(&mut self, _var: &ast::Var) -> Result<()> {
    Ok(())
  }

  fn check_stmt_item(&mut self, item: &ast::Item) -> Result<()> {
    self.check_item(item)
  }

  fn check_stmt_expr(&mut self, expr: &ast::Expr) -> Result<()> {
    self.check_expr(expr)
  }

  fn check_expr(&mut self, expr: &ast::Expr) -> Result<()> {
    match &expr.kind {
      ast::ExprKind::Lit(lit) => self.check_expr_lit(lit),
      ast::ExprKind::UnOp(_, rhs) => self.check_expr_unop(rhs),
      ast::ExprKind::BinOp(_, lhs, rhs) => self.check_expr_binop(lhs, rhs),
      _ => Ok(()),
    }
  }

  fn check_expr_lit(&mut self, lit: &ast::Lit) -> Result<()> {
    match &lit.kind {
      ast::LitKind::Ident(symbol) => self.check_expr_ident(symbol),
      _ => Ok(()),
    }
  }

  fn check_expr_ident(&mut self, ident: &Symbol) -> Result<()> {
    let _ident = self.interner.lookup_ident(*ident);

    Ok(())
  }

  fn check_expr_unop(&mut self, _rhs: &ast::Expr) -> Result<()> {
    Ok(())
  }

  fn check_expr_binop(
    &mut self,
    _lhs: &ast::Expr,
    _rhs: &ast::Expr,
  ) -> Result<()> {
    Ok(())
  }

  #[allow(dead_code)]
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

  #[allow(dead_code)]
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
    };

    ReportError::Semantic(Semantic::NamingConvention(name.into(), naming, span))
  }
}

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn check(session: &mut Session, program: &ast::Program) -> Result<()> {
  NameChecker::new(&mut session.interner, &session.reporter).check(program)
}
