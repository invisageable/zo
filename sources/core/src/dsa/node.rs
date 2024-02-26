pub trait Kind<K> {
  fn kind(&self) -> &K;
}

#[derive(Debug)]
pub struct Node<K> {
  kind: K,
}

impl<K> Kind<K> for Node<K> {
  fn kind(&self) -> &K {
    &self.kind
  }
}
