//! ...

use super::{Error, Report, ReportError};

use crate::span::Span;

#[derive(Debug)]
pub enum Eval {
  HostMachine(String),
  IdentNotFound(Span, String),
  NotConfigurable(String),
  MismatchArgument(Span, usize, usize),
  UnknownArrayAccess(Span, String, String),
  UnknownArrayAccessOperator(Span, String),
  UnknownRecordAccess(Span, String, String),
  UnknownRecordAccessOperator(Span, String),
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
      Self::HostMachine(error) => todo!("HostMachine: {error}"),
      Self::IdentNotFound(span, string) => {
        todo!("IdentNotFound: {span}-{string}")
      }
      Self::NotConfigurable(error) => todo!("NotConfigurable: {error}"),
      Self::MismatchArgument(span, expected, actual) => {
        todo!("MismatchArgument: {span}-{expected}-{actual}")
      }
      Self::UnknownArrayAccess(span, indexed, index) => {
        todo!("UnknownArrayAccess: {span}-{indexed}-{index}")
      }
      Self::UnknownArrayAccessOperator(span, index) => {
        todo!("UnknownArrayAccessOperator: {span}-{index}")
      }
      Self::UnknownRecordAccess(span, indexed, index) => {
        todo!("UnknownRecordAccess: {span}-{indexed}-{index}")
      }
      Self::UnknownRecordAccessOperator(span, index) => {
        todo!("UnknownRecordAccessOperator: {span}-{index}")
      }
      Self::UnknownBinOp(span, binop) => {
        todo!("UnknownBinOp: {span}-{binop}")
      }
      Self::UnknownBinOpOperand(span, lhs, rhs) => {
        todo!("UnknownBinOpOperand: {span}-{lhs}-{rhs}")
      }
      Self::UnknownCallee(span, callee) => {
        todo!("UnknownCallee: {span}-{callee}")
      }
      Self::UnknownUnOp(span, unop) => {
        todo!("UnknownUnOp: {span}-{unop}")
      }
      Self::UnknownUnOpOperand(span, unop) => {
        todo!("UnknownUnOpOperand: {span}-{unop}")
      }
    }
  }
}
