use hashbrown::hash_map::Entry;
use hashbrown::HashMap;

pub trait Name: Sized {
  fn name(&self) -> String;
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Scope<V>
where
  V: Name,
{
  tys: HashMap<String, V>,
  vars: HashMap<String, V>,
  funs: HashMap<String, V>,
}

impl<V> Scope<V>
where
  V: Name,
{
  #[inline]
  pub fn new() -> Self {
    Self {
      tys: HashMap::new(),
      vars: HashMap::new(),
      funs: HashMap::new(),
    }
  }

  pub fn add_var(&mut self, var: V) -> Result<(), String> {
    match self.vars.entry(var.name()) {
      Entry::Occupied(_) => {
        Err(format!("The variable `{}` already exist.", var.name()))
      }
      Entry::Vacant(vars) => {
        vars.insert(var);
        Ok(())
      }
    }
  }

  pub fn add_fun(&mut self, fun: V) -> Result<(), String> {
    match self.funs.entry(fun.name()) {
      Entry::Occupied(_) => {
        Err(format!("The function `{}` already exist.", fun.name()))
      }
      Entry::Vacant(funs) => {
        funs.insert(fun);
        Ok(())
      }
    }
  }

  pub fn add_ty(&mut self, ty: V) -> Result<(), String> {
    match self.tys.entry(ty.name()) {
      Entry::Occupied(_) => {
        Err(format!("The type `{}` already exist.", ty.name()))
      }
      Entry::Vacant(tys) => {
        tys.insert(ty);
        Ok(())
      }
    }
  }

  #[inline]
  pub fn var(&self, name: &str) -> Option<&V> {
    self.vars.get(name)
  }

  #[inline]
  pub fn fun(&self, name: &str) -> Option<&V> {
    self.funs.get(name)
  }

  #[inline]
  pub fn ty(&self, name: &str) -> Option<&V> {
    self.tys.get(name)
  }
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Scopemap<V>
where
  V: Name,
{
  scopes: std::collections::LinkedList<Scope<V>>,
}

impl<V> Scopemap<V>
where
  V: Name,
{
  #[inline]
  pub fn new() -> Self {
    Self {
      scopes: std::collections::LinkedList::new(),
    }
  }

  pub fn scope_entry(&mut self) {
    self.scopes.push_front(Scope::new());
  }

  pub fn scope_exit(&mut self) {
    if self.scopes.pop_front().is_some() {}
  }

  pub fn add_var(&mut self, var: V) -> Result<(), String> {
    match self.scopes.front_mut() {
      Some(scope) => scope.add_var(var),
      None => Err(format!("The variable `{}` already exist.", var.name())),
    }
  }

  pub fn add_fun(&mut self, fun: V) -> Result<(), String> {
    match self.scopes.front_mut() {
      Some(scope) => scope.add_fun(fun),
      None => Err(format!("The function `{}` already exist.", fun.name())),
    }
  }

  pub fn add_ty(&mut self, ty: V) -> Result<(), String> {
    match self.scopes.front_mut() {
      Some(scope) => scope.add_ty(ty),
      None => Err(format!("The type `{}` already exist.", ty.name())),
    }
  }

  pub fn var(&self, name: &str) -> Option<&V> {
    for scope in self.scopes.iter() {
      match scope.var(name) {
        Some(var) => return Some(var),
        None => continue,
      };
    }

    None
  }

  pub fn fun(&self, name: &str) -> Option<&V> {
    for scope in self.scopes.iter() {
      match scope.fun(name) {
        Some(fun) => return Some(fun),
        None => continue,
      };
    }

    None
  }

  pub fn ty(&self, name: &str) -> Option<&V> {
    for scope in self.scopes.iter() {
      match scope.ty(name) {
        Some(ty) => return Some(ty),
        None => continue,
      };
    }

    None
  }
}
