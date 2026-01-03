//! ```sh
//! cargo test -p zo-ty-checker --lib
//! ```
//! ```sh
//! cargo test -p zo-ty-checker name_of_the_test
//! ```

mod common;
mod errors;

use crate::TyChecker;

use zo_interner::Interner;
use zo_sir::{BinOp, UnOp};
use zo_span::Span;
use zo_ty::{IntWidth, Ty};

fn dummy_span() -> Span {
  Span::new(0, 0)
}

// ===== LITERAL TYPING TESTS =====

#[test]
fn test_int_literal_inference() {
  let mut checker = TyChecker::new();

  // 1 -> fresh type var (pure W algorithm)
  let ty = checker.infer_int_literal(1);

  // Should be a type variable (no distinction in pure W)
  let kind = checker.kind_of(ty);

  assert!(matches!(kind, Ty::Infer(_)));
}

#[test]
fn test_float_literal_inference() {
  let mut checker = TyChecker::new();

  // 1.0 -> fresh type var (pure W algorithm)
  let ty = checker.infer_float_literal(1.0);

  // Should be a type variable (no distinction in pure W)
  let kind = checker.kind_of(ty);

  assert!(matches!(kind, Ty::Infer(_)));
}

#[test]
fn test_bool_literal() {
  let mut checker = TyChecker::new();

  let ty_true = checker.infer_bool_literal(true);
  let ty_false = checker.infer_bool_literal(false);

  let kind_true = checker.kind_of(ty_true);
  let kind_false = checker.kind_of(ty_false);

  assert_eq!(kind_true, Ty::Bool);
  assert_eq!(kind_false, Ty::Bool);
}

#[test]
fn test_string_literal() {
  let mut checker = TyChecker::new();

  let ty = checker.infer_str_literal("hello");
  let kind = checker.kind_of(ty);

  assert_eq!(kind, Ty::Str);
}

#[test]
fn test_char_literal() {
  let mut checker = TyChecker::new();

  let ty = checker.infer_char_literal('a');
  let kind = checker.kind_of(ty);

  assert_eq!(kind, Ty::Char);
}

// ===== ARITHMETIC OPERATION TESTS =====

#[test]
fn test_int_addition() {
  let mut checker = TyChecker::new();

  // 1 + 2 -> both unify to same type var
  let lhs = checker.infer_int_literal(1);
  let rhs = checker.infer_int_literal(2);

  let result = checker.infer_binop(BinOp::Add, lhs, rhs, dummy_span());

  assert!(result.is_some());

  // In pure W, result is still a type variable
  let kind = checker.kind_of(result.unwrap());

  assert!(matches!(kind, Ty::Infer(_)));
}

#[test]
fn test_mixed_literals_unify() {
  let mut checker = TyChecker::new();

  // In pure W, int and float literals both get fresh type vars
  // They CAN unify - there's no distinction
  let lhs = checker.infer_int_literal(1);
  let rhs = checker.infer_float_literal(1.0);

  let result = checker.infer_binop(BinOp::Add, lhs, rhs, dummy_span());

  // In pure W, this succeeds - both unify to same type var
  assert!(result.is_some());

  // Result is still a type variable
  if let Some(ty) = result {
    let kind = checker.kind_of(ty);

    assert!(matches!(kind, Ty::Infer(_)));
  }
}

#[test]
fn test_float_arithmetic() {
  let mut checker = TyChecker::new();

  let lhs = checker.infer_float_literal(1.5);
  let rhs = checker.infer_float_literal(2.5);

  let result = checker.infer_binop(BinOp::Mul, lhs, rhs, dummy_span());

  assert!(result.is_some());

  // In pure W, still a type variable
  let kind = checker.kind_of(result.unwrap());

  assert!(matches!(kind, Ty::Infer(_)));
}

#[test]
fn test_comparison_operations() {
  let mut checker = TyChecker::new();

  let lhs = checker.infer_int_literal(1);
  let rhs = checker.infer_int_literal(2);

  // Comparison returns bool
  let result = checker.infer_binop(BinOp::Lt, lhs, rhs, dummy_span());

  assert!(result.is_some());

  let kind = checker.kind_of(result.unwrap());

  assert_eq!(kind, Ty::Bool);
}

#[test]
fn test_logical_operations() {
  let mut checker = TyChecker::new();

  let lhs = checker.infer_bool_literal(true);
  let rhs = checker.infer_bool_literal(false);

  let result = checker.infer_binop(BinOp::And, lhs, rhs, dummy_span());

  assert!(result.is_some());

  let kind = checker.kind_of(result.unwrap());

  assert_eq!(kind, Ty::Bool);
}

#[test]
fn test_bitwise_operations() {
  let mut checker = TyChecker::new();

  let lhs = checker.infer_int_literal(0b1010);
  let rhs = checker.infer_int_literal(0b0101);

  let result = checker.infer_binop(BinOp::BitAnd, lhs, rhs, dummy_span());

  assert!(result.is_some());

  // In pure W, still a type variable
  let kind = checker.kind_of(result.unwrap());

  assert!(matches!(kind, Ty::Infer(_)));
}

#[test]
fn test_unary_operations() {
  let mut checker = TyChecker::new();

  // Negation on int
  let int_ty = checker.infer_int_literal(42);
  let neg_result = checker.infer_unop(UnOp::Neg, int_ty, dummy_span());

  assert!(neg_result.is_some());

  // Not on bool
  let bool_ty = checker.infer_bool_literal(true);
  let not_result = checker.infer_unop(UnOp::Not, bool_ty, dummy_span());

  assert!(not_result.is_some());
  assert_eq!(checker.kind_of(not_result.unwrap()), Ty::Bool);

  // BitNot on int
  let bitnot_result = checker.infer_unop(UnOp::BitNot, int_ty, dummy_span());

  assert!(bitnot_result.is_some());
}

// ===== POLYMORPHISM TESTS =====

#[test]
fn test_let_polymorphism() {
  let mut checker = TyChecker::new();
  let mut interner = Interner::new();

  // Simulate: let id = \x -> x
  // In a real compiler, the RHS of a let is checked in a nested scope
  checker.push_scope(); // Enter scope for RHS

  // Create a polymorphic identity function type: ∀α. α -> α
  let alpha = checker.fresh_var();
  let params = vec![alpha];
  let return_ty = alpha;
  let fun_ty_id = checker.ty_table.intern_fun(params, return_ty);
  let fun_ty = checker.intern_ty(Ty::Fun(fun_ty_id));

  checker.pop_scope(); // Exit scope before generalization

  // Generalize the function (quantify over α)
  let scheme = checker.generalize(fun_ty);

  assert!(!scheme.quantified.is_empty()); // Should have quantified variables

  // Bind as polymorphic
  let id_name = interner.intern("id");
  checker.bind_poly(id_name, scheme);

  // let a = id(1) - instantiate with int
  let id_inst1 = checker.lookup_poly(id_name).unwrap();

  // let b = id(true) - instantiate with bool
  let id_inst2 = checker.lookup_poly(id_name).unwrap();

  // The two instantiations should have different type variables
  assert_ne!(id_inst1, id_inst2);
}

#[test]
fn test_monomorphic_binding() {
  let mut checker = TyChecker::new();
  let mut interner = Interner::new();

  // Non-generalized binding
  let ty = checker.infer_int_literal(42);
  let name = interner.intern("x");

  checker.bind_var(name, ty);

  // Lookup should return same type
  let lookup = checker.lookup_var(name);

  assert_eq!(lookup, Some(ty));
}

// ===== OCCURS CHECK TESTS =====

#[test]
fn test_occurs_check_direct() {
  let mut checker = TyChecker::new();

  // Try to unify α = List<α> (infinite type)
  let alpha = checker.fresh_var();
  let elem_ty = alpha;
  let array_id = checker.ty_table.intern_array(elem_ty, None);
  let array_ty = checker.intern_ty(Ty::Array(array_id));

  // This should fail due to occurs check
  let result = checker.unify(alpha, array_ty, dummy_span());

  assert!(result.is_none());
}

#[test]
fn test_occurs_check_in_function() {
  let mut checker = TyChecker::new();

  // Try to unify α = (α -> β)
  let alpha = checker.fresh_var();
  let beta = checker.fresh_var();

  let params = vec![alpha];
  let fun_id = checker.ty_table.intern_fun(params, beta);
  let fun_ty = checker.intern_ty(Ty::Fun(fun_id));

  // Should fail occurs check
  let result = checker.unify(alpha, fun_ty, dummy_span());

  assert!(result.is_none());
}

#[test]
fn test_occurs_check_nested() {
  let mut checker = TyChecker::new();

  // α = Ref<Array<α>> - nested infinite type
  let alpha = checker.fresh_var();

  // Create Array<α>
  let array_id = checker.ty_table.intern_array(alpha, None);
  let array_ty = checker.intern_ty(Ty::Array(array_id));

  // Create Ref<Array<α>>
  let ref_id = checker.ty_table.intern_ref(false, array_ty);
  let ref_ty = checker.intern_ty(Ty::Ref(ref_id));

  // Should fail occurs check
  let result = checker.unify(alpha, ref_ty, dummy_span());

  assert!(result.is_none());
}

// ===== FUNCTION UNIFICATION TESTS =====

#[test]
fn test_function_unification() {
  let mut checker = TyChecker::new();

  // (Int -> α) = (β -> Bool)
  let int_ty = checker.s32_type();
  let bool_ty = checker.bool_type();
  let alpha = checker.fresh_var();
  let beta = checker.fresh_var();

  // Create (Int -> α)
  let fun1_id = checker.ty_table.intern_fun(vec![int_ty], alpha);
  let fun1 = checker.intern_ty(Ty::Fun(fun1_id));

  // Create (β -> Bool)
  let fun2_id = checker.ty_table.intern_fun(vec![beta], bool_ty);
  let fun2 = checker.intern_ty(Ty::Fun(fun2_id));

  // Unify them
  let result = checker.unify(fun1, fun2, dummy_span());

  assert!(result.is_some());

  // β should unify with Int
  let beta_resolved = checker.resolve_id(beta);

  assert_eq!(beta_resolved, int_ty);

  // α should unify with Bool
  let alpha_resolved = checker.resolve_id(alpha);

  assert_eq!(alpha_resolved, bool_ty);
}

#[test]
fn test_function_arity_mismatch() {
  let mut checker = TyChecker::new();

  let int_ty = checker.s32_type();
  let bool_ty = checker.bool_type();

  // (Int -> Bool) vs (Int, Int -> Bool)
  let fun1_id = checker.ty_table.intern_fun(vec![int_ty], bool_ty);
  let fun1 = checker.intern_ty(Ty::Fun(fun1_id));

  let fun2_id = checker.ty_table.intern_fun(vec![int_ty, int_ty], bool_ty);
  let fun2 = checker.intern_ty(Ty::Fun(fun2_id));

  // Should fail - different arity
  let result = checker.unify(fun1, fun2, dummy_span());

  assert!(result.is_none());
}

// ===== ARRAY/REF NESTING TESTS =====

#[test]
fn test_array_unification() {
  let mut checker = TyChecker::new();

  // Array<Int> = Array<α>
  let int_ty = checker.s32_type();
  let alpha = checker.fresh_var();

  let arr1_id = checker.ty_table.intern_array(int_ty, None);
  let arr1 = checker.intern_ty(Ty::Array(arr1_id));

  let arr2_id = checker.ty_table.intern_array(alpha, None);
  let arr2 = checker.intern_ty(Ty::Array(arr2_id));

  let result = checker.unify(arr1, arr2, dummy_span());

  assert!(result.is_some());

  // α should unify with Int
  let alpha_resolved = checker.resolve_id(alpha);

  assert_eq!(alpha_resolved, int_ty);
}

#[test]
fn test_nested_array_ref() {
  let mut checker = TyChecker::new();

  // Ref<Array<Int>> = Ref<Array<α>>
  let int_ty = checker.s32_type();
  let alpha = checker.fresh_var();

  let arr1_id = checker.ty_table.intern_array(int_ty, None);
  let arr1_ty = checker.intern_ty(Ty::Array(arr1_id));
  let ref1_id = checker.ty_table.intern_ref(false, arr1_ty);
  let ref1 = checker.intern_ty(Ty::Ref(ref1_id));

  let arr2_id = checker.ty_table.intern_array(alpha, None);
  let arr2_ty = checker.intern_ty(Ty::Array(arr2_id));
  let ref2_id = checker.ty_table.intern_ref(false, arr2_ty);
  let ref2 = checker.intern_ty(Ty::Ref(ref2_id));

  let result = checker.unify(ref1, ref2, dummy_span());

  assert!(result.is_some());

  // α should unify with Int
  let alpha_resolved = checker.resolve_id(alpha);

  assert_eq!(alpha_resolved, int_ty);
}

#[test]
fn test_array_size_mismatch() {
  let mut checker = TyChecker::new();

  let int_ty = checker.s32_type();

  // Array<Int, 5> vs Array<Int, 10>
  let arr1_id = checker.ty_table.intern_array(int_ty, Some(5));
  let arr1 = checker.intern_ty(Ty::Array(arr1_id));

  let arr2_id = checker.ty_table.intern_array(int_ty, Some(10));
  let arr2 = checker.intern_ty(Ty::Array(arr2_id));

  // Should fail - different sizes
  let result = checker.unify(arr1, arr2, dummy_span());

  assert!(result.is_none());
}

// ===== SUBSTITUTION TRANSITIVITY TESTS =====

#[test]
fn test_substitution_chain() {
  let mut checker = TyChecker::new();

  // Create chain: α -> β, β -> Int
  let alpha = checker.fresh_var();
  let beta = checker.fresh_var();
  let int_ty = checker.s32_type();

  // Unify α = β
  let result1 = checker.unify(alpha, beta, dummy_span());

  assert!(result1.is_some());

  // Unify β = Int
  let result2 = checker.unify(beta, int_ty, dummy_span());

  assert!(result2.is_some());

  // Resolve α should give Int (path compression)
  let alpha_resolved = checker.resolve_id(alpha);

  assert_eq!(alpha_resolved, int_ty);

  // Check path compression worked - second resolve should be direct
  let alpha_resolved2 = checker.resolve_id(alpha);

  assert_eq!(alpha_resolved2, int_ty);
}

#[test]
fn test_complex_substitution_chain() {
  let mut checker = TyChecker::new();

  // Create longer chain: α -> β -> γ -> δ -> Int
  let alpha = checker.fresh_var();
  let beta = checker.fresh_var();
  let gamma = checker.fresh_var();
  let delta = checker.fresh_var();
  let int_ty = checker.s32_type();

  checker.unify(alpha, beta, dummy_span()).unwrap();
  checker.unify(beta, gamma, dummy_span()).unwrap();
  checker.unify(gamma, delta, dummy_span()).unwrap();
  checker.unify(delta, int_ty, dummy_span()).unwrap();

  // All should resolve to Int
  assert_eq!(checker.resolve_id(alpha), int_ty);
  assert_eq!(checker.resolve_id(beta), int_ty);
  assert_eq!(checker.resolve_id(gamma), int_ty);
  assert_eq!(checker.resolve_id(delta), int_ty);
}

// ===== INTERNING UNIQUENESS TESTS =====

#[test]
fn test_type_interning() {
  let mut checker = TyChecker::new();

  // Intern same type twice
  let bool1 = checker.bool_type();
  let bool2 = checker.bool_type();

  // Should return same TyId
  assert_eq!(bool1, bool2);

  // Test with more complex types
  let int1 = checker.s32_type();
  let int2 = checker.s32_type();

  assert_eq!(int1, int2);
}

#[test]
fn test_function_interning() {
  let mut checker = TyChecker::new();

  let int_ty = checker.s32_type();
  let bool_ty = checker.bool_type();

  // Create same function type twice: Int -> Bool
  let fun1_id = checker.ty_table.intern_fun(vec![int_ty], bool_ty);
  let fun1 = checker.intern_ty(Ty::Fun(fun1_id));

  let fun2_id = checker.ty_table.intern_fun(vec![int_ty], bool_ty);
  let fun2 = checker.intern_ty(Ty::Fun(fun2_id));

  // Should intern to same TyId (with HashMap optimization)
  assert_eq!(fun1, fun2);
}

#[test]
fn test_array_interning() {
  let mut checker = TyChecker::new();

  let int_ty = checker.s32_type();

  // Create same array type twice
  let arr1_id = checker.ty_table.intern_array(int_ty, None);
  let arr1 = checker.intern_ty(Ty::Array(arr1_id));

  let arr2_id = checker.ty_table.intern_array(int_ty, None);
  let arr2 = checker.intern_ty(Ty::Array(arr2_id));

  // Should intern to same TyId
  assert_eq!(arr1, arr2);
}

#[test]
fn test_ref_interning() {
  let mut checker = TyChecker::new();

  let int_ty = checker.s32_type();

  // Create same ref type twice
  let ref1_id = checker.ty_table.intern_ref(false, int_ty);
  let ref1 = checker.intern_ty(Ty::Ref(ref1_id));

  let ref2_id = checker.ty_table.intern_ref(false, int_ty);
  let ref2 = checker.intern_ty(Ty::Ref(ref2_id));

  // Should intern to same TyId
  assert_eq!(ref1, ref2);
}

#[test]
fn test_inference_vars_not_interned() {
  let mut checker = TyChecker::new();

  // Fresh inference variables should always be unique
  let var1 = checker.fresh_var();
  let var2 = checker.fresh_var();

  assert_ne!(var1, var2);

  // In pure W, all type variables are uniform
  let var3 = checker.fresh_var();
  let var4 = checker.fresh_var();

  assert_ne!(var3, var4);
}

// ===== EDGE CASES AND ERROR HANDLING =====

#[test]
fn test_type_annotation() {
  let mut checker = TyChecker::new();

  // Simulate: (x: s32) = 42
  let annotated = checker.s32_type();
  let inferred = checker.infer_int_literal(42);

  let result = checker.handle_ty_annotation(annotated, inferred, dummy_span());

  assert!(result.is_some());

  // Should unify successfully to s32
  let final_ty = result.unwrap();
  let kind = checker.kind_of(final_ty);

  assert_eq!(
    kind,
    Ty::Int {
      signed: true,
      width: IntWidth::S32
    }
  );
}

#[test]
fn test_type_annotation_mismatch() {
  let mut checker = TyChecker::new();

  // In pure W, literals get fresh type vars that can unify with anything
  // So (x: bool) = 42 would succeed (42's type var unifies with bool)
  // To test actual mismatch, we need concrete types

  // Simulate: (x: bool) = (y: s32) where y is already typed
  let annotated = checker.bool_type();
  let concrete_int = checker.s32_type();

  let result =
    checker.handle_ty_annotation(annotated, concrete_int, dummy_span());

  assert!(result.is_none()); // Type mismatch between bool and s32
}

#[test]
fn test_scope_management() {
  let mut checker = TyChecker::new();
  let mut interner = Interner::new();

  let x_name = interner.intern("x");
  let x_ty = checker.s32_type();

  // Bind in current scope
  checker.bind_var(x_name, x_ty);

  assert_eq!(checker.lookup_var(x_name), Some(x_ty));

  // Push new scope
  checker.push_scope();

  // x still visible
  assert_eq!(checker.lookup_var(x_name), Some(x_ty));

  // Shadow with new binding
  let new_ty = checker.bool_type();

  checker.bind_var(x_name, new_ty);

  assert_eq!(checker.lookup_var(x_name), Some(new_ty));

  // Pop scope
  checker.pop_scope();

  // Original binding restored
  assert_eq!(checker.lookup_var(x_name), Some(x_ty));
}
