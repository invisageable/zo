//! ```sh
//! cargo test -p zo-ty-checker --lib tests::errors
//! ```
//!
//! Error-path tests for `zo-ty-checker`.
//!
//! Covers every `ErrorKind` that the type checker raises:
//!
//!   - `TypeMismatch` — concrete vs concrete, mutability, non-numeric ops
//!   - `InfiniteType` — occurs check failures
//!   - `ArgumentCountMismatch` — function arity
//!   - `ArraySizeMismatch` — fixed-size arrays
//!   - `UndefinedVariable` — missing bindings

use crate::TyChecker;
use crate::tests::common::{
  assert_lookup_error, assert_unify_error, assert_unify_ok,
};

use zo_error::ErrorKind;
use zo_interner::Interner;
use zo_reporter::collect_errors;
use zo_sir::{BinOp, UnOp};
use zo_span::Span;
use zo_ty::{Mutability, Ty};

// ===== TypeMismatch =====

#[test]
fn error_type_mismatch_bool_vs_int() {
  let mut checker = TyChecker::new();
  let bool_ty = checker.bool_type();
  let int_ty = checker.s32_type();

  assert_unify_error(&mut checker, bool_ty, int_ty, ErrorKind::TypeMismatch);
}

#[test]
fn error_type_mismatch_str_vs_bool() {
  let mut checker = TyChecker::new();
  let str_ty = checker.str_type();
  let bool_ty = checker.bool_type();

  assert_unify_error(&mut checker, str_ty, bool_ty, ErrorKind::TypeMismatch);
}

#[test]
fn error_type_mismatch_char_vs_int() {
  let mut checker = TyChecker::new();
  let char_ty = checker.char_type();
  let int_ty = checker.s32_type();

  assert_unify_error(&mut checker, char_ty, int_ty, ErrorKind::TypeMismatch);
}

#[test]
fn error_type_mismatch_unit_vs_bool() {
  let mut checker = TyChecker::new();
  let unit_ty = checker.unit_type();
  let bool_ty = checker.bool_type();

  assert_unify_error(&mut checker, unit_ty, bool_ty, ErrorKind::TypeMismatch);
}

#[test]
fn error_type_mismatch_ref_mutability() {
  let mut checker = TyChecker::new();
  let int_ty = checker.s32_type();

  let ref_imm_id = checker.ty_table.intern_ref(Mutability::No, int_ty);
  let ref_imm = checker.intern_ty(Ty::Ref(ref_imm_id));

  let ref_mut_id = checker.ty_table.intern_ref(Mutability::Yes, int_ty);
  let ref_mut = checker.intern_ty(Ty::Ref(ref_mut_id));

  assert_unify_error(&mut checker, ref_imm, ref_mut, ErrorKind::TypeMismatch);
}

#[test]
fn error_type_mismatch_tuple_arity() {
  let mut checker = TyChecker::new();
  let int_ty = checker.s32_type();
  let bool_ty = checker.bool_type();

  let tup1_id = checker.ty_table.intern_tuple(vec![int_ty]);
  let tup1 = checker.intern_ty(Ty::Tuple(tup1_id));

  let tup2_id = checker.ty_table.intern_tuple(vec![int_ty, bool_ty]);
  let tup2 = checker.intern_ty(Ty::Tuple(tup2_id));

  assert_unify_error(&mut checker, tup1, tup2, ErrorKind::TypeMismatch);
}

#[test]
fn error_type_mismatch_tuple_element() {
  let mut checker = TyChecker::new();
  let int_ty = checker.s32_type();
  let bool_ty = checker.bool_type();

  let tup1_id = checker.ty_table.intern_tuple(vec![int_ty, int_ty]);
  let tup1 = checker.intern_ty(Ty::Tuple(tup1_id));

  let tup2_id = checker.ty_table.intern_tuple(vec![int_ty, bool_ty]);
  let tup2 = checker.intern_ty(Ty::Tuple(tup2_id));

  assert_unify_error(&mut checker, tup1, tup2, ErrorKind::TypeMismatch);
}

#[test]
fn error_type_mismatch_param_vs_concrete() {
  let mut checker = TyChecker::new();
  let mut interner = Interner::new();

  let t_sym = interner.intern("T");
  let param_t = checker.intern_ty(Ty::Param(t_sym));
  let int_ty = checker.s32_type();

  assert_unify_error(&mut checker, param_t, int_ty, ErrorKind::TypeMismatch);
}

#[test]
fn error_type_mismatch_param_vs_different_param() {
  let mut checker = TyChecker::new();
  let mut interner = Interner::new();

  let t_sym = interner.intern("T");
  let u_sym = interner.intern("U");
  let param_t = checker.intern_ty(Ty::Param(t_sym));
  let param_u = checker.intern_ty(Ty::Param(u_sym));

  assert_unify_error(&mut checker, param_t, param_u, ErrorKind::TypeMismatch);
}

// ===== TypeMismatch via operators =====

#[test]
fn error_type_mismatch_add_bool() {
  let mut checker = TyChecker::new();
  let _ = collect_errors();

  let bool_ty = checker.bool_type();
  let result = checker.infer_binop(BinOp::Add, bool_ty, bool_ty, Span::ZERO);

  assert!(result.is_none());

  let errors = collect_errors();

  assert!(
    errors.iter().any(|e| e.kind() == ErrorKind::TypeMismatch),
    "Expected TypeMismatch for bool + bool, got: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

#[test]
fn error_type_mismatch_logical_and_int() {
  let mut checker = TyChecker::new();
  let _ = collect_errors();

  let int_ty = checker.s32_type();
  let result = checker.infer_binop(BinOp::And, int_ty, int_ty, Span::ZERO);

  assert!(result.is_none());

  let errors = collect_errors();

  assert!(
    errors.iter().any(|e| e.kind() == ErrorKind::TypeMismatch),
    "Expected TypeMismatch for int && int, got: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

#[test]
fn error_type_mismatch_bitwise_bool() {
  let mut checker = TyChecker::new();
  let _ = collect_errors();

  let bool_ty = checker.bool_type();
  let result = checker.infer_binop(BinOp::BitAnd, bool_ty, bool_ty, Span::ZERO);

  assert!(result.is_none());

  let errors = collect_errors();

  assert!(
    errors.iter().any(|e| e.kind() == ErrorKind::TypeMismatch),
    "Expected TypeMismatch for bool & bool, got: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

#[test]
fn error_type_mismatch_concat_non_str() {
  let mut checker = TyChecker::new();
  let _ = collect_errors();

  let int_ty = checker.s32_type();
  let str_ty = checker.str_type();
  let result = checker.infer_binop(BinOp::Concat, int_ty, str_ty, Span::ZERO);

  assert!(result.is_none());

  let errors = collect_errors();

  assert!(
    errors.iter().any(|e| e.kind() == ErrorKind::TypeMismatch),
    "Expected TypeMismatch for int ++ str, got: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

#[test]
fn error_type_mismatch_neg_bool() {
  let mut checker = TyChecker::new();
  let _ = collect_errors();

  let bool_ty = checker.bool_type();
  let result = checker.infer_unop(UnOp::Neg, bool_ty, Span::ZERO);

  assert!(result.is_none());

  let errors = collect_errors();

  assert!(
    errors.iter().any(|e| e.kind() == ErrorKind::TypeMismatch),
    "Expected TypeMismatch for -bool, got: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

#[test]
fn error_type_mismatch_bitnot_bool() {
  let mut checker = TyChecker::new();
  let _ = collect_errors();

  let bool_ty = checker.bool_type();
  let result = checker.infer_unop(UnOp::BitNot, bool_ty, Span::ZERO);

  assert!(result.is_none());

  let errors = collect_errors();

  assert!(
    errors.iter().any(|e| e.kind() == ErrorKind::TypeMismatch),
    "Expected TypeMismatch for ~bool, got: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

// ===== InfiniteType (occurs check) =====

#[test]
fn error_infinite_type_direct() {
  let mut checker = TyChecker::new();
  let alpha = checker.fresh_var();

  let arr_id = checker.ty_table.intern_array(alpha, None);
  let arr_ty = checker.intern_ty(Ty::Array(arr_id));

  assert_unify_error(&mut checker, alpha, arr_ty, ErrorKind::InfiniteType);
}

#[test]
fn error_infinite_type_in_function() {
  let mut checker = TyChecker::new();
  let alpha = checker.fresh_var();
  let beta = checker.fresh_var();

  let fun_id = checker.ty_table.intern_fun(vec![alpha], beta);
  let fun_ty = checker.intern_ty(Ty::Fun(fun_id));

  assert_unify_error(&mut checker, alpha, fun_ty, ErrorKind::InfiniteType);
}

#[test]
fn error_infinite_type_in_ref() {
  let mut checker = TyChecker::new();
  let alpha = checker.fresh_var();

  let ref_id = checker.ty_table.intern_ref(Mutability::No, alpha);
  let ref_ty = checker.intern_ty(Ty::Ref(ref_id));

  assert_unify_error(&mut checker, alpha, ref_ty, ErrorKind::InfiniteType);
}

#[test]
fn error_infinite_type_in_tuple() {
  let mut checker = TyChecker::new();
  let alpha = checker.fresh_var();
  let int_ty = checker.s32_type();

  let tup_id = checker.ty_table.intern_tuple(vec![alpha, int_ty]);
  let tup_ty = checker.intern_ty(Ty::Tuple(tup_id));

  assert_unify_error(&mut checker, alpha, tup_ty, ErrorKind::InfiniteType);
}

#[test]
fn error_infinite_type_nested() {
  let mut checker = TyChecker::new();
  let alpha = checker.fresh_var();

  // α = Ref<Array<α>>
  let arr_id = checker.ty_table.intern_array(alpha, None);
  let arr_ty = checker.intern_ty(Ty::Array(arr_id));
  let ref_id = checker.ty_table.intern_ref(Mutability::No, arr_ty);
  let ref_ty = checker.intern_ty(Ty::Ref(ref_id));

  assert_unify_error(&mut checker, alpha, ref_ty, ErrorKind::InfiniteType);
}

// ===== ArgumentCountMismatch =====

#[test]
fn error_arg_count_mismatch_1_vs_2() {
  let mut checker = TyChecker::new();
  let int_ty = checker.s32_type();
  let bool_ty = checker.bool_type();

  let fun1_id = checker.ty_table.intern_fun(vec![int_ty], bool_ty);
  let fun1 = checker.intern_ty(Ty::Fun(fun1_id));

  let fun2_id = checker.ty_table.intern_fun(vec![int_ty, int_ty], bool_ty);
  let fun2 = checker.intern_ty(Ty::Fun(fun2_id));

  assert_unify_error(
    &mut checker,
    fun1,
    fun2,
    ErrorKind::ArgumentCountMismatch,
  );
}

#[test]
fn error_arg_count_mismatch_0_vs_1() {
  let mut checker = TyChecker::new();
  let bool_ty = checker.bool_type();
  let int_ty = checker.s32_type();

  let fun0_id = checker.ty_table.intern_fun(vec![], bool_ty);
  let fun0 = checker.intern_ty(Ty::Fun(fun0_id));

  let fun1_id = checker.ty_table.intern_fun(vec![int_ty], bool_ty);
  let fun1 = checker.intern_ty(Ty::Fun(fun1_id));

  assert_unify_error(
    &mut checker,
    fun0,
    fun1,
    ErrorKind::ArgumentCountMismatch,
  );
}

// ===== ArraySizeMismatch =====

#[test]
fn error_array_size_mismatch() {
  let mut checker = TyChecker::new();
  let int_ty = checker.s32_type();

  let arr5_id = checker.ty_table.intern_array(int_ty, Some(5));
  let arr5 = checker.intern_ty(Ty::Array(arr5_id));

  let arr10_id = checker.ty_table.intern_array(int_ty, Some(10));
  let arr10 = checker.intern_ty(Ty::Array(arr10_id));

  assert_unify_error(&mut checker, arr5, arr10, ErrorKind::ArraySizeMismatch);
}

#[test]
fn error_array_size_fixed_vs_dynamic() {
  let mut checker = TyChecker::new();
  let int_ty = checker.s32_type();

  let arr_fixed_id = checker.ty_table.intern_array(int_ty, Some(5));
  let arr_fixed = checker.intern_ty(Ty::Array(arr_fixed_id));

  let arr_dyn_id = checker.ty_table.intern_array(int_ty, None);
  let arr_dyn = checker.intern_ty(Ty::Array(arr_dyn_id));

  assert_unify_error(
    &mut checker,
    arr_fixed,
    arr_dyn,
    ErrorKind::ArraySizeMismatch,
  );
}

// ===== UndefinedVariable =====

#[test]
fn error_undefined_variable() {
  let mut checker = TyChecker::new();
  let mut interner = Interner::new();

  let unknown = interner.intern("unknown_var");

  assert_lookup_error(&mut checker, unknown, ErrorKind::UndefinedVariable);
}

#[test]
fn error_undefined_variable_after_pop_scope() {
  let mut checker = TyChecker::new();
  let mut interner = Interner::new();

  let x = interner.intern("x");
  let int_ty = checker.s32_type();

  checker.push_scope();
  checker.bind_var(x, int_ty);
  checker.pop_scope();

  // x no longer visible.
  assert_lookup_error(&mut checker, x, ErrorKind::UndefinedVariable);
}

// ===== Compound: error does not cascade =====

#[test]
fn error_absorbs_via_ty_error() {
  let mut checker = TyChecker::new();
  let error_ty = checker.error_type();
  let int_ty = checker.s32_type();

  // Ty::Error unifies with anything — no error reported.
  let result = assert_unify_ok(&mut checker, error_ty, int_ty);

  assert_eq!(result, int_ty);
}

#[test]
fn error_absorbs_symmetric() {
  let mut checker = TyChecker::new();
  let error_ty = checker.error_type();
  let bool_ty = checker.bool_type();

  let result = assert_unify_ok(&mut checker, bool_ty, error_ty);

  assert_eq!(result, bool_ty);
}
