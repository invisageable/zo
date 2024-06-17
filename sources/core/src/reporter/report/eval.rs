//! ...

use super::{Error, Report, ReportError};

use crate::span::Span;

#[derive(Debug)]
pub enum Eval {
  HostMachine(String),
  NotConfigurable(String),
  MismatchArgument(Span, usize, usize),
  UnknownArrayAccess(Span, String, String),
  UnknownArrayAccessOperator(Span, String),
  UnknownBinOp(Span, String),
  UnknownBinOpOperand(Span, String, String),
  UnknownCallee(Span, String),
  UnknownUnOp(Span, String),
  UnknownUnOpOperand(Span, String),
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
      Self::MismatchArgument(span, expected, actual) => {
        todo!("{span}-{expected}-{actual}")
      }
      Self::UnknownArrayAccess(span, indexed, index) => {
        todo!("{span}-{indexed}-{index}")
      }
      Self::UnknownArrayAccessOperator(span, index) => {
        todo!("{span}-{index}")
      }
      Self::UnknownBinOp(span, binop) => {
        todo!("{span}-{binop}")
      }
      Self::UnknownBinOpOperand(span, lhs, rhs) => {
        todo!("{span}-{lhs}-{rhs}")
      }
      Self::UnknownCallee(span, callee) => {
        todo!("{span}-{callee}")
      }
      Self::UnknownUnOp(span, unop) => {
        todo!("{span}-{unop}")
      }
      Self::UnknownUnOpOperand(span, unop) => {
        todo!("{span}-{unop}")
      }
    }
  }
}
