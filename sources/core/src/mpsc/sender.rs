use crate::reporter::report::chan::Chan;
use crate::Result;

#[cfg(feature = "flume")]
pub type SenderError = flume::SendError<String>;
#[cfg(feature = "flume")]
pub type SenderInner<S> = flume::Sender<S>;

#[cfg(feature = "kanal")]
pub type SenderError = kanal::ReceiveError;
#[cfg(feature = "kanal")]
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
      .map(|raw| raw.send(item).map_err(Chan::error))
      .unwrap()
  }
}
