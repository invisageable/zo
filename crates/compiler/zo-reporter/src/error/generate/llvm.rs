use super::{Diagnostic, Error, Generate};

use crate::color;
use crate::report::{Report, ReportKind};

use ariadne::Fmt;

/// The representation of code generation errors.
#[derive(Debug)]
pub enum Llvm {
  /// An engine error.
  Engine(Engine),
}

impl<'a> Diagnostic<'a> for Llvm {
  #[inline]
  fn report(&self) -> Report<'a> {
    match self {
      Self::Engine(engine) => engine.report(),
    }
  }
}

/// The `engine` errors.
#[derive(Debug)]
pub enum Engine {
  /// An engine creation error.
  Creation(String),
  /// An engine execution error.
  Execution(String),
}

impl<'a> Diagnostic<'a> for Engine {
  #[inline]
  fn report(&self) -> Report<'a> {
    match self {
      Self::Creation(error) => Report {
        kind: ReportKind::ERROR,
        message: format!(
          "{} {error}",
          "creation engine failed.".fg(color::title())
        )
        .into(),
        ..Default::default()
      },
      Self::Execution(error) => Report {
        kind: ReportKind::ERROR,
        message: format!(
          "{} {error}",
          "execution engine failed.".fg(color::title())
        )
        .into(),
        ..Default::default()
      },
    }
  }
}

/// An engine creation error.
#[inline(always)]
pub fn engine(msg: impl ToString) -> Error {
  Error::Generate(Generate::Llvm(Llvm::Engine(Engine::Creation(
    msg.to_string(),
  ))))
}

/// An engine execution error.
#[inline(always)]
pub fn engine_execution(msg: impl ToString) -> Error {
  Error::Generate(Generate::Llvm(Llvm::Engine(Engine::Execution(
    msg.to_string(),
  ))))
}
