use crate::ConstFold;

use zo_interner::{Interner, Symbol};
use zo_span::Span;
use zo_ty::{IntWidth, Ty};
use zo_value::{ValueId, ValueStorage};

pub const SPAN: Span = Span::ZERO;

/// Default types for tests.
pub const U64: Ty = Ty::Int {
  signed: false,
  width: IntWidth::U64,
};
pub const S64: Ty = Ty::Int {
  signed: true,
  width: IntWidth::S64,
};
pub const F64: Ty = Ty::Float(zo_ty::FloatWidth::F64);
pub const BOOL: Ty = Ty::Bool;

/// Test harness that owns a [`ValueStorage`] and provides
/// convenience methods for storing values and creating a
/// [`ConstFold`] instance.
pub struct Harness {
  pub values: ValueStorage,
  pub interner: Interner,
}
impl Harness {
  pub fn new() -> Self {
    Self {
      values: ValueStorage::new(64),
      interner: Interner::new(),
    }
  }

  pub fn int(&mut self, value: u64) -> ValueId {
    self.values.store_int(value)
  }

  pub fn float(&mut self, value: f64) -> ValueId {
    self.values.store_float(value)
  }

  pub fn bool(&mut self, value: bool) -> ValueId {
    self.values.store_bool(value)
  }

  pub fn string(&mut self, value: &str) -> (ValueId, Symbol) {
    let sym = self.interner.intern(value);
    let id = self.values.store_string(sym);

    (id, sym)
  }

  pub fn runtime(&mut self) -> ValueId {
    self.values.store_runtime(0)
  }

  pub fn fold(&mut self) -> ConstFold<'_> {
    ConstFold::new(&self.values, &mut self.interner)
  }
}
