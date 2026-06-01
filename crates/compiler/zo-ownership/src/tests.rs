use crate::Ownership;

use zo_error::ErrorKind;
use zo_interner::{Interner, Symbol};
use zo_reporter::collect_errors;
use zo_sir::{Insn, LoadSource, Sir};
use zo_span::Span;
use zo_ty::{SelfKind, TyId};
use zo_ty_checker::TyChecker;
use zo_value::{FunctionKind, Pubness, ValueId};

fn make_sir(instructions: Vec<Insn>) -> Sir {
  let next_value_id = instructions.len() as u32;
  // Synthetic test SIR has no source — ZERO is the
  // "no location" sentinel. These tests assert on error
  // kind, not spans; the span-precision test below builds
  // its own distinct spans.
  let spans = vec![Span::ZERO; instructions.len()];

  Sir {
    instructions,
    spans,
    node_idxs: Vec::new(),
    next_value_id,
    next_label_id: 0,
    node_cursor: 0,
    vec_elem_tys: std::collections::HashMap::new(),
  }
}

fn fundef(name: Symbol, self_kind: SelfKind) -> Insn {
  Insn::FunDef {
    name,
    params: vec![],
    return_ty: TyId(1),
    body_start: 0,
    kind: FunctionKind::UserDefined,
    pubness: Pubness::No,
    self_kind,
    link_name: None,
    owning_pack: None,
    span: Span::ZERO,
    is_test: false,
  }
}

fn load(dst: u32, local: Symbol) -> Insn {
  Insn::Load {
    dst: ValueId(dst),
    src: LoadSource::Local(local),
    ty_id: TyId(1),
  }
}

fn call(dst: u32, name: Symbol, args: Vec<u32>) -> Insn {
  Insn::Call {
    dst: ValueId(dst),
    name,
    callee_pack: None,
    args: args.into_iter().map(ValueId).collect(),
    ty_id: TyId(1),
  }
}

fn store(name: Symbol, value: u32) -> Insn {
  Insn::Store {
    name,
    value: ValueId(value),
    ty_id: TyId(1),
  }
}

fn ret() -> Insn {
  Insn::Return {
    value: None,
    ty_id: TyId(1),
  }
}

/// Runs the pass on a freshly-cleared error channel and returns
/// the kinds it reported. These fragments carry no `Drop`
/// markers, so the destructor lookup never fires and a fresh
/// `TyChecker` suffices.
fn run(sir: &mut Sir, interner: &Interner) -> Vec<ErrorKind> {
  let _ = collect_errors();

  let ty = TyChecker::new();

  Ownership::new(sir, interner, &ty).check();

  collect_errors().iter().map(|e| e.kind()).collect()
}

#[test]
fn double_free_is_reported() {
  let mut interner = Interner::new();
  let free = interner.intern("Vec::free");
  let caller = interner.intern("caller");
  let v = interner.intern("v");

  let mut sir = make_sir(vec![
    fundef(free, SelfKind::Consume),
    ret(),
    fundef(caller, SelfKind::None),
    load(0, v),
    call(1, free, vec![0]),
    load(2, v),
    call(3, free, vec![2]),
    ret(),
  ]);

  // Consuming an already-moved binding is a double-free, not a
  // plain use-after-move.
  assert_eq!(run(&mut sir, &interner), vec![ErrorKind::DoubleFree]);
}

#[test]
fn diagnostic_points_at_use_site_and_move_site() {
  let mut interner = Interner::new();
  let free = interner.intern("Vec::free");
  let caller = interner.intern("caller");
  let v = interner.intern("v");

  // Distinct spans so we can assert which is the primary
  // (offending use) and which is the secondary (first move).
  let first_free = Span::new(10, 6);
  let second_free = Span::new(20, 6);

  let instructions = vec![
    fundef(free, SelfKind::Consume),
    ret(),
    fundef(caller, SelfKind::None),
    load(0, v),
    call(1, free, vec![0]), // first free  — idx 4
    load(2, v),
    call(3, free, vec![2]), // second free — idx 6
    ret(),
  ];
  let mut spans = vec![Span::ZERO; instructions.len()];
  spans[4] = first_free;
  spans[6] = second_free;

  let mut sir = Sir {
    instructions,
    spans,
    node_idxs: Vec::new(),
    next_value_id: 8,
    next_label_id: 0,
    node_cursor: 0,
    vec_elem_tys: std::collections::HashMap::new(),
  };

  let _ = collect_errors();
  let ty = TyChecker::new();
  Ownership::new(&mut sir, &interner, &ty).check();
  let errors = collect_errors();

  assert_eq!(errors.len(), 1);
  assert_eq!(errors[0].kind(), ErrorKind::DoubleFree);
  // Primary points at the offending (second) consume.
  assert_eq!(errors[0].span(), second_free);
  // Secondary points at where it was first moved.
  assert_eq!(errors[0].secondary_span(), Some(first_free));
}

#[test]
fn use_after_free_is_reported() {
  let mut interner = Interner::new();
  let free = interner.intern("Vec::free");
  let len = interner.intern("Vec::len");
  let caller = interner.intern("caller");
  let v = interner.intern("v");

  let mut sir = make_sir(vec![
    fundef(free, SelfKind::Consume),
    ret(),
    fundef(len, SelfKind::Read),
    ret(),
    fundef(caller, SelfKind::None),
    load(0, v),
    call(1, free, vec![0]),
    load(2, v),
    call(3, len, vec![2]),
    ret(),
  ]);

  assert_eq!(run(&mut sir, &interner), vec![ErrorKind::UseAfterMove]);
}

#[test]
fn reassignment_clears_the_move() {
  let mut interner = Interner::new();
  let free = interner.intern("Vec::free");
  let caller = interner.intern("caller");
  let v = interner.intern("v");

  let mut sir = make_sir(vec![
    fundef(free, SelfKind::Consume),
    ret(),
    fundef(caller, SelfKind::None),
    load(0, v),
    call(1, free, vec![0]),
    store(v, 4),
    load(2, v),
    call(3, free, vec![2]),
    ret(),
  ]);

  assert!(run(&mut sir, &interner).is_empty());
}

#[test]
fn non_consuming_calls_never_move() {
  let mut interner = Interner::new();
  let show = interner.intern("show");
  let caller = interner.intern("caller");
  let x = interner.intern("x");

  let mut sir = make_sir(vec![
    fundef(show, SelfKind::Read),
    ret(),
    fundef(caller, SelfKind::None),
    load(0, x),
    call(1, show, vec![0]),
    load(2, x),
    call(3, show, vec![2]),
    ret(),
  ]);

  assert!(run(&mut sir, &interner).is_empty());
}
