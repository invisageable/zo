use super::{Error, Report};

#[derive(Debug)]
pub enum Eval {
  UnknownUnOp(Span, String),
}

impl Error for Eval {
  fn report(&self) -> Report {
    match self {
      Self::UnknownUnOp(span, unop) => todo!("{span}-{unop}"),
      _ => todo!(),
    }
  }
}
