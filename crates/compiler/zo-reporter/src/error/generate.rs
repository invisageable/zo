pub mod llvm;

use super::{Diagnostic, Error};

use crate::report::Report;

/// The representation of code generation errors.
#[derive(Debug)]
pub enum Generate {
  /// An engine error.
  Llvm(llvm::Llvm),
}

impl<'a> Diagnostic<'a> for Generate {
  #[inline]
  fn report(&self) -> Report<'a> {
    match self {
      Self::Llvm(llvm) => llvm.report(),
    }
  }
}
