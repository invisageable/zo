use super::receiver::Receiver;
use super::sender::Sender;

pub const CHANNEL_CAPACITY: usize = 1;

pub fn unbounded<T>() -> (Sender<T>, Receiver<T>) {
  let (sender, receiver) = kanal::unbounded();

  (Sender::new(sender), Receiver::new(receiver))
}

pub fn bounded<T>(capacity: usize) -> (Sender<T>, Receiver<T>) {
  let (sender, receiver) = kanal::bounded(capacity);

  (Sender::new(sender), Receiver::new(receiver))
}
