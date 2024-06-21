//! ...

use zo_ast::ast::{
  Ast, BinOp, BinOpKind, Expr, ExprKind, Ext, Fun, Item, ItemKind, Lit,
  LitKind, Pattern, PatternKind, Stmt, StmtKind, TyAlias, UnOp, UnOpKind, Var,
};

use zo_core::interner::symbol::Symbol;
use zo_core::interner::Interner;
use zo_core::reporter::Reporter;
use zo_core::writer::Writer;
use zo_core::{to, Result};

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
      writer: Writer::new(2usize),
    }
  }

  #[inline]
  pub fn output(&mut self) -> Result<Box<[u8]>> {
    Ok(self.writer.as_bytes())
  }

  pub fn translate(&mut self, _ast: &Ast) -> Result<()> {
    Ok(())
  }
}
