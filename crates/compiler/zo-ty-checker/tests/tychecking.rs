//! ```sh
//! cargo test -p zo-ty-checker --test tychecking
//! ```

use zo_interner::Interner;
use zo_span::Span;
use zo_ty::{FloatWidth, IntWidth, Ty};
use zo_ty_checker::TyChecker;

use proptest::prelude::*;
// use proptest::test_runner::{Config, FileFailurePersistence};

/// Generate arbitrary integer widths
fn arb_int_width() -> impl Strategy<Value = IntWidth> {
  prop_oneof![
    Just(IntWidth::S8),
    Just(IntWidth::S16),
    Just(IntWidth::S32),
    Just(IntWidth::S64),
    Just(IntWidth::U8),
    Just(IntWidth::U16),
    Just(IntWidth::U32),
    Just(IntWidth::U64),
    Just(IntWidth::Arch),
  ]
}

/// Generate arbitrary float widths
fn arb_float_width() -> impl Strategy<Value = FloatWidth> {
  prop_oneof![
    Just(FloatWidth::F32),
    Just(FloatWidth::F64),
    Just(FloatWidth::Arch),
  ]
}

/// Generate arbitrary concrete (non-infer) types
fn arb_concrete_type() -> impl Strategy<Value = Ty> {
  prop_oneof![
    Just(Ty::Unit),
    Just(Ty::Bool),
    Just(Ty::Bytes),
    Just(Ty::Char),
    Just(Ty::Str),
    (any::<bool>(), arb_int_width())
      .prop_map(|(signed, width)| Ty::Int { signed, width }),
    arb_float_width().prop_map(Ty::Float),
  ]
}

proptest! {
  // #![proptest_config(Config {
  //   failure_persistence: Some(Box::new(
  //     FileFailurePersistence::WithSource("tests/tychecking")
  //   )),
  //   ..Config::default()
  // })]

  #[test]
  fn prop_unify_reflexive(kind in arb_concrete_type()) {
    let mut checker = TyChecker::new();
    let ty = checker.intern_ty(kind);
    let span = Span::new(0, 0);

    // Unifying a type with itself should always succeed
    let result = checker.unify(ty, ty, span);

    prop_assert!(result.is_some());
    prop_assert_eq!(result.unwrap(), ty);
  }

  #[test]
  fn prop_unify_symmetric(kind1 in arb_concrete_type(), kind2 in arb_concrete_type()) {
    let mut checker1 = TyChecker::new();
    let mut checker2 = TyChecker::new();
    let span = Span::new(0, 0);

    let ty1_a = checker1.intern_ty(kind1);
    let ty2_a = checker1.intern_ty(kind2);

    let ty1_b = checker2.intern_ty(kind1);
    let ty2_b = checker2.intern_ty(kind2);

    // unify(a, b) should have same success/failure as unify(b, a)
    let result1 = checker1.unify(ty1_a, ty2_a, span);
    let result2 = checker2.unify(ty2_b, ty1_b, span);

    prop_assert_eq!(result1.is_some(), result2.is_some());
  }

  #[test]
  fn prop_fresh_vars_unique(n in 1usize..100) {
    let mut checker = TyChecker::new();
    let mut vars = Vec::new();

    for _ in 0..n {
      vars.push(checker.fresh_var());
    }

    // All fresh variables should be unique
    let mut seen = std::collections::HashSet::new();

    for var in vars {
      prop_assert!(seen.insert(var.0));
    }
  }

  #[test]
  fn prop_infer_unifies_with_any(kind in arb_concrete_type()) {
    let mut checker = TyChecker::new();
    let span = Span::new(0, 0);

    let infer_ty = checker.fresh_var();
    let concrete_ty = checker.intern_ty(kind);

    // Inference variable should unify with any concrete type
    let result = checker.unify(infer_ty, concrete_ty, span);

    prop_assert!(result.is_some());

    // After unification, resolving the infer var should give the concrete type
    let resolved = checker.resolve_id(infer_ty);

    prop_assert_eq!(resolved, concrete_ty);
  }

  #[test]
  fn prop_substitution_idempotent(kind in arb_concrete_type()) {
    let mut checker = TyChecker::new();
    let span = Span::new(0, 0);

    let var = checker.fresh_var();
    let ty = checker.intern_ty(kind);

    // Unify variable with type
    checker.unify(var, ty, span).unwrap();

    // Resolving multiple times should give same result
    let resolved1 = checker.resolve_id(var);
    let resolved2 = checker.resolve_id(resolved1);
    let resolved3 = checker.resolve_id(resolved2);

    prop_assert_eq!(resolved1, resolved2);
    prop_assert_eq!(resolved2, resolved3);
  }

  #[test]
  fn prop_interning_consistent(kind in arb_concrete_type()) {
    let mut checker = TyChecker::new();

    // Interning the same type multiple times should give the same ID
    let ty1 = checker.intern_ty(kind);
    let ty2 = checker.intern_ty(kind);
    let ty3 = checker.intern_ty(kind);

    prop_assert_eq!(ty1, ty2);
    prop_assert_eq!(ty2, ty3);
  }

  #[test]
  fn prop_function_unification(
    param_count in 0usize..5,
    use_infer in prop::bool::ANY,
  ) {
    let mut checker = TyChecker::new();
    let span = Span::new(0, 0);

    // Create function with inference variables or concrete types
    let mut params1 = Vec::new();
    let mut params2 = Vec::new();

    for _ in 0..param_count {
      if use_infer {
        params1.push(checker.fresh_var());
        params2.push(checker.fresh_var());
      } else {
        let ty = checker.bool_type();
        params1.push(ty);
        params2.push(ty);
      }
    }

    let ret1 = if use_infer { checker.fresh_var() } else { checker.bool_type() };
    let ret2 = if use_infer { checker.fresh_var() } else { checker.bool_type() };

    let fun1_id = checker.ty_table.intern_fun(params1, ret1);
    let fun2_id = checker.ty_table.intern_fun(params2, ret2);

    let fun1 = checker.intern_ty(Ty::Fun(fun1_id));
    let fun2 = checker.intern_ty(Ty::Fun(fun2_id));

    // Functions with same arity should unify (with infer vars)
    // or succeed/fail consistently (with concrete types)
    let result = checker.unify(fun1, fun2, span);

    if use_infer {
      prop_assert!(result.is_some(), "Functions with inference variables should unify");
    } else {
      prop_assert!(result.is_some(), "Identical functions should unify");
    }
  }

  #[test]
  fn prop_array_unification(size in prop::option::of(0u32..100)) {
    let mut checker = TyChecker::new();
    let span = Span::new(0, 0);

    // Create arrays with same element type but potentially different sizes
    let elem_ty = checker.fresh_var();

    let arr1_id = checker.ty_table.intern_array(elem_ty, size);
    let arr2_id = checker.ty_table.intern_array(elem_ty, size);

    let arr1 = checker.intern_ty(Ty::Array(arr1_id));
    let arr2 = checker.intern_ty(Ty::Array(arr2_id));

    // Arrays with same element type and size should unify
    let result = checker.unify(arr1, arr2, span);
    prop_assert!(result.is_some());
  }

  #[test]
  fn prop_generalization_valid(num_vars in 1usize..10) {
    let mut checker = TyChecker::new();

    // Create a type with multiple inference variables in a nested scope
    checker.push_scope();

    let mut vars = Vec::new();

    for _ in 0..num_vars {
      vars.push(checker.fresh_var());
    }

    // Create a function type using these variables
    let ret_ty = vars[0];
    let fun_id = checker.ty_table.intern_fun(vars, ret_ty);
    let fun_ty = checker.intern_ty(Ty::Fun(fun_id));

    checker.pop_scope();

    // Generalize should capture all the variables
    let scheme = checker.generalize(fun_ty);

    // All variables should be quantified (they were created at level 1)
    prop_assert_eq!(scheme.quantified.len(), num_vars);

    // Instantiating should create fresh variables
    let inst1 = checker.instantiate(&scheme);
    let inst2 = checker.instantiate(&scheme);

    // Different instantiations should use different fresh variables
    prop_assert_ne!(inst1, inst2);
  }

  #[test]
  fn prop_scope_levels(depth in 1usize..10) {
    let mut checker = TyChecker::new();

    // Push multiple scopes
    for _ in 0..depth {
      checker.push_scope();
    }

    // Create a variable at this depth
    let var = checker.fresh_var();

    // Pop all scopes
    for _ in 0..depth {
      checker.pop_scope();
    }

    // Variable should be generalizable (created at higher level)
    let scheme = checker.generalize(var);

    prop_assert!(!scheme.quantified.is_empty());
  }

  #[test]
  fn prop_occurs_check(use_function in prop::bool::ANY, use_array in prop::bool::ANY) {
    let mut checker = TyChecker::new();
    let span = Span::new(0, 0);

    let var = checker.fresh_var();

    // Try to create infinite type: var = T<var>
    let infinite_ty = if use_function {
      let fun_id = checker.ty_table.intern_fun(vec![var], var);

      checker.intern_ty(Ty::Fun(fun_id))
    } else if use_array {
      let arr_id = checker.ty_table.intern_array(var, None);

      checker.intern_ty(Ty::Array(arr_id))
    } else {
      // Reference to itself
      let ref_id = checker.ty_table.intern_ref(false, var);

      checker.intern_ty(Ty::Ref(ref_id))
    };

    // Should fail due to occurs check
    let result = checker.unify(var, infinite_ty, span);

    prop_assert!(result.is_none(), "Occurs check should prevent infinite types");
  }

  #[test]
  fn prop_mono_vs_poly_binding(use_poly in prop::bool::ANY) {
    let mut checker = TyChecker::new();
    let mut interner = Interner::new();

    let name = interner.intern("test_var");

    if use_poly {
      // Polymorphic binding
      checker.push_scope();
      let ty = checker.fresh_var();
      checker.pop_scope();

      let scheme = checker.generalize(ty);
      checker.bind_poly(name, scheme);

      // Multiple lookups should give different types
      let ty1 = checker.lookup_poly(name).unwrap();
      let ty2 = checker.lookup_poly(name).unwrap();

      prop_assert_ne!(ty1, ty2, "Polymorphic lookups should instantiate fresh types");
    } else {
      // Monomorphic binding
      let ty = checker.bool_type();
      checker.bind_var(name, ty);

      // Multiple lookups should give same type
      let ty1 = checker.lookup_var(name).unwrap();
      let ty2 = checker.lookup_var(name).unwrap();

      prop_assert_eq!(ty1, ty2, "Monomorphic lookups should return same type");
    }
  }

  #[test]
  fn prop_path_compression(chain_length in 2usize..20) {
    let mut checker = TyChecker::new();
    let span = Span::new(0, 0);

    // Create a chain of inference variables
    let mut vars = Vec::new();

    for _ in 0..chain_length {
      vars.push(checker.fresh_var());
    }

    // Unify them in a chain: v0 -> v1 -> v2 -> ... -> concrete
    for i in 0..vars.len()-1 {
      checker.unify(vars[i], vars[i+1], span).unwrap();
    }

    // Unify last with concrete type
    let concrete = checker.s32_type();
    checker.unify(vars[vars.len()-1], concrete, span).unwrap();

    // First resolution might traverse the chain
    let resolved = checker.resolve_id(vars[0]);

    prop_assert_eq!(resolved, concrete);

    // Second resolution should be direct (path compression)
    let resolved2 = checker.resolve_id(vars[0]);

    prop_assert_eq!(resolved2, concrete);
  }

  #[test]
  fn prop_principal_type(use_annotations in prop::bool::ANY) {
    let mut checker = TyChecker::new();
    let span = Span::new(0, 0);

    // λx. x has principal type ∀α. α → α
    // This is the most general type for the identity function

    // Create type variable at higher level to enable generalization
    checker.push_scope();  // Level becomes 1

    let x_ty = checker.fresh_var();  // Created at level 1
    let fun_id = checker.ty_table.intern_fun(vec![x_ty], x_ty);
    let identity_ty = checker.intern_ty(Ty::Fun(fun_id));

    checker.pop_scope();  // Level back to 0

    // If we add constraints, the type should specialize
    if use_annotations {
      // Constrain x to be Int
      let int_ty = checker.s32_type();

      checker.unify(x_ty, int_ty, span).unwrap();

      // Now the function type should be Int → Int
      let fun = checker.ty_table.fun(&fun_id).unwrap();
      let resolved_param = checker.resolve_id(fun.return_ty);

      prop_assert_eq!(resolved_param, int_ty);
    } else {
      // Without constraints, x remains polymorphic
      let scheme = checker.generalize(identity_ty);
      // Should have exactly one quantified variable (the parameter/return type)
      // x_ty was created at level 1, now we're at level 0, so it gets generalized
      prop_assert_eq!(scheme.quantified.len(), 1);
    }
  }

  #[test]
  fn prop_unification_transitivity(
    use_int in prop::bool::ANY,
    chain_len in 2usize..5
  ) {
    let mut checker = TyChecker::new();
    let span = Span::new(0, 0);

    let mut types = Vec::new();

    // Create a chain of types
    for i in 0..chain_len {
      if i == chain_len - 1 && use_int {
        // Last one is concrete
        types.push(checker.s32_type());
      } else {
        // Others are inference variables
        types.push(checker.fresh_var());
      }
    }

    // Unify: T0 = T1, T1 = T2, ..., Tn-1 = Tn
    for i in 0..types.len()-1 {
      let result = checker.unify(types[i], types[i+1], span);
      prop_assert!(result.is_some());
    }

    // By transitivity: T0 = Tn
    let first_resolved = checker.resolve_id(types[0]);
    let last_resolved = checker.resolve_id(types[types.len()-1]);

    prop_assert_eq!(first_resolved, last_resolved);

    // All intermediate types should also resolve to the same type
    for ty in &types {
      let resolved = checker.resolve_id(*ty);
      prop_assert_eq!(resolved, first_resolved);
    }
  }

  #[test]
  fn prop_inference_completeness(num_constraints in 1usize..10) {
    let mut checker = TyChecker::new();
    let span = Span::new(0, 0);

    // Create a system of type constraints
    let mut vars = Vec::new();

    for _ in 0..num_constraints {
      vars.push(checker.fresh_var());
    }

    // Add various constraints
    for i in 0..vars.len() {
      if i % 3 == 0 && i + 1 < vars.len() {
        // Some variables unify with their neighbors
        checker.unify(vars[i], vars[i + 1], span).unwrap();
      } else if i % 3 == 1 {
        // Some unify with concrete types
        let bool_ty = checker.bool_type();

        checker.unify(vars[i], bool_ty, span).unwrap();
      }
      // Others remain free
    }

    // Verify that inference finds a valid solution
    // All constrained variables should resolve to their expected types
    for i in 0..vars.len() {
      let resolved = checker.resolve_id(vars[i]);

      if i % 3 == 0 && i + 1 < vars.len() {
        // Should be same as neighbor
        let neighbor_resolved = checker.resolve_id(vars[i + 1]);

        prop_assert_eq!(resolved, neighbor_resolved);
      } else if i % 3 == 1 {
        // Should be bool
        let bool_ty = checker.bool_type();

        prop_assert_eq!(resolved, bool_ty);
      }
      // Free variables can be anything
    }
  }

  #[test]
  fn prop_determinism(kind in arb_concrete_type(), iterations in 2usize..5) {
    // Same operations should produce same results
    let mut results = Vec::new();

    for _ in 0..iterations {
      let mut checker = TyChecker::new();
      let span = Span::new(0, 0);

      // Perform same sequence of operations
      let ty1 = checker.intern_ty(kind);
      let var1 = checker.fresh_var();
      let var2 = checker.fresh_var();

      checker.unify(var1, ty1, span).unwrap();
      checker.unify(var2, var1, span).unwrap();

      let resolved = checker.resolve_id(var2);

      // Store the result (as the internal u32)
      results.push(resolved.0);
    }

    // All iterations should produce the same result
    for i in 1..results.len() {
      prop_assert_eq!(results[0], results[i],
        "Type checking should be deterministic");
    }
  }

  #[test]
  fn prop_monotonicity(concrete_ty in arb_concrete_type()) {
    let mut checker = TyChecker::new();
    let span = Span::new(0, 0);

    // Create an inference variable (represents unannotated value)
    let inferred = checker.fresh_var();

    // Without annotation, it can unify with any type
    let test_ty = checker.intern_ty(concrete_ty);
    let unify_result = checker.unify(inferred, test_ty, span);

    prop_assert!(unify_result.is_some(), "Inference var should unify with any type");

    // Reset for second test
    let mut checker2 = TyChecker::new();
    let inferred2 = checker2.fresh_var();

    // With annotation, it's restricted to that specific type
    let annotated_ty = checker2.s32_type();
    let annotation_result = checker2.handle_ty_annotation(annotated_ty, inferred2, span);

    if annotation_result.is_some() {
      // After annotation, can only unify with compatible types
      let resolved = checker2.resolve_id(inferred2);

      prop_assert_eq!(resolved, annotated_ty, "Annotation should restrict type");

      // Try to unify with a different type - should fail if incompatible
      let bool_ty = checker2.bool_type();
      let invalid_unify = checker2.unify(inferred2, bool_ty, span);

      prop_assert!(invalid_unify.is_none(), "Annotated type should not unify with incompatible type");
    }
  }
}
