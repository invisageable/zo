use zhoo_ast::ast;

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

  pub fn output(&mut self) -> Result<Box<[u8]>> {
    todo!()
  }

  pub fn translate(&mut self, _program: &ast::Program) -> Result<()> {
    Ok(())
  }
}
