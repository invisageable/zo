#[derive(Debug)]
pub struct Stack<I> {
  capacity: usize,
  items: Vec<I>,
}

impl<I> Stack<I> {
  pub fn of(capacity: usize) -> Self {
    Self {
      capacity,
      items: Vec::with_capacity(capacity),
    }
  }

  pub fn is_empty(&self) -> bool {
    self.items.is_empty()
  }

  pub fn peek(&self) -> Option<&I> {
    self.items.last()
  }

  pub fn pop(&mut self) -> Option<I> {
    self.items.pop()
  }

  pub fn push(&mut self, item: I) -> bool {
    if self.items.len() == self.capacity {
      return false;
    }

    self.items.push(item);

    true
  }

  pub fn size(&self) -> usize {
    self.items.len()
  }
}

impl<I> Default for Stack<I> {
  fn default() -> Self {
    Self::of(128usize)
  }
}

#[cfg(test)]
mod test {
  use super::Stack;

  struct Item;
  impl Item {
    fn foobar(&self) -> &'static str {
      "I'm an item."
    }
  }

  fn make_stack() -> Stack<Item> {
    Stack::of(128usize)
  }

  fn fill_stack(stack: &mut Stack<Item>, items: Vec<Item>) {
    for item in items {
      stack.push(item);
    }
  }

  #[test]
  fn should_make_filled_stack() {
    let mut stack = make_stack();

    fill_stack(&mut stack, vec![Item, Item, Item, Item]);
    assert!(stack.size() > 0);

    let Some(item) = stack.pop() else {
      std::process::exit(1)
    };

    assert!(item.foobar() == "I'm an item.");
  }
}
