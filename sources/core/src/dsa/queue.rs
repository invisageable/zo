#[derive(Debug)]
pub struct Queue<T> {
  items: std::collections::LinkedList<T>,
}

impl<T> Queue<T> {
  #[inline]
  pub fn new() -> Self {
    Self {
      items: std::collections::LinkedList::new(),
    }
  }

  #[inline]
  pub fn is_empty(&self) -> bool {
    self.items.is_empty()
  }

  #[inline]
  pub fn enqueue(&mut self, item: T) {
    self.items.push_back(item);
  }

  #[inline]
  pub fn dequeue(&mut self) -> Option<T> {
    match self.is_empty() {
      false => self.items.pop_front(),
      true => None,
    }
  }

  #[inline]
  pub fn peek(&self) -> Option<&T> {
    self.items.front()
  }

  #[inline]
  pub fn size(&self) -> usize {
    self.items.len()
  }
}

impl<T> Default for Queue<T> {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod test {
  use super::Queue;

  use crate::EXIT_FAILURE;

  struct Item;
  impl Item {
    #[inline]
    fn foobar(&self) -> &'static str {
      "I'm an item."
    }
  }

  fn make_queue() -> Queue<Item> {
    Queue::new()
  }

  fn fill_queue(queue: &mut Queue<Item>, insts: Vec<Item>) {
    for inst in insts {
      queue.enqueue(inst);
    }
  }

  #[test]
  fn should_make_filled_queue() {
    let mut queue = make_queue();

    fill_queue(&mut queue, vec![Item, Item, Item, Item]);
    assert!(queue.size() > 0);

    let Some(item) = queue.dequeue() else {
      std::process::exit(EXIT_FAILURE)
    };

    assert!(item.foobar() == "I'm an item.");
  }
}
