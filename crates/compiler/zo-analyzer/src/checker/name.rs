use zo_ast::ast;
use zo_interner::interner::symbol::Symbolize;
use zo_interner::interner::Interner;
use zo_reporter::error::Error;
use zo_reporter::reporter::Reporter;
use zo_reporter::{error, Result};
use zo_session::session::Session;

use swisskit::case::strcase::StrCase;
use swisskit::span::Span;
use swisskit::{is, to};

/// The representation of a name checker.
struct NameChecker<'ast> {
  /// The interner — see also [`Interner`].
  interner: &'ast mut Interner,
  /// The reporter — see also [`Reporter`].
  reporter: &'ast Reporter,
}

impl<'ast> NameChecker<'ast> {
  /// Creates a new name checker.
  #[inline]
  fn new(interner: &'ast mut Interner, reporter: &'ast Reporter) -> Self {
    Self { interner, reporter }
  }

  /// Checks the naming convention from an AST.
  fn check(&mut self, ast: &ast::Ast) -> Result<()> {
    for stmt in ast.iter() {
      if let Err(error) = self.check_stmt(stmt) {
        self.reporter.add_report(error);
      }
    }

    Ok(())
  }

  /// Checks the naming convention of a item.
  fn check_item(&mut self, item: &ast::Item) -> Result<()> {
    match &item.kind {
      ast::ItemKind::Var(var) => self.check_item_var(var),
    }
  }

  /// Checks the naming convention of a variable item.
  fn check_item_var(&mut self, var: &ast::Var) -> Result<()> {
    self.check_global_var(var)
  }

  /// Checks the naming convention of a gloabl variable item.
  fn check_global_var(&mut self, var: &ast::Var) -> Result<()> {
    self.check_pattern(&var.pattern, StrCase::SnakeScreaming)?;
    self.check_expr(&var.value)
  }

  /// Checks the naming convention of a pattern.
  fn check_pattern(
    &mut self,
    pattern: &ast::Pattern,
    case: StrCase,
  ) -> Result<()> {
    let span = pattern.span;
    let name = self.interner.lookup(**pattern.as_symbol());

    match case {
      StrCase::Pascal => verify_pascal_case(span, name),
      StrCase::Snake => verify_snake_case(span, name),
      StrCase::SnakeScreaming => verify_snake_screaming_case(span, name),
      _ => unreachable!(),
    }
  }

  /// Checks the naming convention of a statement.
  fn check_stmt(&mut self, stmt: &ast::Stmt) -> Result<()> {
    match &stmt.kind {
      ast::StmtKind::Var(var) => self.check_stmt_var(var),
      ast::StmtKind::Item(item) => self.check_stmt_item(item),
      ast::StmtKind::Expr(expr) => self.check_stmt_expr(expr),
    }
  }

  /// Checks the naming convention of a variable statement.
  fn check_stmt_var(&mut self, var: &ast::Var) -> Result<()> {
    self.check_local_var(var)
  }

  /// Checks the naming convention of a local variable statement.
  fn check_local_var(&mut self, var: &ast::Var) -> Result<()> {
    self.check_pattern(&var.pattern, StrCase::Snake)?;
    self.check_expr(&var.value)
  }

  /// Checks the naming convention of an item statement.
  fn check_stmt_item(&mut self, item: &ast::Item) -> Result<()> {
    self.check_item(item)
  }

  /// Checks the naming convention of an expression statement.
  fn check_stmt_expr(&mut self, expr: &ast::Expr) -> Result<()> {
    self.check_expr(expr)
  }

  /// Checks the naming convention of a expression.
  fn check_expr(&mut self, _expr: &ast::Expr) -> Result<()> {
    Ok(())
  }
}

/// Verifies the pascal case naming convention.
fn verify_pascal_case(span: Span, name: &str) -> Result<()> {
  if is!(pascal name) {
    return Ok(());
  }

  Err(error_naming_convention(name, span, StrCase::Pascal))
}

/// Verifies the snake case naming convention.
fn verify_snake_case(span: Span, name: &str) -> Result<()> {
  if is!(snake name) {
    return Ok(());
  }

  Err(error_naming_convention(name, span, StrCase::Snake))
}

/// Verifies the snake screaming case naming convention.
fn verify_snake_screaming_case(span: Span, name: &str) -> Result<()> {
  if is!(snake_screaming name) {
    return Ok(());
  }

  Err(error_naming_convention(name, span, StrCase::SnakeScreaming))
}

/// A naming convention error.
fn error_naming_convention(name: &str, span: Span, naming: StrCase) -> Error {
  let naming = match naming {
    StrCase::Pascal => to!(pascal name),
    StrCase::Snake => to!(snake name),
    StrCase::SnakeScreaming => to!(snake_screaming name),
    _ => unreachable!(),
  };

  error::semantic::naming_convention(span, name, naming)
}

/// Checks the naming convention from an AST.
pub fn check(session: &mut Session, ast: &ast::Ast) -> Result<()> {
  NameChecker::new(&mut session.interner, &session.reporter).check(ast)
}
