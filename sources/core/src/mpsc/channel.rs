//! ...

use super::receiver::Receiver;
use super::sender::Sender;

pub const CAPACITY: usize = 1usize;

#[inline]
pub fn unbounded<T>() -> (Sender<T>, Receiver<T>) {
  #[cfg(feature = "crossbeam-channel")]
  let (sender, receiver) = crossbeam_channel::unbounded();

  #[cfg(feature = "flume")]
  let (sender, receiver) = flume::unbounded();

  #[cfg(feature = "kanal")]
  let (sender, receiver) = kanal::unbounded();

  (Sender::new(sender), Receiver::new(receiver))
}

#[inline]
pub fn bounded<T>(capacity: usize) -> (Sender<T>, Receiver<T>) {
  #[cfg(feature = "crossbeam-channel")]
  let (sender, receiver) = crossbeam_channel::bounded(capacity);

  #[cfg(feature = "flume")]
  let (sender, receiver) = flume::bounded(capacity);

  #[cfg(feature = "kanal")]
  let (sender, receiver) = kanal::bounded(capacity);

  (Sender::new(sender), Receiver::new(receiver))
}
