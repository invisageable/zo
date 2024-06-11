//! ...

use super::{Error, Report, ReportError};

use crate::span::Span;

#[derive(Debug)]
pub enum Eval {
  HostMachine(String),
  NotConfigurable(String),
  UnknownUnOp(Span, String),
}

impl Eval {
  #[inline]
  pub fn host_machine(error: impl ToString) -> ReportError {
    ReportError::Eval(Eval::HostMachine(error.to_string()))
  }

  #[inline]
  pub fn not_configurable(error: impl ToString) -> ReportError {
    ReportError::Eval(Eval::NotConfigurable(error.to_string()))
  }
}

impl Error for Eval {
  fn report(&self) -> Report {
    match self {
      Self::HostMachine(error) => todo!("{error}"),
      Self::NotConfigurable(error) => todo!("{error}"),
      Self::UnknownUnOp(span, unop) => todo!("{span}-{unop}"),
    }
  }
}
