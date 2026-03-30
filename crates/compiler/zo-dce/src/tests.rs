pub(crate) mod common;
pub(crate) mod errors;

use crate::Dce;

use common::{call, fun, fun_names, make_sir};

use zo_interner::{Interner, Symbol};
use zo_sir::{BinOp, Insn};
use zo_ty::TyId;
use zo_value::{Pubness, ValueId};

// ===== BASIC ELIMINATION =====

#[test]
fn removes_private_uncalled_function() {
  let mut i = Interner::new();
  let show = i.intern("show");
  let showln = i.intern("showln");
  let main = i.intern("main");

  let mut insns = vec![];

  // show — private, never called → removed.
  insns.extend(fun(show, Pubness::No, vec![]));
  // showln — called by main → kept.
  insns.extend(fun(showln, Pubness::No, vec![]));
  // main — entry point → kept.
  insns.extend(fun(main, Pubness::No, vec![call(showln)]));

  let mut sir = make_sir(insns);

  Dce::new(&mut sir, main).eliminate();

  assert_eq!(fun_names(&sir), vec![showln, main]);
}

#[test]
fn keeps_all_when_all_called() {
  let mut i = Interner::new();
  let foo = i.intern("foo");
  let bar = i.intern("bar");
  let main = i.intern("main");

  let mut insns = vec![];

  insns.extend(fun(foo, Pubness::No, vec![]));
  insns.extend(fun(bar, Pubness::No, vec![]));
  insns.extend(fun(main, Pubness::No, vec![call(foo), call(bar)]));

  let mut sir = make_sir(insns);

  Dce::new(&mut sir, main).eliminate();

  assert_eq!(fun_names(&sir), vec![foo, bar, main]);
}

#[test]
fn empty_sir_is_noop() {
  let mut sir = make_sir(vec![]);

  Dce::new(&mut sir, Symbol(0)).eliminate();

  assert!(sir.instructions.is_empty());
}

#[test]
fn no_functions_preserves_top_level() {
  let mut sir = make_sir(vec![Insn::ModuleLoad {
    path: vec![],
    imported_symbols: vec![],
  }]);

  Dce::new(&mut sir, Symbol(0)).eliminate();

  assert_eq!(sir.instructions.len(), 1);
}

// ===== TRANSITIVE REACHABILITY =====

#[test]
fn transitive_call_chain_kept() {
  let mut i = Interner::new();
  let a = i.intern("a");
  let b = i.intern("b");
  let c = i.intern("c");
  let main = i.intern("main");

  // main → a → b → c. All reachable.
  let mut insns = vec![];

  insns.extend(fun(c, Pubness::No, vec![]));
  insns.extend(fun(b, Pubness::No, vec![call(c)]));
  insns.extend(fun(a, Pubness::No, vec![call(b)]));
  insns.extend(fun(main, Pubness::No, vec![call(a)]));

  let mut sir = make_sir(insns);

  Dce::new(&mut sir, main).eliminate();

  assert_eq!(fun_names(&sir), vec![c, b, a, main]);
}

#[test]
fn dead_chain_removed() {
  let mut i = Interner::new();
  let dead_a = i.intern("dead_a");
  let dead_b = i.intern("dead_b");
  let alive = i.intern("alive");
  let main = i.intern("main");

  // dead_a → dead_b (neither reachable from main).
  // main → alive.
  let mut insns = vec![];

  insns.extend(fun(dead_b, Pubness::No, vec![]));
  insns.extend(fun(dead_a, Pubness::No, vec![call(dead_b)]));
  insns.extend(fun(alive, Pubness::No, vec![]));
  insns.extend(fun(main, Pubness::No, vec![call(alive)]));

  let mut sir = make_sir(insns);

  Dce::new(&mut sir, main).eliminate();

  // dead_a and dead_b should both be gone.
  assert_eq!(fun_names(&sir), vec![alive, main]);
}

#[test]
fn diamond_call_graph() {
  let mut i = Interner::new();
  let leaf = i.intern("leaf");
  let left = i.intern("left");
  let right = i.intern("right");
  let main = i.intern("main");

  // main → left → leaf
  // main → right → leaf
  let mut insns = vec![];

  insns.extend(fun(leaf, Pubness::No, vec![]));
  insns.extend(fun(left, Pubness::No, vec![call(leaf)]));
  insns.extend(fun(right, Pubness::No, vec![call(leaf)]));
  insns.extend(fun(main, Pubness::No, vec![call(left), call(right)]));

  let mut sir = make_sir(insns);

  Dce::new(&mut sir, main).eliminate();

  assert_eq!(fun_names(&sir), vec![leaf, left, right, main]);
}

// ===== PUB FUNCTION PRESERVATION =====

#[test]
fn pub_function_kept_even_if_uncalled() {
  let mut i = Interner::new();
  let api = i.intern("api");
  let main = i.intern("main");

  let mut insns = vec![];

  // api — pub but never called locally → kept.
  insns.extend(fun(api, Pubness::Yes, vec![]));
  insns.extend(fun(main, Pubness::No, vec![]));

  let mut sir = make_sir(insns);

  Dce::new(&mut sir, main).eliminate();

  assert_eq!(fun_names(&sir), vec![api, main]);
}

#[test]
fn pub_function_callees_transitively_kept() {
  let mut i = Interner::new();
  let helper = i.intern("helper");
  let api = i.intern("api");
  let main = i.intern("main");

  // api (pub) → helper. helper is reachable via pub root.
  let mut insns = vec![];

  insns.extend(fun(helper, Pubness::No, vec![]));
  insns.extend(fun(api, Pubness::Yes, vec![call(helper)]));
  insns.extend(fun(main, Pubness::No, vec![]));

  let mut sir = make_sir(insns);

  Dce::new(&mut sir, main).eliminate();

  assert_eq!(fun_names(&sir), vec![helper, api, main]);
}

// ===== EDGE CASES =====

#[test]
fn recursive_function_kept() {
  let mut i = Interner::new();
  let fib = i.intern("fib");
  let main = i.intern("main");

  // fib calls itself. main → fib.
  let mut insns = vec![];

  insns.extend(fun(fib, Pubness::No, vec![call(fib)]));
  insns.extend(fun(main, Pubness::No, vec![call(fib)]));

  let mut sir = make_sir(insns);

  Dce::new(&mut sir, main).eliminate();

  assert_eq!(fun_names(&sir), vec![fib, main]);
}

#[test]
fn mutual_recursion_kept() {
  let mut i = Interner::new();
  let ping = i.intern("ping");
  let pong = i.intern("pong");
  let main = i.intern("main");

  // ping → pong, pong → ping. main → ping.
  let mut insns = vec![];

  insns.extend(fun(ping, Pubness::No, vec![call(pong)]));
  insns.extend(fun(pong, Pubness::No, vec![call(ping)]));
  insns.extend(fun(main, Pubness::No, vec![call(ping)]));

  let mut sir = make_sir(insns);

  Dce::new(&mut sir, main).eliminate();

  assert_eq!(fun_names(&sir), vec![ping, pong, main]);
}

#[test]
fn only_main_no_other_functions() {
  let mut i = Interner::new();
  let main = i.intern("main");

  let mut insns = vec![];

  insns.extend(fun(main, Pubness::No, vec![]));

  let mut sir = make_sir(insns);

  Dce::new(&mut sir, main).eliminate();

  assert_eq!(fun_names(&sir), vec![main]);
}

// ===== DEAD INSTRUCTION ELIMINATION =====

#[test]
fn dead_insn_unused_const() {
  let mut i = Interner::new();
  let main = i.intern("main");
  let x = i.intern("x");

  let mut sir = make_sir(vec![
    Insn::FunDef {
      name: main,
      params: vec![],
      return_ty: TyId(1),
      body_start: 1,
      kind: zo_value::FunctionKind::UserDefined,
      pubness: Pubness::No,
    },
    // %0 = const 42 — used by store.
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
    // %1 = const 99 — UNUSED, should be eliminated.
    Insn::ConstInt {
      dst: ValueId(1),
      value: 99,
      ty_id: TyId(8),
    },
    Insn::Return {
      value: None,
      ty_id: TyId(1),
    },
  ]);

  let before = sir.instructions.len();

  Dce::new(&mut sir, main).eliminate();

  // The unused const 99 should be removed.
  assert!(
    sir.instructions.len() < before,
    "Expected dead instruction to be removed"
  );

  // const 42, store, and return should remain.
  assert!(
    !sir
      .instructions
      .iter()
      .any(|insn| { matches!(insn, Insn::ConstInt { value: 99, .. }) })
  );
}

#[test]
fn dead_insn_chain() {
  let mut i = Interner::new();
  let main = i.intern("main");

  let mut sir = make_sir(vec![
    Insn::FunDef {
      name: main,
      params: vec![],
      return_ty: TyId(1),
      body_start: 1,
      kind: zo_value::FunctionKind::UserDefined,
      pubness: Pubness::No,
    },
    // %0 = const 1 — used only by %2.
    Insn::ConstInt {
      dst: ValueId(0),
      value: 1,
      ty_id: TyId(8),
    },
    // %1 = const 2 — used only by %2.
    Insn::ConstInt {
      dst: ValueId(1),
      value: 2,
      ty_id: TyId(8),
    },
    // %2 = add %0, %1 — UNUSED. entire chain is dead.
    Insn::BinOp {
      dst: ValueId(2),
      op: BinOp::Add,
      lhs: ValueId(0),
      rhs: ValueId(1),
      ty_id: TyId(8),
    },
    Insn::Return {
      value: None,
      ty_id: TyId(1),
    },
  ]);

  Dce::new(&mut sir, main).eliminate();

  // Only FunDef + Return should survive.
  assert_eq!(sir.instructions.len(), 2);
  assert!(matches!(&sir.instructions[0], Insn::FunDef { .. }));
  assert!(matches!(&sir.instructions[1], Insn::Return { .. }));
}

#[test]
fn dead_insn_preserves_calls() {
  let mut i = Interner::new();
  let main = i.intern("main");
  let showln = i.intern("showln");

  let mut sir = make_sir(vec![
    Insn::FunDef {
      name: main,
      params: vec![],
      return_ty: TyId(1),
      body_start: 1,
      kind: zo_value::FunctionKind::UserDefined,
      pubness: Pubness::No,
    },
    // Call is impure — must not be removed even if dst unused.
    Insn::Call {
      dst: ValueId(0),
      name: showln,
      args: vec![],
      ty_id: TyId(1),
    },
    Insn::Return {
      value: None,
      ty_id: TyId(1),
    },
  ]);

  let before = sir.instructions.len();

  Dce::new(&mut sir, main).eliminate();

  // Nothing removed — Call is impure.
  assert_eq!(sir.instructions.len(), before);
}

// ===== DEAD VARIABLE (STORE) ELIMINATION =====
// Disabled: insn_var_use needs complete var-use extraction.

#[test]
#[ignore = "dead variable pass disabled — incomplete var-use tracking"]
fn dead_var_unused_store() {
  let mut i = Interner::new();
  let main = i.intern("main");
  let x = i.intern("x");
  let y = i.intern("y");

  let mut sir = make_sir(vec![
    Insn::FunDef {
      name: main,
      params: vec![],
      return_ty: TyId(1),
      body_start: 1,
      kind: zo_value::FunctionKind::UserDefined,
      pubness: Pubness::No,
    },
    // Store x — never loaded → dead.
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
    // Store y — loaded by return → alive.
    Insn::ConstInt {
      dst: ValueId(1),
      value: 99,
      ty_id: TyId(8),
    },
    Insn::Store {
      name: y,
      value: ValueId(1),
      ty_id: TyId(8),
    },
    Insn::Load {
      dst: ValueId(2),
      src: zo_sir::LoadSource::Local(y),
      ty_id: TyId(8),
    },
    Insn::Return {
      value: Some(ValueId(2)),
      ty_id: TyId(8),
    },
  ]);

  Dce::new(&mut sir, main).eliminate();

  // Store x should be gone. Store y should remain.
  let store_names: Vec<_> = sir
    .instructions
    .iter()
    .filter_map(|insn| {
      if let Insn::Store { name, .. } = insn {
        Some(*name)
      } else {
        None
      }
    })
    .collect();

  assert_eq!(store_names, vec![y]);
}

#[test]
#[ignore = "dead variable pass disabled — incomplete var-use tracking"]
fn dead_var_overwritten_store() {
  let mut i = Interner::new();
  let main = i.intern("main");
  let x = i.intern("x");

  let mut sir = make_sir(vec![
    Insn::FunDef {
      name: main,
      params: vec![],
      return_ty: TyId(1),
      body_start: 1,
      kind: zo_value::FunctionKind::UserDefined,
      pubness: Pubness::No,
    },
    // First store to x — overwritten before any load → dead.
    Insn::ConstInt {
      dst: ValueId(0),
      value: 1,
      ty_id: TyId(8),
    },
    Insn::Store {
      name: x,
      value: ValueId(0),
      ty_id: TyId(8),
    },
    // Second store to x — this one is loaded.
    Insn::ConstInt {
      dst: ValueId(1),
      value: 2,
      ty_id: TyId(8),
    },
    Insn::Store {
      name: x,
      value: ValueId(1),
      ty_id: TyId(8),
    },
    Insn::Load {
      dst: ValueId(2),
      src: zo_sir::LoadSource::Local(x),
      ty_id: TyId(8),
    },
    Insn::Return {
      value: Some(ValueId(2)),
      ty_id: TyId(8),
    },
  ]);

  Dce::new(&mut sir, main).eliminate();

  // First store (value 1) should be dead. Second (value 2) alive.
  let store_values: Vec<_> = sir
    .instructions
    .iter()
    .filter_map(|insn| {
      if let Insn::Store { value, .. } = insn {
        Some(*value)
      } else {
        None
      }
    })
    .collect();

  assert_eq!(store_values, vec![ValueId(1)]);
}

// ===== UNREACHABLE CODE AFTER RETURN =====

#[test]
fn unreachable_after_return() {
  let mut i = Interner::new();
  let main = i.intern("main");
  let x = i.intern("x");

  let mut sir = make_sir(vec![
    Insn::FunDef {
      name: main,
      params: vec![],
      return_ty: TyId(1),
      body_start: 1,
      kind: zo_value::FunctionKind::UserDefined,
      pubness: Pubness::No,
    },
    Insn::Return {
      value: None,
      ty_id: TyId(1),
    },
    // Everything after Return is unreachable.
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
  ]);

  Dce::new(&mut sir, main).eliminate();

  // Only FunDef + Return should remain.
  assert_eq!(sir.instructions.len(), 2);
  assert!(matches!(&sir.instructions[0], Insn::FunDef { .. }));
  assert!(matches!(&sir.instructions[1], Insn::Return { .. }));
}

#[test]
fn unreachable_stops_at_label() {
  let mut i = Interner::new();
  let main = i.intern("main");

  let mut sir = make_sir(vec![
    Insn::FunDef {
      name: main,
      params: vec![],
      return_ty: TyId(1),
      body_start: 1,
      kind: zo_value::FunctionKind::UserDefined,
      pubness: Pubness::No,
    },
    Insn::Return {
      value: None,
      ty_id: TyId(1),
    },
    // Dead instruction between Return and Label.
    Insn::ConstInt {
      dst: ValueId(0),
      value: 99,
      ty_id: TyId(8),
    },
    // Label ends the dead zone — kept.
    Insn::Label { id: 0 },
    // This is after the label — kept.
    Insn::ConstInt {
      dst: ValueId(1),
      value: 100,
      ty_id: TyId(8),
    },
  ]);

  Dce::new(&mut sir, main).eliminate();

  // const 99 (between Return and Label) should be gone.
  assert!(
    !sir
      .instructions
      .iter()
      .any(|insn| { matches!(insn, Insn::ConstInt { value: 99, .. }) })
  );

  // Label should survive (ends the dead zone).
  assert!(
    sir
      .instructions
      .iter()
      .any(|insn| { matches!(insn, Insn::Label { .. }) })
  );
}

#[test]
fn unreachable_empty_sir_is_noop() {
  let mut sir = make_sir(vec![]);

  Dce::new(&mut sir, Symbol(0)).eliminate();

  assert!(sir.instructions.is_empty());
}
