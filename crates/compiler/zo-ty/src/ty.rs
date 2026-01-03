use zo_interner::Symbol;

use rustc_hash::FxHashMap as HashMap;

/// A type identifier - an index into the type table.
/// This is a newtype wrapper around u32 for type safety.
/// Can represent both concrete types (int, bool) and type variables (α, β).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TyId(pub u32);

// Now Ty can derive Hash because it only contains simple types and IDs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ty {
  Error,
  Unit,
  Int {
    signed: bool,
    width: IntWidth,
  },
  Float(FloatWidth),
  Bool,
  Bytes,
  Char,
  Str,

  /// A struct type, identified by its name.
  Struct(Symbol),

  /// An enum type, identified by its name.
  Enum(Symbol),

  /// A function type - points to interned storage
  Fun(FunTyId),

  /// A reference type - points to interned storage
  Ref(RefTyId),

  /// An array type - points to interned storage
  Array(ArrayTyId),

  /// Template Fragment type with unique ID for each template literal
  Fragment(FragmentTyId),

  /// The template type marker </> used in type annotations
  Template,

  /// An inference variable representing a type that is not yet known.
  /// In pure W algorithm, all type variables are uniform.
  Infer(InferVarId),

  /// The type of types (for type expressions like `s32`)
  Type,

  Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntWidth {
  S8,
  S16,
  S32,
  S64,
  U8,
  U16,
  U32,
  U64,
  Arch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FloatWidth {
  F32,
  F64,
  Arch,
}

// Lightweight IDs for interned composite types
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunTyId(pub u32);

// The actual structural data for complex types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunTy {
  // Start index in the global param_tys array
  pub param_start: u32,
  // Number of parameters
  pub param_count: u32,
  // Type ID for return type
  pub return_ty: TyId,
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RefTyId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RefTy {
  pub is_mut: bool,
  // Type ID for inner type
  pub inner_ty: TyId,
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArrayTyId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArrayTy {
  /// Element type ID
  pub elem_ty: TyId,
  /// Size of the array - None for dynamic, Some(n) for fixed size.
  pub size: Option<u32>,
}

/// An identifier for inference variables - distinct from TyId.
/// This makes the type system's invariants explicit.
/// In pure W algorithm, all type variables are uniform.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InferVarId(pub u32);

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FragmentTyId(pub u32);

/// Type table using structure of arrays for cache-friendly access.
/// This stores all compound types encountered during execution.
#[derive(Debug, Default)]
pub struct TyTable {
  /// Array types stored contiguously.
  pub array_types: Vec<ArrayTy>,
  /// Array type interning map for O(1) deduplication.
  array_intern: HashMap<ArrayTy, ArrayTyId>,
  /// Fun types stored contiguously.  
  pub function_types: Vec<FunTy>,
  /// Fun type interning map for O(1) deduplication.
  fun_intern: HashMap<(Vec<TyId>, TyId), FunTyId>,
  /// Reference types stored contiguously.
  pub ref_types: Vec<RefTy>,
  /// Reference type interning map for O(1) deduplication.
  ref_intern: HashMap<RefTy, RefTyId>,
  /// Global array for all function parameter types.
  pub param_tys: Vec<TyId>,
}
impl TyTable {
  pub fn new() -> Self {
    Self::default()
  }

  /// Intern an array type, returning its ID.
  /// Uses HashMap for O(1) deduplication.
  pub fn intern_array(
    &mut self,
    elem_ty: TyId,
    size: Option<u32>,
  ) -> ArrayTyId {
    let array_ty = ArrayTy { elem_ty, size };

    // Check if already interned in O(1)
    if let Some(&id) = self.array_intern.get(&array_ty) {
      return id;
    }

    let id = ArrayTyId(self.array_types.len() as u32);
    self.array_types.push(array_ty);
    self.array_intern.insert(array_ty, id);
    id
  }

  /// Get an array type by ID.
  pub fn array(&self, id: ArrayTyId) -> Option<&ArrayTy> {
    self.array_types.get(id.0 as usize)
  }

  /// Intern a function type, returning its ID.
  /// Uses HashMap for O(1) deduplication.
  pub fn intern_fun(
    &mut self,
    param_tys: Vec<TyId>,
    return_ty: TyId,
  ) -> FunTyId {
    // Check if already interned using params + return as key
    let key = (param_tys.clone(), return_ty);

    if let Some(&id) = self.fun_intern.get(&key) {
      return id;
    }

    let param_start = self.param_tys.len() as u32;
    let param_count = param_tys.len() as u32;

    // Add parameters to global array
    self.param_tys.extend(&param_tys);

    let fun_ty = FunTy {
      param_start,
      param_count,
      return_ty,
    };

    let id = FunTyId(self.function_types.len() as u32);

    self.function_types.push(fun_ty);
    self.fun_intern.insert(key, id);
    id
  }

  /// Get a function type by ID.
  pub fn fun(&self, id: &FunTyId) -> Option<&FunTy> {
    self.function_types.get(id.0 as usize)
  }

  /// Get the parameter types for a function.
  pub fn fun_params(&self, fun: &FunTy) -> &[TyId] {
    let start = fun.param_start as usize;
    let end = (fun.param_start + fun.param_count) as usize;

    &self.param_tys[start..end]
  }

  /// Intern a reference type, returning its ID.
  /// Uses HashMap for O(1) deduplication.
  pub fn intern_ref(&mut self, is_mut: bool, inner_ty: TyId) -> RefTyId {
    let ref_ty = RefTy { is_mut, inner_ty };

    // Check if already interned in O(1)
    if let Some(&id) = self.ref_intern.get(&ref_ty) {
      return id;
    }

    let id = RefTyId(self.ref_types.len() as u32);

    self.ref_types.push(ref_ty);
    self.ref_intern.insert(ref_ty, id);
    id
  }

  /// Get a reference type by ID.
  pub fn reference(&self, id: RefTyId) -> Option<&RefTy> {
    self.ref_types.get(id.0 as usize)
  }
}
