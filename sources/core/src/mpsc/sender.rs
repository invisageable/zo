use crate::reporter::report::chan::Chan;
use crate::reporter::report::ReportError;
use crate::Result;

pub type SenderError = kanal::ReceiveError;
pub type SenderInner<R> = kanal::Sender<R>;

#[derive(Clone, Debug)]
pub struct Sender<T> {
  raw: std::sync::Arc<std::sync::Mutex<SenderInner<T>>>,
}

impl<T> Sender<T> {
  pub fn new(sender: SenderInner<T>) -> Self {
    Self {
      raw: std::sync::Arc::new(std::sync::Mutex::new(sender)),
    }
  }

  pub fn send(&self, item: T) -> Result<()> {
    self
      .raw
      .lock()
      .map(|raw| {
        raw.send(item).map_err(|error| {
          ReportError::Chan(Chan::NotFoundSender(error.to_string()))
        })
      })
      .unwrap()
  }
}
