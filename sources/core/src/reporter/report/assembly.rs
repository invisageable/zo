use super::{Error, Report};

#[derive(Debug)]
pub enum Assembly {
  Dummy,
  NotFoundVar(String),
}

impl Error for Assembly {
  fn report(&self) -> Report {
    match self {
      Self::Dummy => Report::default(),
      Self::NotFoundVar(var) => todo!("{var}"),
    }
  }
}
