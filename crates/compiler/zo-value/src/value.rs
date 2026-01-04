use zo_interner::Symbol;
use zo_ty::TyId;

/// VALUE AS FLYWEIGHT INDEX (Manifesto: everything is an index).
#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ValueId(pub u32);
impl std::ops::Deref for ValueId {
  type Target = u32;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}
impl std::fmt::Display for ValueId {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.0)
  }
}

/// Rerpresents the [`Value`] kind enumeration.
///
/// it helps to know what type of value this [`ValueId`] represents.
#[derive(Copy, Clone, Debug)]
pub enum Value {
  Unit,
  Bool,
  Int,
  Float,
  String,
  Char,
  Bytes,
  Type,
  Array,
  Template,
  Runtime,
  Binding,
}

/// Represents a [`ValueStorage`] instance.
pub struct ValueStorage {
  ///  1 byte per value
  pub kinds: Vec<Value>,
  /// Bool values.
  pub bools: Vec<bool>,
  /// Integer literals (always positive).
  pub ints: Vec<u64>,
  /// Float constants.
  pub floats: Vec<f64>,
  /// String/Bytes values (interned).
  pub strings: Vec<Symbol>,
  /// Character constants.
  pub chars: Vec<char>,
  /// Type values.
  pub types: Vec<TyId>,
  /// Array values.
  pub arrays: Vec<u32>,
  /// Template fragments.
  pub templates: Vec<u32>,
  // Runtime SIR references.
  pub runtimes: Vec<u32>,
  // Binding name + type.
  pub bindings: Vec<(Symbol, TyId)>,
  /// Mapping from ValueId to index in side array
  /// Index into the appropriate side array
  pub indices: Vec<u32>,
}
impl ValueStorage {
  pub fn new(capacity: usize) -> Self {
    Self {
      kinds: Vec::with_capacity(capacity / 10),
      bools: Vec::new(),
      ints: Vec::new(),
      floats: Vec::new(),
      strings: Vec::new(),
      chars: Vec::new(),
      types: Vec::new(),
      arrays: Vec::new(),
      templates: Vec::new(),
      runtimes: Vec::new(),
      bindings: Vec::new(),
      indices: Vec::with_capacity(capacity / 10),
    }
  }

  /// Store an integer literal and return its [`ValueId`].
  #[inline(always)]
  pub fn store_int(&mut self, value: u64) -> ValueId {
    let idx = self.ints.len() as u32;

    self.ints.push(value);

    let value_id = ValueId(self.kinds.len() as u32);

    self.kinds.push(Value::Int);
    self.indices.push(idx);

    value_id
  }

  /// Store an float literal and return its [`ValueId`].
  #[inline(always)]
  pub fn store_float(&mut self, value: f64) -> ValueId {
    let idx = self.floats.len() as u32;

    self.floats.push(value);

    let value_id = ValueId(self.kinds.len() as u32);

    self.kinds.push(Value::Float);
    self.indices.push(idx);

    value_id
  }

  /// Store a boolean value and return its [`ValueId`].
  #[inline(always)]
  pub fn store_bool(&mut self, value: bool) -> ValueId {
    let idx = self.bools.len() as u32;

    self.bools.push(value);

    let value_id = ValueId(self.kinds.len() as u32);

    self.kinds.push(Value::Bool);
    self.indices.push(idx);

    value_id
  }

  /// Store a string value (as Symbol) and return its [`ValueId`].
  #[inline(always)]
  pub fn store_string(&mut self, symbol: Symbol) -> ValueId {
    let idx = self.strings.len() as u32;

    self.strings.push(symbol);

    let value_id = ValueId(self.kinds.len() as u32);

    self.kinds.push(Value::String);
    self.indices.push(idx);

    value_id
  }

  /// Stores a type value and return its [`ValueId`].
  #[inline(always)]
  pub fn store_type(&mut self, ty: TyId) -> ValueId {
    let idx = self.types.len() as u32;

    self.types.push(ty);

    let value_id = ValueId(self.kinds.len() as u32);

    self.kinds.push(Value::Type);
    self.indices.push(idx);

    value_id
  }

  /// Stores a binding (identifier with type).
  #[inline(always)]
  pub fn store_binding(&mut self, name: Symbol, ty: TyId) -> ValueId {
    let idx = self.bindings.len() as u32;

    self.bindings.push((name, ty));

    let value_id = ValueId(self.kinds.len() as u32);

    self.kinds.push(Value::Binding);
    self.indices.push(idx);

    value_id
  }

  /// Stores a template value and return its [`ValueId`].
  #[inline(always)]
  pub fn store_template(&mut self, template_ref: u32) -> ValueId {
    let idx = self.templates.len() as u32;

    self.templates.push(template_ref);

    let value_id = ValueId(self.kinds.len() as u32);

    self.kinds.push(Value::Template);
    self.indices.push(idx);

    value_id
  }

  /// Stores a runtime value (placeholder for SIR reference).
  #[inline(always)]
  pub fn store_runtime(&mut self, sir_ref: u32) -> ValueId {
    let idx = self.runtimes.len() as u32;

    self.runtimes.push(sir_ref);

    let value_id = ValueId(self.kinds.len() as u32);

    self.kinds.push(Value::Runtime);
    self.indices.push(idx);

    value_id
  }

  /// Gets the next [`ValueId`] that will be allocated.
  #[inline(always)]
  pub fn next_value_id(&self) -> u32 {
    self.kinds.len() as u32
  }
}

/// Represents a [`Pubness`] flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pubness {
  No,
  Yes,
}

/// Represents [`Mutability`] flag for variables.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mutability {
  No,
  Yes,
}

/// Represents a [`FunDef`] entry instance.
#[derive(Clone, Debug)]
pub struct FunDef {
  /// The name of the function.
  pub name: Symbol,
  /// The parameters of the function.
  pub params: Vec<(Symbol, TyId)>,
  /// The return type.
  pub return_ty: TyId,
  /// The id of the block entry.
  pub body_start: u32,
}

/// Represents a [`Local`] variable entry instance.
#[derive(Clone, Copy, Debug)]
pub struct Local {
  /// The name of the local.
  pub name: Symbol,
  /// The id of the type.
  pub ty_id: TyId,
  /// The id of the value.
  pub value_id: ValueId,
  /// The mutability flag.
  pub mutability: Mutability,
  /// The pubness flag.
  pub pubness: Pubness,
}
