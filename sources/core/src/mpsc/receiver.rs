//! ...

use crate::reporter::report::chan::Chan;
use crate::Result;

#[cfg(feature = "crossbeam-channel")]
pub type ReceiverError = crossbeam_channel::RecvError;
#[cfg(feature = "crossbeam-channel")]
pub type ReceiverInner<R> = crossbeam_channel::Receiver<R>;

#[cfg(feature = "flume")]
pub type ReceiverError = flume::RecvError;
#[cfg(feature = "flume")]
pub type ReceiverInner<R> = flume::Receiver<R>;

#[cfg(feature = "kanal")]
pub type ReceiverError = kanal::ReceiveError;
#[cfg(feature = "kanal")]
pub type ReceiverInner<R> = kanal::Receiver<R>;

#[derive(Clone, Debug)]
pub struct Receiver<T> {
  raw: std::sync::Arc<std::sync::Mutex<ReceiverInner<T>>>,
}

impl<T> Receiver<T> {
  #[inline]
  pub fn new(receiver: ReceiverInner<T>) -> Self {
    Self {
      raw: std::sync::Arc::new(std::sync::Mutex::new(receiver)),
    }
  }

  #[inline]
  pub fn recv(&self) -> Result<T> {
    self
      .raw
      .lock()
      .map(|raw| raw.recv().map_err(Chan::error))
      .unwrap()
  }
}
