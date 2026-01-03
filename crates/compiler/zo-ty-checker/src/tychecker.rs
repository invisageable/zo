use zo_error::{Error, ErrorKind};
use zo_interner::{Interner, Symbol};
use zo_reporter::report_error;
use zo_sir::{BinOp, UnOp};
use zo_span::Span;
use zo_ty::FloatWidth;
use zo_ty::{InferVarId, IntWidth, Ty, TyId, TyTable};

use rustc_hash::FxHashMap as HashMap;

/// Type scheme for let-polymorphism (∀α. τ)
/// Represents a polymorphic type with quantified type variables
#[derive(Debug, Clone)]
pub struct TyScheme {
  /// Quantified type variables (the ∀α part)
  pub quantified: Vec<InferVarId>,
  /// The actual type (the τ part)
  pub ty: TyId,
}

/// Type checker implementing Hindley-Milner W algorithm
pub struct TyChecker {
  /// Counter for fresh type IDs (for all types in tys)
  next_ty_id: u32,
  /// Counter for fresh inference variable IDs
  next_infer_var: InferVarId,
  /// Type storage - structure of arrays (Data-oriented design)
  /// Each TyId indexes into this array
  tys: Vec<Ty>,
  /// Type interning map for O(1) deduplication
  /// Maps concrete Ty to its canonical TyId
  intern_map: HashMap<Ty, TyId>,
  /// Type table for compound types (functions, arrays, refs)
  pub ty_table: TyTable,
  /// Substitution environment (for W algorithm)
  /// Maps inference variable ID to resolved type
  substitutions: HashMap<InferVarId, TyId>,
  /// Level for each inference variable (for efficient generalization)
  /// Maps inference variable ID to its creation level
  var_levels: HashMap<InferVarId, u32>,
  /// Current scope level (incremented on push_scope, decremented on pop_scope)
  current_level: u32,
  /// Variable to type mapping (current scope)
  ty_env: HashMap<Symbol, TyId>,
  /// Stack of type environments for nested scopes
  ty_env_stack: Vec<HashMap<Symbol, TyId>>,
  /// Let-polymorphism: Track which type variables can be generalized
  /// Maps a let-bound variable to its generic type scheme
  poly_schemes: HashMap<Symbol, TyScheme>,
  /// Type aliases: maps alias name to underlying type
  /// e.g., "type FooBar = u32" stores FooBar -> u32
  ty_aliases: HashMap<Symbol, TyId>,
  /// Stack of type alias environments for nested scopes
  type_aliases_stack: Vec<HashMap<Symbol, TyId>>,
}
impl TyChecker {
  /// Create a new type checker with pre-allocated capacity
  pub fn new() -> Self {
    Self {
      next_ty_id: 0,
      next_infer_var: InferVarId(0),
      tys: Vec::with_capacity(1024),
      intern_map: HashMap::default(),
      ty_table: TyTable::new(),
      substitutions: HashMap::default(),
      var_levels: HashMap::default(),
      current_level: 0,
      ty_env: HashMap::default(),
      ty_env_stack: Vec::new(),
      poly_schemes: HashMap::default(),
      ty_aliases: HashMap::default(),
      type_aliases_stack: Vec::new(),
    }
  }

  /// Create a fresh type variable (α, β, γ, ...)
  /// In pure W algorithm, all type variables are uniform
  pub fn fresh_var(&mut self) -> TyId {
    let var_id = self.next_infer_var;
    self.next_infer_var = InferVarId(self.next_infer_var.0 + 1);

    // Record the level at which this variable was created
    self.var_levels.insert(var_id, self.current_level);

    // Use intern_ty for consistency
    self.intern_ty(Ty::Infer(var_id))
  }

  /// Get or create the unit type
  pub fn unit_type(&mut self) -> TyId {
    self.intern_ty(Ty::Unit)
  }

  /// Get or create the bool type
  pub fn bool_type(&mut self) -> TyId {
    self.intern_ty(Ty::Bool)
  }

  /// Get or create the f32 type
  pub fn f32_type(&mut self) -> TyId {
    self.intern_ty(Ty::Float(FloatWidth::F32))
  }

  /// Get or create the f64 type
  pub fn f64_type(&mut self) -> TyId {
    self.intern_ty(Ty::Float(FloatWidth::F64))
  }

  /// Get or create the s32 type
  pub fn s32_type(&mut self) -> TyId {
    self.intern_ty(Ty::Int {
      signed: true,
      width: IntWidth::S32,
    })
  }

  /// Get or create the u32 type
  pub fn u32_type(&mut self) -> TyId {
    self.intern_ty(Ty::Int {
      signed: false,
      width: IntWidth::U32,
    })
  }

  /// Get or create the char type
  pub fn char_type(&mut self) -> TyId {
    self.intern_ty(Ty::Char)
  }

  /// Get or create the str type
  pub fn str_type(&mut self) -> TyId {
    self.intern_ty(Ty::Str)
  }

  /// Get or create the default int type (s32)
  pub fn int_type(&mut self) -> TyId {
    self.s32_type()
  }

  /// Get or create the type type (the type of types)
  pub fn type_type(&mut self) -> TyId {
    self.intern_ty(Ty::Type)
  }

  /// Get or create the error type (for error recovery)
  pub fn error_type(&mut self) -> TyId {
    self.intern_ty(Ty::Error)
  }

  /// Get or create the template type
  pub fn template_ty(&mut self) -> TyId {
    self.intern_ty(Ty::Template)
  }

  /// Intern a type - deduplicates and returns existing if already present
  /// Uses HashMap for O(1) lookup instead of O(n) linear scan
  pub fn intern_ty(&mut self, kind: Ty) -> TyId {
    // Don't intern inference variables - each is unique
    if matches!(kind, Ty::Infer(_)) {
      let id = TyId(self.next_ty_id);

      self.next_ty_id += 1;

      // Ensure tys has enough space
      if self.tys.len() <= id.0 as usize {
        self.tys.resize(id.0 as usize + 1, Ty::Error);
      }

      self.tys[id.0 as usize] = kind;

      return id;
    }

    // Check if this concrete type already exists in O(1)
    if let Some(id) = self.intern_map.get(&kind) {
      return *id;
    }

    // Create new type
    let id = TyId(self.next_ty_id);

    self.next_ty_id += 1;

    // Ensure tys has enough space
    if self.tys.len() <= id.0 as usize {
      self.tys.resize(id.0 as usize + 1, Ty::Error);
    }

    self.tys[id.0 as usize] = kind;

    self.intern_map.insert(kind, id);

    id
  }

  /// Resolve a TyId to its underlying Ty (mainly for tests)
  pub fn resolve_ty(&self, ty_id: TyId) -> Ty {
    if (ty_id.0 as usize) < self.tys.len() {
      self.tys[ty_id.0 as usize]
    } else {
      Ty::Error
    }
  }

  /// Gets the kind of a type.
  pub fn kind(&self, ty: TyId) -> Option<&Ty> {
    self.tys.get(ty.0 as usize)
  }

  // === PHASE 1.2: UNIFICATION ENGINE ===

  /// Unify two types (Hindley-Milner unification)
  /// Returns the unified type if successful, None if type mismatch
  pub fn unify(&mut self, t1: TyId, t2: TyId, span: Span) -> Option<TyId> {
    let repr1 = self.resolve_id(t1);
    let repr2 = self.resolve_id(t2);

    // If already unified to same representative
    if repr1 == repr2 {
      return Some(repr1);
    }

    let ty1 = self.tys[repr1.0 as usize];
    let ty2 = self.tys[repr2.0 as usize];

    match (ty1, ty2) {
      // One is an inference variable
      (Ty::Infer(var), _) => {
        if self.occurs_check(var, repr2) {
          report_error(Error::new(ErrorKind::InfiniteType, span));
          return None;
        }

        self.substitutions.insert(var, repr2);
        Some(repr2)
      }
      (_, Ty::Infer(var)) => {
        if self.occurs_check(var, repr1) {
          report_error(Error::new(ErrorKind::InfiniteType, span));
          return None;
        }

        self.substitutions.insert(var, repr1);
        Some(repr1)
      }

      // Fun types - unify params and return
      (Ty::Fun(f1), Ty::Fun(f2)) => {
        // Get function types (FunctionTy is Copy now)
        let fun1 = *self.ty_table.fun(&f1)?;
        let fun2 = *self.ty_table.fun(&f2)?;

        if fun1.param_count != fun2.param_count {
          report_error(Error::new(ErrorKind::ArgumentCountMismatch, span));
          return None;
        }

        // Get parameter slices
        let params1 = self.ty_table.fun_params(&fun1).to_vec();
        let params2 = self.ty_table.fun_params(&fun2).to_vec();

        // Unify parameters pairwise
        for (p1, p2) in params1.iter().zip(params2.iter()) {
          self.unify(*p1, *p2, span)?;
        }

        // Unify return types
        self.unify(fun1.return_ty, fun2.return_ty, span)?;

        // Return the canonical representative after all unifications
        Some(self.resolve_id(repr1))
      }

      // Array types - unify element types
      (Ty::Array(a1), Ty::Array(a2)) => {
        let arr1 = *self.ty_table.array(a1)?;
        let arr2 = *self.ty_table.array(a2)?;

        // Unify element types
        self.unify(arr1.elem_ty, arr2.elem_ty, span)?;

        // Check sizes match if both are fixed
        if arr1.size != arr2.size {
          report_error(Error::new(ErrorKind::ArraySizeMismatch, span));
          return None;
        }

        // Return the canonical representative after unification
        Some(self.resolve_id(repr1))
      }

      // Reference types - unify inner types
      (Ty::Ref(r1), Ty::Ref(r2)) => {
        let ref1 = *self.ty_table.reference(r1)?;
        let ref2 = *self.ty_table.reference(r2)?;

        // Check mutability matches
        if ref1.is_mut != ref2.is_mut {
          report_error(Error::new(ErrorKind::TypeMismatch, span));
          return None;
        }

        // Unify inner types
        self.unify(ref1.inner_ty, ref2.inner_ty, span)?;

        // Return the canonical representative after unification
        Some(self.resolve_id(repr1))
      }

      // Concrete types must match exactly
      // No substitutions were added, so repr1 is still canonical
      (
        Ty::Int {
          signed: s1,
          width: w1,
        },
        Ty::Int {
          signed: s2,
          width: w2,
        },
      ) if s1 == s2 && w1 == w2 => Some(repr1),

      (Ty::Float(w1), Ty::Float(w2)) if w1 == w2 => Some(repr1),
      (Ty::Bool, Ty::Bool) => Some(repr1),
      (Ty::Char, Ty::Char) => Some(repr1),
      (Ty::Str, Ty::Str) => Some(repr1),
      (Ty::Unit, Ty::Unit) => Some(repr1),

      _ => {
        report_error(Error::new(ErrorKind::TypeMismatch, span));
        None
      }
    }
  }

  /// Resolve a type variable through substitutions to its canonical
  /// representative. Implements path compression for efficiency.
  ///
  /// INVARIANT: Only inference variables can have substitutions.
  /// We extract the InferVarId from the Ty to look up substitutions.
  pub fn resolve_id(&mut self, ty: TyId) -> TyId {
    // First check if this TyId represents an inference variable
    if let Some(kind) = self.tys.get(ty.0 as usize) {
      let var_id = match *kind {
        Ty::Infer(id) => id,
        _ => return ty, // Not an inference variable, already canonical
      };

      if let Some(&subst) = self.substitutions.get(&var_id) {
        // Path compression: update substitution to point directly to the
        // representative
        let repr = self.resolve_id(subst);

        if repr != subst {
          self.substitutions.insert(var_id, repr);
        }

        return repr;
      }
    }

    ty
  }

  /// Get the kind of a type (after resolution)
  pub(crate) fn kind_of(&mut self, ty: TyId) -> Ty {
    let repr = self.resolve_id(ty);

    self.tys[repr.0 as usize]
  }

  /// Occurs check - prevents infinite types like α = List<α>
  fn occurs_check(&mut self, var: InferVarId, ty: TyId) -> bool {
    match self.kind_of(ty) {
      Ty::Infer(v) if v == var => true,
      Ty::Fun(f) => {
        let fun = *self.ty_table.fun(&f).unwrap();
        let params = self.ty_table.fun_params(&fun).to_vec();

        for param in params {
          if self.occurs_check(var, param) {
            return true;
          }
        }

        self.occurs_check(var, fun.return_ty)
      }
      Ty::Array(a) => {
        let arr = *self.ty_table.array(a).unwrap();

        self.occurs_check(var, arr.elem_ty)
      }
      Ty::Ref(r) => {
        let ref_ty = *self.ty_table.reference(r).unwrap();

        self.occurs_check(var, ref_ty.inner_ty)
      }
      _ => false,
    }
  }

  // === PHASE 1.3: TYPE ENVIRONMENT ===

  /// Push a new scope onto the environment stack
  pub fn push_scope(&mut self) {
    self.ty_env_stack.push(self.ty_env.clone());
    self.type_aliases_stack.push(self.ty_aliases.clone());

    self.current_level += 1;
  }

  /// Pop a scope from the environment stack
  pub fn pop_scope(&mut self) {
    if let Some(prev_env) = self.ty_env_stack.pop() {
      self.ty_env = prev_env;
    }

    if let Some(prev_aliases) = self.type_aliases_stack.pop() {
      self.ty_aliases = prev_aliases;
    }

    if self.current_level > 0 {
      self.current_level -= 1;
    }
  }

  /// Bind a variable to a type in the current scope
  pub fn bind_var(&mut self, name: Symbol, ty: TyId) {
    self.ty_env.insert(name, ty);
  }

  /// Look up a variable's type in the environment
  pub fn lookup_var(&self, name: Symbol) -> Option<TyId> {
    self.ty_env.get(&name).copied()
  }

  /// Generalize a type (for let-polymorphism)
  /// Finds all free type variables and quantifies them
  /// Uses level-based generalization for O(1) performance
  pub fn generalize(&mut self, ty: TyId) -> TyScheme {
    let mut free_vars = Vec::new();

    self.collect_free_vars(ty, &mut free_vars);

    // Only generalize variables created at levels higher than current level
    // This is equivalent to checking if they're not in the environment,
    // but O(1) instead of O(n)
    free_vars.retain(|v| {
      self
        .var_levels
        .get(v)
        .is_some_and(|level| *level > self.current_level)
    });

    TyScheme {
      quantified: free_vars,
      ty,
    }
  }

  /// Instantiate a type scheme with fresh type variables
  pub fn instantiate(&mut self, scheme: &TyScheme) -> TyId {
    if scheme.quantified.is_empty() {
      return scheme.ty;
    }

    // Create fresh variables for each quantified variable
    let mut subst = HashMap::default();

    for var in &scheme.quantified {
      subst.insert(*var, self.fresh_var());
    }

    // Apply substitution to the type
    self.substitute_ty(&scheme.ty, &subst)
  }

  /// Collect free type variables in a type
  fn collect_free_vars(&mut self, ty: TyId, vars: &mut Vec<InferVarId>) {
    match self.kind_of(ty) {
      Ty::Infer(v) => {
        if !vars.contains(&v) {
          vars.push(v);
        }
      }
      Ty::Fun(f) => {
        let fun = *self.ty_table.fun(&f).unwrap();
        let params = self.ty_table.fun_params(&fun).to_vec();

        for param in params {
          self.collect_free_vars(param, vars);
        }

        self.collect_free_vars(fun.return_ty, vars);
      }
      Ty::Array(id) => {
        let arr = *self.ty_table.array(id).unwrap();

        self.collect_free_vars(arr.elem_ty, vars);
      }
      Ty::Ref(r) => {
        let ref_ty = *self.ty_table.reference(r).unwrap();

        self.collect_free_vars(ref_ty.inner_ty, vars);
      }
      _ => {}
    }
  }

  /// Substitute type variables in a type
  fn substitute_ty(
    &mut self,
    ty: &TyId,
    subst: &HashMap<InferVarId, TyId>,
  ) -> TyId {
    match self.kind_of(*ty) {
      Ty::Infer(v) => subst.get(&v).copied().unwrap_or(*ty),
      Ty::Fun(f) => {
        let fun = *self.ty_table.fun(&f).unwrap();
        let params = self.ty_table.fun_params(&fun).to_vec();

        let new_params = params
          .iter()
          .map(|p| self.substitute_ty(p, subst))
          .collect::<Vec<_>>();

        let new_return = self.substitute_ty(&fun.return_ty, subst);
        let new_fun_id = self.ty_table.intern_fun(new_params, new_return);

        self.intern_ty(Ty::Fun(new_fun_id))
      }
      Ty::Array(a) => {
        let arr = *self.ty_table.array(a).unwrap();
        let new_elem = self.substitute_ty(&arr.elem_ty, subst);
        let new_arr_id = self.ty_table.intern_array(new_elem, arr.size);

        self.intern_ty(Ty::Array(new_arr_id))
      }
      Ty::Ref(r) => {
        let ref_ty = *self.ty_table.reference(r).unwrap();
        let new_inner = self.substitute_ty(&ref_ty.inner_ty, subst);
        let new_ref_id = self.ty_table.intern_ref(ref_ty.is_mut, new_inner);

        self.intern_ty(Ty::Ref(new_ref_id))
      }
      _ => *ty,
    }
  }

  /// Bind a polymorphic type scheme to a variable
  pub fn bind_poly(&mut self, name: Symbol, scheme: TyScheme) {
    self.poly_schemes.insert(name, scheme);
  }

  /// Look up a polymorphic variable and instantiate it
  pub fn lookup_poly(&mut self, name: Symbol) -> Option<TyId> {
    let scheme = self.poly_schemes.get(&name)?.clone();

    Some(self.instantiate(&scheme))
  }

  // === PHASE 1.4: TYPE INFERENCE (W ALGORITHM) ===
  // The W algorithm is THE Hindley-Milner type inference algorithm
  // W(Γ, e) → (S, τ) where:
  // - Γ is the type environment
  // - e is the expression
  // - S is the substitution
  // - τ is the inferred type

  /// Infer type for an integer literal (W algorithm [LIT] rule)
  /// In pure W, literals get fresh type variables
  pub fn infer_int_literal(&mut self, _value: i64) -> TyId {
    self.fresh_var()
  }

  /// Infer type for a float literal (W algorithm [LIT] rule)
  /// In pure W, literals get fresh type variables
  pub fn infer_float_literal(&mut self, _value: f64) -> TyId {
    self.fresh_var()
  }

  /// Infer type for a boolean literal (W algorithm [LIT] rule)
  pub fn infer_bool_literal(&mut self, _value: bool) -> TyId {
    self.bool_type()
  }

  /// Infer type for a string literal (W algorithm [LIT] rule)
  pub fn infer_str_literal(&mut self, _value: &str) -> TyId {
    self.str_type()
  }

  /// Infer type for a char literal (W algorithm [LIT] rule)
  pub fn infer_char_literal(&mut self, _value: char) -> TyId {
    self.char_type()
  }

  /// Infer type for a binary operation
  pub fn infer_binop(
    &mut self,
    op: BinOp,
    lhs_ty: TyId,
    rhs_ty: TyId,
    span: Span,
  ) -> Option<TyId> {
    match op {
      // Arithmetic operations
      BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => {
        // Unify both operands - they must be the same numeric type
        let ty = self.unify(lhs_ty, rhs_ty, span)?;

        // Ensure it's a numeric type (int or float)
        let repr = self.resolve_id(ty);

        match self.tys[repr.0 as usize] {
          Ty::Infer(_) => Some(ty),
          Ty::Int { .. } | Ty::Float(_) => Some(ty),
          _ => {
            report_error(Error::new(ErrorKind::TypeMismatch, span));
            None
          }
        }
      }

      // Comparison operations
      BinOp::Eq
      | BinOp::Neq
      | BinOp::Lt
      | BinOp::Lte
      | BinOp::Gt
      | BinOp::Gte => {
        // Operands must be the same type
        self.unify(lhs_ty, rhs_ty, span)?;
        Some(self.bool_type())
      }

      // Logical operations
      BinOp::And | BinOp::Or => {
        let lhs_ty_bool = self.bool_type();
        let rhs_ty_bool = self.bool_type();

        // Both operands must be bool
        self.unify(lhs_ty, lhs_ty_bool, span)?;
        self.unify(rhs_ty, rhs_ty_bool, span)?;
        Some(self.bool_type())
      }

      // Bitwise operations
      BinOp::BitAnd
      | BinOp::BitOr
      | BinOp::BitXor
      | BinOp::Shl
      | BinOp::Shr => {
        // Must be integer types
        let ty = self.unify(lhs_ty, rhs_ty, span)?;

        // Ensure it's an integer type
        let repr = self.resolve_id(ty);

        match self.tys[repr.0 as usize] {
          Ty::Infer(_) => Some(ty),
          Ty::Int { .. } => Some(ty),
          _ => {
            report_error(Error::new(ErrorKind::TypeMismatch, span));
            None
          }
        }
      }
    }
  }

  /// Infer type for a unary operation
  pub fn infer_unop(
    &mut self,
    op: UnOp,
    rhs_ty: TyId,
    span: Span,
  ) -> Option<TyId> {
    match op {
      UnOp::Neg => {
        // Negation works on numeric types
        let repr = self.resolve_id(rhs_ty);

        match self.tys[repr.0 as usize] {
          Ty::Infer(_) | Ty::Int { .. } | Ty::Float(_) => Some(rhs_ty),
          _ => {
            report_error(Error::new(ErrorKind::TypeMismatch, span));
            None
          }
        }
      }

      UnOp::Not => {
        let rhs_bool_ty = self.bool_type();

        // Logical not requires bool
        self.unify(rhs_ty, rhs_bool_ty, span)
      }

      UnOp::BitNot => {
        // Bitwise not requires integer
        let repr = self.resolve_id(rhs_ty);

        match self.tys[repr.0 as usize] {
          Ty::Infer(_) | Ty::Int { .. } => Some(rhs_ty),
          _ => {
            report_error(Error::new(ErrorKind::TypeMismatch, span));
            None
          }
        }
      }
      _ => None,
    }
  }

  /// Infer type for a variable reference (W algorithm [VAR] rule)
  /// [VAR]: Γ(x) = σ ⊢ x : τ where τ = inst(σ)
  pub fn infer_var(&mut self, name: Symbol, span: Span) -> Option<TyId> {
    // First check polymorphic bindings (let-bound functions)
    // This implements the instantiation part of the [VAR] rule
    if let Some(ty) = self.lookup_poly(name) {
      return Some(ty);
    }

    // Then check regular bindings (monomorphic variables)
    if let Some(ty) = self.lookup_var(name) {
      return Some(ty);
    }

    report_error(Error::new(ErrorKind::UndefinedVariable, span));
    None
  }

  /// Handle type annotation (e.g., x: s32)
  pub fn handle_ty_annotation(
    &mut self,
    annotated_ty: TyId,
    inferred_ty: TyId,
    span: Span,
  ) -> Option<TyId> {
    self.unify(annotated_ty, inferred_ty, span)
  }

  // === PHASE 1.5: TYPE ALIASES ===

  /// Define a type alias in the current scope
  /// e.g., "type FooBar = u32" binds FooBar -> u32
  pub fn define_ty_alias(&mut self, name: Symbol, ty: TyId) {
    self.ty_aliases.insert(name, ty);
  }

  /// Look up a type alias
  pub fn lookup_ty_alias(&self, name: Symbol) -> Option<TyId> {
    self.ty_aliases.get(&name).copied()
  }

  /// Resolve a type name, which could be:
  /// 1. A type alias (e.g., FooBar -> u32)
  /// 2. A struct type
  /// 3. An enum type
  ///
  /// ---
  ///
  /// Returns None if the name is not found.
  pub fn resolve_ty_name(&mut self, name: Symbol) -> Option<TyId> {
    if let Some(ty) = self.lookup_ty_alias(name) {
      return Some(ty);
    }

    // Check if it's a struct type
    // For now, we create a Struct type on demand
    // In a real compiler, we'd check if the struct is defined
    Some(self.intern_ty(Ty::Struct(name)))
  }

  /// Resolve a type from its Symbol (already interned during tokenization)
  /// This is called during HIR execution when we encounter a type reference
  ///
  /// For builtin types, we check against well-known symbols
  /// For user-defined types, we check type aliases
  pub fn resolve_ty_symbol(
    &mut self,
    sym: Symbol,
    interner: &Interner,
  ) -> Option<TyId> {
    match interner.get(sym) {
      "s8" => Some(self.intern_ty(Ty::Int {
        signed: true,
        width: IntWidth::S8,
      })),
      "s16" => Some(self.intern_ty(Ty::Int {
        signed: true,
        width: IntWidth::S16,
      })),
      "s32" | "int" => Some(self.s32_type()),
      "s64" => Some(self.intern_ty(Ty::Int {
        signed: true,
        width: IntWidth::S64,
      })),
      "u8" => Some(self.intern_ty(Ty::Int {
        signed: false,
        width: IntWidth::U8,
      })),
      "u16" => Some(self.intern_ty(Ty::Int {
        signed: false,
        width: IntWidth::U16,
      })),
      "u32" | "uint" => Some(self.u32_type()),
      "u64" => Some(self.intern_ty(Ty::Int {
        signed: false,
        width: IntWidth::U64,
      })),
      "f32" | "float" => Some(self.f32_type()),
      "f64" => Some(self.f64_type()),
      "bool" => Some(self.bool_type()),
      "char" => Some(self.char_type()),
      "str" => Some(self.str_type()),
      "unit" | "()" => Some(self.unit_type()),
      _ => self.resolve_ty_name(sym),
    }
  }
}
impl Default for TyChecker {
  fn default() -> Self {
    Self::new()
  }
}
