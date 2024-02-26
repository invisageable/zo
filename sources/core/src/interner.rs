// adaptation basé sur "Fast and SImple Interner" by @matklad: https://matklad.github.io/2020/03/22/fast-simple-rust-interner.html

pub mod symbol;

use symbol::Symbol;

use hashbrown::HashMap;

#[derive(Clone, Debug, Default)]
pub struct Interner {
  map: HashMap<&'static str, Symbol>,
  vec: Vec<&'static str>,
  buf: String,
  full: Vec<String>,
}

impl Interner {
  #[inline]
  pub fn new() -> Self {
    Self::with_capacity(0usize)
  }

  pub fn with_capacity(capacity: usize) -> Self {
    let capacity = capacity.next_power_of_two();

    Self {
      map: HashMap::new(),
      vec: Vec::with_capacity(0usize),
      buf: String::with_capacity(capacity),
      full: Vec::with_capacity(0usize),
    }
  }

  pub fn intern(&mut self, id: &str) -> Symbol {
    if let Some(&id) = self.map.get(id) {
      return id;
    }

    let id = self.alloc(id);
    let symbol = Symbol(self.map.len() as u32);

    self.map.insert(id, symbol);
    self.vec.push(id);

    symbol
  }

  #[inline]
  pub fn lookup_int(&self, id: impl Into<usize>) -> i64 {
    self.vec[id.into()].parse().unwrap()
  }

  #[inline]
  pub fn lookup_float(&self, id: impl Into<usize>) -> f64 {
    self.vec[id.into()].parse().unwrap()
  }

  #[inline]
  pub fn lookup_char(&self, id: impl Into<usize>) -> char {
    self.vec[id.into()].chars().next().unwrap()
  }

  #[inline]
  pub fn lookup_str(&self, id: impl Into<usize>) -> &str {
    self.vec[id.into()]
  }

  #[inline]
  pub fn lookup_identifier(&self, id: impl Into<usize>) -> &str {
    self.vec[id.into()]
  }

  fn alloc(&mut self, id: &str) -> &'static str {
    let capacity = self.buf.capacity();

    if capacity < self.buf.len() + id.len() {
      let new_capacity = (capacity.max(id.len()) + 1).next_power_of_two();
      let new_buffer = String::with_capacity(new_capacity);
      let old_buffer = std::mem::replace(&mut self.buf, new_buffer);

      self.full.push(old_buffer);
    }

    let interned = {
      let start = self.buf.len();

      self.buf.push_str(id);

      &self.buf[start..]
    };

    unsafe { &*(interned as *const str) }
  }
}
