use zo_interner::{DenseId, Symbol};
use zo_span::Span;
use zo_ty::{Mutability, TyId};

/// VALUE AS FLYWEIGHT INDEX (Manifesto: everything is an index).
#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
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

impl DenseId for ValueId {
  #[inline]
  fn from_u32(id: u32) -> Self {
    ValueId(id)
  }

  #[inline]
  fn to_u32(self) -> u32 {
    self.0
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
  Closure,
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
  /// Closure values (fun_name, captures).
  pub closures: Vec<ClosureValue>,
  /// Mapping from ValueId to index in side array
  /// Index into the appropriate side array
  pub indices: Vec<u32>,
}

/// Metadata about a single captured variable.
#[derive(Clone, Copy, Debug)]
pub struct CaptureInfo {
  /// The variable name.
  pub name: Symbol,
  /// The SIR value of the captured variable.
  pub sir_value: ValueId,
  /// Whether the captured variable is mutable (`mut`).
  pub is_mutable: bool,
}

/// A closure value: generated function name + captured values.
#[derive(Clone, Debug)]
pub struct ClosureValue {
  /// Generated function name (e.g. `__closure_0`).
  pub fun_name: Symbol,
  /// Captured variables with metadata.
  pub captures: Vec<CaptureInfo>,
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
      closures: Vec::new(),
      indices: Vec::with_capacity(capacity / 10),
    }
  }

  /// Register a value kind + side-array index pair.
  #[inline(always)]
  fn store(&mut self, kind: Value, idx: u32) -> ValueId {
    let value_id = ValueId(self.kinds.len() as u32);

    self.kinds.push(kind);
    self.indices.push(idx);

    value_id
  }

  #[inline(always)]
  pub fn store_int(&mut self, value: u64) -> ValueId {
    let idx = self.ints.len() as u32;
    self.ints.push(value);
    self.store(Value::Int, idx)
  }

  #[inline(always)]
  pub fn store_float(&mut self, value: f64) -> ValueId {
    let idx = self.floats.len() as u32;
    self.floats.push(value);
    self.store(Value::Float, idx)
  }

  #[inline(always)]
  pub fn store_bool(&mut self, value: bool) -> ValueId {
    let idx = self.bools.len() as u32;
    self.bools.push(value);
    self.store(Value::Bool, idx)
  }

  #[inline(always)]
  pub fn store_string(&mut self, symbol: Symbol) -> ValueId {
    let idx = self.strings.len() as u32;
    self.strings.push(symbol);
    self.store(Value::String, idx)
  }

  #[inline(always)]
  pub fn store_type(&mut self, ty: TyId) -> ValueId {
    let idx = self.types.len() as u32;
    self.types.push(ty);
    self.store(Value::Type, idx)
  }

  #[inline(always)]
  pub fn store_binding(&mut self, name: Symbol, ty: TyId) -> ValueId {
    let idx = self.bindings.len() as u32;
    self.bindings.push((name, ty));
    self.store(Value::Binding, idx)
  }

  #[inline(always)]
  pub fn store_template(&mut self, template_ref: u32) -> ValueId {
    let idx = self.templates.len() as u32;
    self.templates.push(template_ref);
    self.store(Value::Template, idx)
  }

  /// `true` when `vid` resolves to a compile-time scalar
  /// literal (`Int` / `Float` / `Bool` / `Char`, plus
  /// `String` when `with_string` is set). Single source of
  /// truth for the "is this constant-foldable?" gate that
  /// `execute_binop` and the template-interp baker both
  /// consult — without it, the two predicates drifted (the
  /// fold path excluded `String`, the baker included it).
  #[inline]
  pub fn is_scalar_const(&self, vid: ValueId, with_string: bool) -> bool {
    matches!(
      self.kinds.get(vid.0 as usize),
      Some(Value::Int | Value::Float | Value::Bool | Value::Char,),
    ) || (with_string
      && matches!(self.kinds.get(vid.0 as usize), Some(Value::String)))
  }

  #[inline(always)]
  pub fn store_runtime(&mut self, sir_ref: u32) -> ValueId {
    let idx = self.runtimes.len() as u32;
    self.runtimes.push(sir_ref);
    self.store(Value::Runtime, idx)
  }

  #[inline(always)]
  pub fn store_closure(&mut self, cv: ClosureValue) -> ValueId {
    let idx = self.closures.len() as u32;
    self.closures.push(cv);
    self.store(Value::Closure, idx)
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

/// Represents a [`FunctionKind`] — user-defined vs
/// intrinsic (external, empty body) vs closure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FunctionKind {
  UserDefined,
  Intrinsic,
  /// Closure — captures are prepended to params.
  /// `capture_count` distinguishes captures from user params.
  Closure {
    capture_count: u32,
  },
}

/// Represents a [`LocalKind`] — parameter vs local
/// variable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalKind {
  Parameter,
  Variable,
  Constant,
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
  /// Whether this function is intrinsic (empty body).
  pub kind: FunctionKind,
  /// Visibility modifier.
  pub pubness: Pubness,
  /// Generic type parameters: original inference var TyIds.
  /// Empty for non-generic functions. At each call site,
  /// these are substituted with fresh vars.
  pub type_params: Vec<TyId>,
  /// Concrete return type args as resolved Ty values
  /// (e.g. `Result<str, int>` → [Ty::Str, Ty::Int]).
  /// Stored as Ty (not TyId) so they survive cross-module
  /// translation without TyId invalidation.
  pub return_type_args: Vec<zo_ty::Ty>,
  /// `true` when the first parameter was declared as
  /// `mut self`. Set only on apply-context methods —
  /// non-method functions and `self`-only methods are
  /// `false`. Read at every dot-call site to verify the
  /// receiver's binding is `mut`.
  pub mut_self: bool,
  /// Source span of the function's introducer. Propagated
  /// to `Insn::FunDef::span` at emission time so DCE,
  /// unused-fn warnings, and rationale notes can anchor
  /// at the user's `fun` declaration. `Span::ZERO` for
  /// synthetic functions (closures, monomorphized methods)
  /// that have no direct source location.
  pub span: Span,
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
  /// SIR ValueId from the init expression (locals) or
  /// None (params — Load emitted on each reference).
  pub sir_value: Option<ValueId>,
  /// Whether this local is a function parameter.
  pub local_kind: LocalKind,
}
