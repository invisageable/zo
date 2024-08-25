// derived from https://matklad.github.io/2020/03/22/fast-simple-rust-interner.html. The latter is
// Copyright (c) Mar 22, 2020, Alex Kladov.

pub mod symbol;

use symbol::Symbol;

use hashbrown::HashMap;

#[derive(Debug, Default)]
pub struct Interner {
  map: HashMap<&'static str, Symbol>,
  vec: Vec<&'static str>,
  buf: String,
  full: Vec<String>,
}

impl Interner {
  #[inline(always)]
  pub fn new() -> Self {
    Self::with_capacity(1024usize)
  }

  pub fn with_capacity(cap: usize) -> Self {
    let cap = cap.next_power_of_two();

    Self {
      map: HashMap::default(),
      vec: Vec::new(),
      buf: String::with_capacity(cap),
      full: Vec::new(),
    }
  }

  pub fn intern(&mut self, name: &str) -> Symbol {
    if let Some(&sym) = self.map.get(name) {
      return sym;
    }

    let name = unsafe { self.alloc(name) };
    let sym = Symbol::new(self.map.len() as u32);

    self.map.insert(name, sym);
    self.vec.push(name);

    debug_assert!(self.lookup(*sym) == name);
    debug_assert!(self.intern(name) == sym);

    sym
  }

  pub fn lookup(&self, id: u32) -> &str {
    self.vec[id as usize]
  }

  pub fn lookup_int(&self, id: impl Into<usize>) -> i64 {
    self.vec[id.into()].parse().unwrap()
  }

  pub fn lookup_float(&self, id: impl Into<usize>) -> f64 {
    self.vec[id.into()].parse().unwrap()
  }

  pub fn lookup_char(&self, id: impl Into<usize>) -> char {
    self.vec[id.into()].chars().next().unwrap()
  }

  unsafe fn alloc(&mut self, name: &str) -> &'static str {
    let cap = self.buf.capacity();

    if cap < self.buf.len() + name.len() {
      let new_cap = (cap.max(name.len()) + 1).next_power_of_two();
      let new_buf = String::with_capacity(new_cap);
      let old_buf = std::mem::replace(&mut self.buf, new_buf);

      self.full.push(old_buf);
    }

    let interned = {
      let start = self.buf.len();

      self.buf.push_str(name);

      &self.buf[start..]
    };

    &*(interned as *const str)
  }
}
