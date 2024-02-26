use super::receiver::Receiver;
use super::sender::Sender;

pub const CAPACITY: usize = 1usize;

#[inline]
pub fn unbounded<T>() -> (Sender<T>, Receiver<T>) {
  let (sender, receiver) = kanal::unbounded();

  (Sender::new(sender), Receiver::new(receiver))
}

#[inline]
pub fn bounded<T>(capacity: usize) -> (Sender<T>, Receiver<T>) {
  let (sender, receiver) = kanal::bounded(capacity);

  (Sender::new(sender), Receiver::new(receiver))
}
