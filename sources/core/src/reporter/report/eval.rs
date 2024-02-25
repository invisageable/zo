use super::{Error, Report};

#[derive(Debug)]
pub enum Eval {
  Dummy,
  UnknownUnOp(Span, String),
}

impl Error for Eval {
  fn report(&self) -> Report {
    match self {
      Self::Dummy => Report::default(),
      Self::UnknownUnOp(span, unop) => todo!("{span}-{unop}"),
      _ => todo!(),
    }
  }
}
