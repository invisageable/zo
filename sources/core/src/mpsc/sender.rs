use crate::reporter::report::chan::Chan;
use crate::Result;

#[cfg(feature = "crossbeam-channel")]
pub type SenderError = crossbeam_channel::SendError<String>;
#[cfg(feature = "crossbeam-channel")]
pub type SenderInner<S> = crossbeam_channel::Sender<S>;

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
  #[inline]
  pub fn new(sender: SenderInner<T>) -> Self {
    Self {
      raw: std::sync::Arc::new(std::sync::Mutex::new(sender)),
    }
  }

  #[inline]
  pub fn send(&self, item: T) -> Result<()> {
    self
      .raw
      .lock()
      .map(|raw| raw.send(item).map_err(Chan::error))
      .unwrap()
  }
}
