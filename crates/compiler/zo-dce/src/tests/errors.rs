//! ```sh
//! cargo test -p zo-dce --lib tests::errors
//! ```

use super::common::make_sir;

use crate::Dce;

use zo_error::ErrorKind;
use zo_interner::Interner;
use zo_reporter::collect_errors;
use zo_sir::Insn;
use zo_ty::TyId;
use zo_value::{FunctionKind, Pubness, ValueId};

#[test]
#[ignore = "warnings disabled — needs Severity::Warning in zo-error"]
fn warns_on_unused_function() {
  let _ = collect_errors();

  let mut interner = Interner::new();
  let dead = interner.intern("dead_fn");
  let main = interner.intern("main");

  let mut sir = make_sir(vec![
    Insn::FunDef {
      name: dead,
      params: vec![],
      return_ty: TyId(1),
      body_start: 1,
      kind: FunctionKind::UserDefined,
      pubness: Pubness::No,
    },
    Insn::Return {
      value: None,
      ty_id: TyId(1),
    },
    Insn::FunDef {
      name: main,
      params: vec![],
      return_ty: TyId(1),
      body_start: 3,
      kind: FunctionKind::UserDefined,
      pubness: Pubness::No,
    },
    Insn::Return {
      value: None,
      ty_id: TyId(1),
    },
  ]);

  Dce::new(&mut sir, main, &interner).eliminate();

  let errors = collect_errors();

  assert!(
    errors.iter().any(|e| e.kind() == ErrorKind::UnusedFunction),
    "Expected UnusedFunction warning, got: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

#[test]
#[ignore = "warnings disabled — needs Severity::Warning in zo-error"]
fn warns_on_unused_variable() {
  let _ = collect_errors();

  let mut interner = Interner::new();
  let main = interner.intern("main");
  let x = interner.intern("x");

  let mut sir = make_sir(vec![
    Insn::FunDef {
      name: main,
      params: vec![],
      return_ty: TyId(1),
      body_start: 1,
      kind: FunctionKind::UserDefined,
      pubness: Pubness::No,
    },
    Insn::ConstInt {
      dst: ValueId(0),
      value: 42,
      ty_id: TyId(8),
    },
    Insn::Store {
      name: x,
      value: ValueId(0),
      ty_id: TyId(8),
    },
    Insn::Return {
      value: None,
      ty_id: TyId(1),
    },
  ]);

  Dce::new(&mut sir, main, &interner).eliminate();

  let errors = collect_errors();

  assert!(
    errors.iter().any(|e| e.kind() == ErrorKind::UnusedVariable),
    "Expected UnusedVariable warning, got: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

#[test]
fn no_warning_when_all_used() {
  let _ = collect_errors();

  let mut interner = Interner::new();
  let main = interner.intern("main");
  let x = interner.intern("x");

  let mut sir = make_sir(vec![
    Insn::FunDef {
      name: main,
      params: vec![],
      return_ty: TyId(1),
      body_start: 1,
      kind: FunctionKind::UserDefined,
      pubness: Pubness::No,
    },
    Insn::ConstInt {
      dst: ValueId(0),
      value: 42,
      ty_id: TyId(8),
    },
    Insn::Store {
      name: x,
      value: ValueId(0),
      ty_id: TyId(8),
    },
    Insn::Load {
      dst: ValueId(1),
      src: zo_sir::LoadSource::Local(x),
      ty_id: TyId(8),
    },
    Insn::Return {
      value: Some(ValueId(1)),
      ty_id: TyId(8),
    },
  ]);

  Dce::new(&mut sir, main, &interner).eliminate();

  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "Expected no warnings, got: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}
