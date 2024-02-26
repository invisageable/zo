use super::{Error, Report, ReportError};

#[derive(Debug)]
pub enum Chan {
  NotFoundSignal(String),
}

impl Error for Chan {
  fn report(&self) -> Report {
    match self {
      Self::NotFoundSignal(message) => todo!("{message}"),
    }
  }
}

impl Chan {
  pub fn error(message: impl ToString) -> ReportError {
    ReportError::Chan(Chan::NotFoundSignal(message.to_string()))
  }
}
