use zo_interner::interner::symbol::Symbol;
use zo_reporter::{error, Result};
use zo_ty::ty::Ty;
use zo_value::value::Value;

use hashbrown::hash_map::Entry;
use hashbrown::HashMap;

/// The representation of a scope.
#[derive(Debug)]
pub struct Scope {
  /// Stores variables.
  vars: HashMap<Symbol, Value>,
  /// Stores functions.
  funs: HashMap<Symbol, Value>,
  /// Stores types.
  tys: HashMap<Symbol, Ty>,
}

impl Scope {
  /// Creates a new scope.
  #[inline]
  pub fn new() -> Self {
    Self {
      vars: HashMap::with_capacity(0usize),
      funs: HashMap::with_capacity(0usize),
      tys: HashMap::with_capacity(0usize),
    }
  }

  /// Sets a variable from a name and his instance.
  pub fn add_var(&mut self, name: Symbol, value: Value) -> Result<()> {
    match self.vars.entry(name) {
      Entry::Occupied(_) => Err(error::eval::name_clash_var(value.span, name)),
      Entry::Vacant(vars) => {
        vars.insert(value);
        Ok(())
      }
    }
  }

  /// Sets a function from a name and his instance.
  pub fn add_fun(&mut self, name: Symbol, value: Value) -> Result<()> {
    match self.funs.entry(name) {
      Entry::Occupied(_) => Err(error::eval::name_clash_fun(value.span, name)),
      Entry::Vacant(funs) => {
        funs.insert(value);
        Ok(())
      }
    }
  }

  /// Sets a type from a name and his instance.
  pub fn add_ty(&mut self, name: Symbol, ty: Ty) -> Result<()> {
    match self.tys.entry(name) {
      Entry::Occupied(_) => Err(error::eval::name_clash_ty(ty.span, name)),
      Entry::Vacant(tys) => {
        tys.insert(ty);
        Ok(())
      }
    }
  }

  /// Gets a variable from a name.
  #[inline]
  pub fn var(&self, name: &Symbol) -> Option<&Value> {
    self.vars.get(name)
  }

  /// Gets a function from a name.
  #[inline]
  pub fn fun(&self, name: &Symbol) -> Option<&Value> {
    self.funs.get(name)
  }

  /// Gets a type from a name.
  #[inline]
  pub fn ty(&self, name: &Symbol) -> Option<&Ty> {
    self.tys.get(name)
  }
}

impl Default for Scope {
  #[inline]
  fn default() -> Self {
    Self::new()
  }
}

/// The representation of a scope map.
#[derive(Debug)]
pub struct ScopeMap {
  /// A set of scopes.
  scopes: std::collections::LinkedList<Scope>,
}

impl ScopeMap {
  /// Creates a new scope map.
  #[inline]
  pub const fn new() -> Self {
    Self {
      scopes: std::collections::LinkedList::new(),
    }
  }

  /// Adds a new scope in the scope map.
  #[inline]
  pub fn scope_entry(&mut self) {
    self.scopes.push_front(Scope::new());
  }

  /// Deletes the last scope in the scope map.
  #[inline]
  pub fn scope_exit(&mut self) {
    self.scopes.pop_front();
  }

  /// Adds a variable to the scope map from a name and his instance.
  pub fn add_var(&mut self, name: Symbol, value: Value) -> Result<()> {
    match self.scopes.front_mut() {
      Some(scope) => scope.add_var(name, value),
      None => Err(error::eval::name_clash_var(value.span, name)),
    }
  }

  /// Adds a function to the scope map from a name and his instance.
  pub fn add_fun(&mut self, name: Symbol, value: Value) -> Result<()> {
    match self.scopes.front_mut() {
      Some(scope) => scope.add_fun(name, value),
      None => Err(error::eval::name_clash_fun(value.span, name)),
    }
  }

  /// Adds a type in the scope map from a name and his instance.
  pub fn add_ty(&mut self, name: Symbol, ty: Ty) -> Result<()> {
    match self.scopes.front_mut() {
      Some(scope) => scope.add_ty(name, ty),
      None => Err(error::eval::name_clash_ty(ty.span, name)),
    }
  }

  /// Gets a variable in the scope map from a name.
  pub fn var(&self, name: &Symbol) -> Option<&Value> {
    for scope in self.scopes.iter() {
      match scope.var(name) {
        Some(var) => return Some(var),
        None => continue,
      };
    }

    None
  }

  /// Gets a function in the scope map from a name.
  pub fn fun(&self, name: &Symbol) -> Option<&Value> {
    for scope in self.scopes.iter() {
      match scope.fun(name) {
        Some(fun) => return Some(fun),
        None => continue,
      };
    }

    None
  }

  /// Gets a type in the scope map from a name.
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
  #[inline]
  fn default() -> Self {
    Self::new()
  }
}
