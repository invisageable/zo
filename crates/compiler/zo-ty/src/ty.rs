use zo_interner::Symbol;

use rustc_hash::FxHashMap as HashMap;

/// Represents [`Mutability`] flag for variables/refs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mutability {
  No,
  Yes,
}

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

  /// A struct type, identified by its interned ID.
  Struct(StructTyId),

  /// An enum type, identified by its interned ID.
  Enum(EnumTyId),

  /// A function type - points to interned storage
  Fun(FunTyId),

  /// A reference type - points to interned storage
  Ref(RefTyId),

  /// An array type - points to interned storage
  Array(ArrayTyId),

  /// A tuple type - points to interned storage
  Tuple(TupleTyId),

  /// Template Fragment type with unique ID for each template literal
  Fragment(FragmentTyId),

  /// The template type marker </> used in type annotations
  Template,

  /// An inference variable representing a type that is not yet known.
  /// In pure W algorithm, all type variables are uniform.
  Infer(InferVarId),

  /// A named type parameter: `$T`, `$A`, `$K`, etc.
  /// Used in generic function/struct/enum definitions.
  Param(Symbol),

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
  pub mutability: Mutability,
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

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TupleTyId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TupleTy {
  /// Start index in the global tuple_elem_tys array.
  pub elem_start: u32,
  /// Number of elements.
  pub elem_count: u32,
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StructTyId(pub u32);

/// Struct type: name + field range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StructTy {
  /// Name of the struct.
  pub name: Symbol,
  /// Start index in the global struct_fields array.
  pub field_start: u32,
  /// Number of fields.
  pub field_count: u32,
}

/// A single struct field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StructField {
  /// Field name.
  pub name: Symbol,
  /// Field type.
  pub ty_id: TyId,
  /// Whether this field has a default value.
  pub has_default: bool,
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnumTyId(pub u32);

/// Enum type: name + variant range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnumTy {
  /// Name of the enum.
  pub name: Symbol,
  /// Start index in the global enum_variants array.
  pub variant_start: u32,
  /// Number of variants.
  pub variant_count: u32,
}

/// A single enum variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnumVariant {
  /// Variant name.
  pub name: Symbol,
  /// Discriminant value (auto-incremented or explicit).
  pub discriminant: u32,
  /// Start index in the global variant_fields array.
  pub field_start: u32,
  /// Number of payload fields (0 for unit variants).
  pub field_count: u32,
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
  /// Tuple types stored contiguously.
  pub tuple_types: Vec<TupleTy>,
  /// Tuple type interning map for O(1) deduplication.
  tuple_intern: HashMap<Vec<TyId>, TupleTyId>,
  /// Global array for all function parameter types.
  pub param_tys: Vec<TyId>,
  /// Global array for all tuple element types.
  pub tuple_elem_tys: Vec<TyId>,
  /// Enum types stored contiguously.
  pub enum_types: Vec<EnumTy>,
  /// Enum type interning map for O(1) deduplication.
  enum_intern: HashMap<Symbol, EnumTyId>,
  /// All enum variants across all enums.
  pub enum_variants: Vec<EnumVariant>,
  /// Global array for variant payload field types.
  pub variant_field_tys: Vec<TyId>,
  /// Struct types stored contiguously.
  pub struct_types: Vec<StructTy>,
  /// Struct type interning map for O(1) deduplication.
  struct_intern: HashMap<Symbol, StructTyId>,
  /// All struct fields across all structs.
  pub struct_fields: Vec<StructField>,
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
  pub fn intern_ref(
    &mut self,
    mutability: Mutability,
    inner_ty: TyId,
  ) -> RefTyId {
    let ref_ty = RefTy {
      mutability,
      inner_ty,
    };

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

  /// Intern a tuple type, returning its ID.
  /// Uses HashMap for O(1) deduplication.
  pub fn intern_tuple(&mut self, elem_tys: Vec<TyId>) -> TupleTyId {
    if let Some(&id) = self.tuple_intern.get(&elem_tys) {
      return id;
    }

    let elem_start = self.tuple_elem_tys.len() as u32;
    let elem_count = elem_tys.len() as u32;

    self.tuple_elem_tys.extend(&elem_tys);

    let tuple_ty = TupleTy {
      elem_start,
      elem_count,
    };

    let id = TupleTyId(self.tuple_types.len() as u32);

    self.tuple_types.push(tuple_ty);
    self.tuple_intern.insert(elem_tys, id);

    id
  }

  /// Get a tuple type by ID.
  pub fn tuple(&self, id: TupleTyId) -> Option<&TupleTy> {
    self.tuple_types.get(id.0 as usize)
  }

  /// Get the element types of a tuple.
  pub fn tuple_elems(&self, tup: &TupleTy) -> &[TyId] {
    let start = tup.elem_start as usize;
    let end = (tup.elem_start + tup.elem_count) as usize;

    &self.tuple_elem_tys[start..end]
  }

  /// Intern an enum type, returning its ID.
  pub fn intern_enum(
    &mut self,
    name: Symbol,
    variants: &[(Symbol, u32, Vec<TyId>)],
  ) -> EnumTyId {
    if let Some(&id) = self.enum_intern.get(&name) {
      return id;
    }

    let variant_start = self.enum_variants.len() as u32;

    for (vname, disc, fields) in variants {
      let field_start = self.variant_field_tys.len() as u32;

      let field_count = fields.len() as u32;

      self.variant_field_tys.extend(fields);

      self.enum_variants.push(EnumVariant {
        name: *vname,
        discriminant: *disc,
        field_start,
        field_count,
      });
    }

    let enum_ty = EnumTy {
      name,
      variant_start,
      variant_count: variants.len() as u32,
    };

    let id = EnumTyId(self.enum_types.len() as u32);

    self.enum_types.push(enum_ty);
    self.enum_intern.insert(name, id);

    id
  }

  /// Get an enum type by ID.
  pub fn enum_ty(&self, id: EnumTyId) -> Option<&EnumTy> {
    self.enum_types.get(id.0 as usize)
  }

  /// Get the variants of an enum.
  pub fn enum_variants(&self, e: &EnumTy) -> &[EnumVariant] {
    let start = e.variant_start as usize;
    let end = start + e.variant_count as usize;

    &self.enum_variants[start..end]
  }

  /// Get the field types of a variant.
  pub fn variant_fields(&self, v: &EnumVariant) -> &[TyId] {
    let start = v.field_start as usize;
    let end = start + v.field_count as usize;

    &self.variant_field_tys[start..end]
  }

  /// Intern a struct type, returning its ID.
  pub fn intern_struct(
    &mut self,
    name: Symbol,
    fields: &[(Symbol, TyId, bool)],
  ) -> StructTyId {
    if let Some(&id) = self.struct_intern.get(&name) {
      return id;
    }

    let field_start = self.struct_fields.len() as u32;

    for &(fname, fty, has_default) in fields {
      self.struct_fields.push(StructField {
        name: fname,
        ty_id: fty,
        has_default,
      });
    }

    let struct_ty = StructTy {
      name,
      field_start,
      field_count: fields.len() as u32,
    };

    let id = StructTyId(self.struct_types.len() as u32);

    self.struct_types.push(struct_ty);
    self.struct_intern.insert(name, id);

    id
  }

  /// Get a struct type by ID.
  pub fn struct_ty(&self, id: StructTyId) -> Option<&StructTy> {
    self.struct_types.get(id.0 as usize)
  }

  /// Look up a struct by name.
  pub fn struct_intern_lookup(&self, name: Symbol) -> Option<&StructTyId> {
    self.struct_intern.get(&name)
  }

  /// Look up an enum by name.
  pub fn enum_intern_lookup(&self, name: Symbol) -> Option<&EnumTyId> {
    self.enum_intern.get(&name)
  }

  /// Get the fields of a struct.
  pub fn struct_fields(&self, s: &StructTy) -> &[StructField] {
    let start = s.field_start as usize;
    let end = start + s.field_count as usize;

    &self.struct_fields[start..end]
  }
}
