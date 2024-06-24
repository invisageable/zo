//! ...

use zo_ty::ty::Ty;
use zo_value::value::Value;

use zo_core::interner::symbol::Symbol;

use hashbrown::hash_map::Entry;
use hashbrown::HashMap;

#[derive(Debug)]
pub struct Scope {
  vars: HashMap<Symbol, Value>,
  funs: HashMap<Symbol, Value>,
  tys: HashMap<Symbol, Ty>,
}

impl Scope {
  #[inline]
  pub fn new() -> Self {
    Self {
      vars: HashMap::with_capacity(0usize),
      funs: HashMap::with_capacity(0usize),
      tys: HashMap::with_capacity(0usize),
    }
  }

  pub fn add_var(&mut self, name: Symbol, value: Value) -> Result<(), String> {
    match self.vars.entry(name) {
      Entry::Occupied(_) => {
        Err(format!("the variable `{name}` already exist."))
      }
      Entry::Vacant(vars) => {
        vars.insert(value);
        Ok(())
      }
    }
  }

  pub fn add_fun(&mut self, name: Symbol, value: Value) -> Result<(), String> {
    match self.funs.entry(name) {
      Entry::Occupied(_) => {
        Err(format!("the function `{name}` already exist."))
      }
      Entry::Vacant(funs) => {
        funs.insert(value);
        Ok(())
      }
    }
  }

  pub fn add_ty(&mut self, name: Symbol, ty: Ty) -> Result<(), String> {
    match self.tys.entry(name) {
      Entry::Occupied(_) => Err(format!("the ty `{name}` already exist.")),
      Entry::Vacant(tys) => {
        tys.insert(ty);
        Ok(())
      }
    }
  }

  #[inline]
  pub fn set_var(&mut self, name: Symbol, value: Value) {
    self.vars.insert(name, value);
  }

  #[inline]
  pub fn set_fun(&mut self, name: Symbol, value: Value) {
    self.funs.insert(name, value);
  }

  #[inline]
  pub fn set_ty(&mut self, name: Symbol, ty: Ty) {
    self.tys.insert(name, ty);
  }

  #[inline]
  pub fn var(&self, name: &Symbol) -> Option<&Value> {
    self.vars.get(name)
  }

  #[inline]
  pub fn fun(&self, name: &Symbol) -> Option<&Value> {
    self.funs.get(name)
  }

  #[inline]
  pub fn ty(&self, name: &Symbol) -> Option<&Ty> {
    self.tys.get(name)
  }
}

impl Default for Scope {
  fn default() -> Self {
    Self::new()
  }
}

#[derive(Debug)]
pub struct ScopeMap {
  scopes: std::collections::LinkedList<Scope>,
}

impl ScopeMap {
  #[inline]
  pub const fn new() -> Self {
    Self {
      scopes: std::collections::LinkedList::new(),
    }
  }

  #[inline]
  pub fn scope_entry(&mut self) {
    self.scopes.push_front(Scope::new());
  }

  #[inline]
  pub fn scope_exit(&mut self) {
    self.scopes.pop_front();
  }

  pub fn add_var(&mut self, name: Symbol, value: Value) -> Result<(), String> {
    match self.scopes.front_mut() {
      Some(scope) => scope.add_var(name, value),
      None => Err(format!("The variable `{name}` already exist.")),
    }
  }

  pub fn add_fun(&mut self, name: Symbol, value: Value) -> Result<(), String> {
    match self.scopes.front_mut() {
      Some(scope) => scope.add_fun(name, value),
      None => Err(format!("The function `{name}` already exist.")),
    }
  }

  pub fn set_var(&mut self, name: Symbol, value: Value) -> Result<(), String> {
    match self.scopes.front_mut() {
      Some(scope) => Ok(scope.set_var(name, value)),
      None => Err(format!("set_var error not implemented yet.")),
    }
  }

  pub fn set_fun(&mut self, name: Symbol, value: Value) -> Result<(), String> {
    match self.scopes.front_mut() {
      Some(scope) => Ok(scope.set_fun(name, value)),
      None => Err(format!("set_fun error not implemented yet.")),
    }
  }

  pub fn set_ty(&mut self, name: Symbol, ty: Ty) -> Result<(), String> {
    match self.scopes.front_mut() {
      Some(scope) => Ok(scope.set_ty(name, ty)),
      None => Err(format!("set_fun error not implemented yet.")),
    }
  }

  pub fn var(&self, name: &Symbol) -> Option<&Value> {
    for scope in self.scopes.iter() {
      match scope.var(name) {
        Some(var) => return Some(var),
        None => continue,
      };
    }

    None
  }

  pub fn fun(&self, name: &Symbol) -> Option<&Value> {
    for scope in self.scopes.iter() {
      match scope.fun(name) {
        Some(fun) => return Some(fun),
        None => continue,
      };
    }

    None
  }

  pub fn ty(&self, name: &Symbol) -> Option<&Ty> {
    for scope in self.scopes.iter() {
      match scope.ty(name) {
        Some(ty) => return Some(ty),
        None => continue,
      };
    }

    None
  }
}

impl Default for ScopeMap {
  fn default() -> Self {
    Self::new()
  }
}
