use super::ty::Ty;

use zo_core::interner::symbol::Symbol;

use hashbrown::hash_map::Entry;
use hashbrown::HashMap;

#[derive(Clone, Default, Debug, PartialEq)]
pub struct TyCtx<'ctx> {
  vars: HashMap<&'ctx Symbol, Ty>,
  tys: HashMap<&'ctx Symbol, Ty>,
  funs: HashMap<&'ctx Symbol, (Ty, Vec<Ty>)>,
}

impl<'ctx> TyCtx<'ctx> {
  #[inline]
  pub fn new() -> Self {
    Self {
      tys: HashMap::with_capacity(0usize),
      vars: HashMap::with_capacity(0usize),
      funs: HashMap::with_capacity(0usize),
    }
  }

  pub fn add_var(
    &mut self,
    symbol: &'ctx Symbol,
    var: Ty,
  ) -> Result<(), String> {
    match self.vars.entry(symbol) {
      Entry::Occupied(_) => {
        Err(format!("The variable `{}` already exist.", symbol)) // VarClash.
      }
      Entry::Vacant(vars) => {
        vars.insert(var);
        Ok(())
      }
    }
  }

  pub fn add_fun(
    &mut self,
    symbol: &'ctx Symbol,
    fun: (Ty, Vec<Ty>),
  ) -> Result<(), String> {
    match self.funs.entry(symbol) {
      Entry::Occupied(_) => {
        Err(format!("The function `{}` already exist.", symbol)) // FunClash.
      }
      Entry::Vacant(funs) => {
        funs.insert(fun);
        Ok(())
      }
    }
  }

  pub fn add_ty(&mut self, symbol: &'ctx Symbol, ty: Ty) -> Result<(), String> {
    match self.tys.entry(symbol) {
      Entry::Occupied(_) => {
        Err(format!("The type `{}` already exist.", symbol)) // TyClash.
      }
      Entry::Vacant(tys) => {
        tys.insert(ty);
        Ok(())
      }
    }
  }

  #[inline]
  pub fn var(&self, var: &'ctx Symbol) -> Option<&Ty> {
    self.vars.get(var)
  }

  #[inline]
  pub fn fun(&self, fun: &'ctx Symbol) -> Option<&(Ty, Vec<Ty>)> {
    self.funs.get(fun)
  }

  #[inline]
  pub fn ty(&self, ty: &'ctx Symbol) -> Option<&Ty> {
    self.tys.get(ty)
  }
}
