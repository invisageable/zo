use super::{Error, Report};

#[derive(Debug)]
pub enum Chan {
  NotFoundSender(String),
  NotFoundReceiver(String),
}

impl Error for Chan {
  fn report(&self) -> Report {
    match self {
      Self::NotFoundSender(message) => todo!("{message}"),
      Self::NotFoundReceiver(message) => todo!("{message}"),
    }
  }
}
