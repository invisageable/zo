//! ...

use zo_value::value::Value;

use zo_core::interner::symbol::Symbol;

use hashbrown::hash_map::Entry;
use hashbrown::HashMap;
use smol_str::SmolStr;

#[derive(Default, Debug)]
pub struct Scope {
  vars: HashMap<Symbol, Value>,
  funs: HashMap<Symbol, Value>,
}

impl Scope {
  #[inline]
  pub fn new() -> Self {
    Self {
      vars: HashMap::with_capacity(0usize),
      funs: HashMap::with_capacity(0usize),
    }
  }

  pub fn add_var(&mut self, name: Symbol, value: Value) -> Result<(), String> {
    match self.vars.entry(name) {
      Entry::Occupied(_) => {
        Err(format!("the variable `{}` already exist.", name))
      }
      Entry::Vacant(vars) => {
        vars.insert(value);
        Ok(())
      }
    }
  }

  pub fn add_fun(&mut self, fun: Value) -> Result<(), String> {
    match self.funs.entry(fun.symbolize()) {
      Entry::Occupied(_) => {
        Err(format!("the function `{}` already exist.", fun.symbolize()))
      }
      Entry::Vacant(funs) => {
        funs.insert(fun);
        Ok(())
      }
    }
  }

  #[inline]
  pub fn var(&self, name: &Symbol) -> Option<&Value> {
    self.vars.get(name)
  }

  #[inline]
  pub fn fun(&self, name: &Symbol) -> Option<&Value> {
    self.funs.get(name)
  }
}
