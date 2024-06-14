//! ...

use zo_ast::ast::{Ast, Expr, ExprKind};

use zo_core::interner::Interner;
use zo_core::reporter::Reporter;
use zo_core::writer::Writer;
use zo_core::Result;

pub(crate) struct Translator<'ast> {
  interner: &'ast Interner,
  reporter: &'ast Reporter,
  writer: Writer,
}

impl<'ast> Translator<'ast> {
  #[inline]
  pub fn new(interner: &'ast Interner, reporter: &'ast Reporter) -> Self {
    Self {
      interner,
      reporter,
      writer: Writer::new(0usize),
    }
  }

  pub fn output(&mut self) -> Result<Box<[u8]>> {
    todo!()
  }

  pub fn translate(&mut self, ast: &Ast) -> Result<()> {
    for expr in &ast.exprs {
      self.translate_expr(expr)?;
    }

    Ok(())
  }

  fn translate_expr(&mut self, expr: &Expr) -> Result<()> {
    match &expr.kind {
      ExprKind::Lit(_lit) => todo!(),
      _ => todo!(),
    }
  }
}
