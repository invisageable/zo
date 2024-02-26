use crate::reporter::report::chan::Chan;
use crate::reporter::report::ReportError;
use crate::Result;

pub type ReceiverError = kanal::ReceiveError;
pub type ReceiverInner<R> = kanal::Receiver<R>;

#[derive(Clone, Debug)]
pub struct Receiver<T> {
  raw: std::sync::Arc<std::sync::Mutex<ReceiverInner<T>>>,
}

impl<T> Receiver<T> {
  pub fn new(receiver: ReceiverInner<T>) -> Self {
    Self {
      raw: std::sync::Arc::new(std::sync::Mutex::new(receiver)),
    }
  }

  pub fn recv(&self) -> Result<T> {
    self
      .raw
      .lock()
      .map(|raw| {
        raw.recv().map_err(|error| {
          ReportError::Chan(Chan::NotFoundReceiver(error.to_string()))
        })
      })
      .unwrap()
  }
}
