pub(crate) mod common;
pub(crate) mod errors;

use crate::Dce;

use common::{call, fun, fun_names, make_sir};

use zo_interner::{Interner, Symbol};
use zo_sir::{BinOp, ImportKind, Insn};
use zo_span::Span;
use zo_ty::{SelfKind, TyId};
use zo_value::{Pubness, ValueId};

// ===== BASIC ELIMINATION =====

#[test]
fn removes_private_uncalled_function() {
  let mut interner = Interner::new();
  let show = interner.intern("show");
  let showln = interner.intern("showln");
  let main = interner.intern("main");

  let mut insns = vec![];

  insns.extend(fun(show, Pubness::No, vec![]));
  insns.extend(fun(showln, Pubness::No, vec![]));
  insns.extend(fun(main, Pubness::No, vec![call(showln)]));

  let mut sir = make_sir(insns);

  Dce::new(&mut sir, vec![main], &interner).eliminate();

  assert_eq!(fun_names(&sir), vec![showln, main]);
}

#[test]
fn keeps_all_when_all_called() {
  let mut interner = Interner::new();
  let foo = interner.intern("foo");
  let bar = interner.intern("bar");
  let main = interner.intern("main");

  let mut insns = vec![];

  insns.extend(fun(foo, Pubness::No, vec![]));
  insns.extend(fun(bar, Pubness::No, vec![]));
  insns.extend(fun(main, Pubness::No, vec![call(foo), call(bar)]));

  let mut sir = make_sir(insns);

  Dce::new(&mut sir, vec![main], &interner).eliminate();

  assert_eq!(fun_names(&sir), vec![foo, bar, main]);
}

#[test]
fn empty_sir_is_noop() {
  let interner = Interner::new();
  let mut sir = make_sir(vec![]);

  Dce::new(&mut sir, vec![Symbol(0)], &interner).eliminate();

  assert!(sir.instructions.is_empty());
}

#[test]
fn no_functions_preserves_top_level() {
  let interner = Interner::new();
  let mut sir = make_sir(vec![Insn::ModuleLoad {
    path: vec![],
    kind: ImportKind::Qualified,
    pubness: Pubness::No,
  }]);

  Dce::new(&mut sir, vec![Symbol(0)], &interner).eliminate();

  assert_eq!(sir.instructions.len(), 1);
}

// ===== TRANSITIVE REACHABILITY =====

#[test]
fn transitive_call_chain_kept() {
  let mut interner = Interner::new();
  let a = interner.intern("a");
  let b = interner.intern("b");
  let c = interner.intern("c");
  let main = interner.intern("main");

  // main → a → b → c. All reachable.
  let mut insns = vec![];

  insns.extend(fun(c, Pubness::No, vec![]));
  insns.extend(fun(b, Pubness::No, vec![call(c)]));
  insns.extend(fun(a, Pubness::No, vec![call(b)]));
  insns.extend(fun(main, Pubness::No, vec![call(a)]));

  let mut sir = make_sir(insns);

  Dce::new(&mut sir, vec![main], &interner).eliminate();

  assert_eq!(fun_names(&sir), vec![c, b, a, main]);
}

#[test]
fn dead_chain_removed() {
  let mut interner = Interner::new();
  let dead_a = interner.intern("dead_a");
  let dead_b = interner.intern("dead_b");
  let alive = interner.intern("alive");
  let main = interner.intern("main");

  // dead_a → dead_b (neither reachable from main).
  // main → alive.
  let mut insns = vec![];

  insns.extend(fun(dead_b, Pubness::No, vec![]));
  insns.extend(fun(dead_a, Pubness::No, vec![call(dead_b)]));
  insns.extend(fun(alive, Pubness::No, vec![]));
  insns.extend(fun(main, Pubness::No, vec![call(alive)]));

  let mut sir = make_sir(insns);

  Dce::new(&mut sir, vec![main], &interner).eliminate();

  // dead_a and dead_b should both be gone.
  assert_eq!(fun_names(&sir), vec![alive, main]);
}

#[test]
fn diamond_call_graph() {
  let mut interner = Interner::new();
  let leaf = interner.intern("leaf");
  let left = interner.intern("left");
  let right = interner.intern("right");
  let main = interner.intern("main");

  // main → left → leaf
  // main → right → leaf
  let mut insns = vec![];

  insns.extend(fun(leaf, Pubness::No, vec![]));
  insns.extend(fun(left, Pubness::No, vec![call(leaf)]));
  insns.extend(fun(right, Pubness::No, vec![call(leaf)]));
  insns.extend(fun(main, Pubness::No, vec![call(left), call(right)]));

  let mut sir = make_sir(insns);

  Dce::new(&mut sir, vec![main], &interner).eliminate();

  assert_eq!(fun_names(&sir), vec![leaf, left, right, main]);
}

// ===== PUB FUNCTION PRESERVATION =====

#[test]
fn pub_function_kept_even_if_uncalled() {
  // Default Dce setup (no preload packs) keeps every
  // `pub fun` as a root — user `pub` declarations get
  // reached through dynamic dispatch (`showln(struct)`
  // → `Type::show`) that the static call graph misses.
  let mut interner = Interner::new();
  let api = interner.intern("api");
  let main = interner.intern("main");

  let mut insns = vec![];

  insns.extend(fun(api, Pubness::Yes, vec![]));
  insns.extend(fun(main, Pubness::No, vec![]));

  let mut sir = make_sir(insns);

  Dce::new(&mut sir, vec![main], &interner).eliminate();

  assert_eq!(fun_names(&sir), vec![api, main]);
}

#[test]
fn pub_function_callees_transitively_kept() {
  let mut interner = Interner::new();
  let helper = interner.intern("helper");
  let api = interner.intern("api");
  let main = interner.intern("main");

  let mut insns = vec![];

  insns.extend(fun(helper, Pubness::No, vec![]));
  insns.extend(fun(api, Pubness::Yes, vec![call(helper)]));
  insns.extend(fun(main, Pubness::No, vec![]));

  let mut sir = make_sir(insns);

  Dce::new(&mut sir, vec![main], &interner).eliminate();

  assert_eq!(fun_names(&sir), vec![helper, api, main]);
}

// ===== EDGE CASES =====

#[test]
fn recursive_function_kept() {
  let mut interner = Interner::new();
  let fib = interner.intern("fib");
  let main = interner.intern("main");

  // fib calls itself. main → fib.
  let mut insns = vec![];

  insns.extend(fun(fib, Pubness::No, vec![call(fib)]));
  insns.extend(fun(main, Pubness::No, vec![call(fib)]));

  let mut sir = make_sir(insns);

  Dce::new(&mut sir, vec![main], &interner).eliminate();

  assert_eq!(fun_names(&sir), vec![fib, main]);
}

#[test]
fn mutual_recursion_kept() {
  let mut interner = Interner::new();
  let ping = interner.intern("ping");
  let pong = interner.intern("pong");
  let main = interner.intern("main");

  // ping → pong, pong → ping. main → ping.
  let mut insns = vec![];

  insns.extend(fun(ping, Pubness::No, vec![call(pong)]));
  insns.extend(fun(pong, Pubness::No, vec![call(ping)]));
  insns.extend(fun(main, Pubness::No, vec![call(ping)]));

  let mut sir = make_sir(insns);

  Dce::new(&mut sir, vec![main], &interner).eliminate();

  assert_eq!(fun_names(&sir), vec![ping, pong, main]);
}

#[test]
fn only_main_no_other_functions() {
  let mut interner = Interner::new();
  let main = interner.intern("main");

  let mut insns = vec![];

  insns.extend(fun(main, Pubness::No, vec![]));

  let mut sir = make_sir(insns);

  Dce::new(&mut sir, vec![main], &interner).eliminate();

  assert_eq!(fun_names(&sir), vec![main]);
}

// ===== DEAD INSTRUCTION ELIMINATION =====

#[test]
fn dead_insn_unused_const() {
  let mut interner = Interner::new();
  let main = interner.intern("main");
  let x = interner.intern("x");

  let mut sir = make_sir(vec![
    Insn::FunDef {
      name: main,
      params: vec![],
      return_ty: TyId(1),
      body_start: 1,
      kind: zo_value::FunctionKind::UserDefined,
      pubness: Pubness::No,
      self_kind: SelfKind::None,
      link_name: None,
      owning_pack: None,
      span: Span::ZERO,
      is_test: false,
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

  Dce::new(&mut sir, vec![main], &interner).eliminate();

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
  let mut interner = Interner::new();
  let main = interner.intern("main");

  let mut sir = make_sir(vec![
    Insn::FunDef {
      name: main,
      params: vec![],
      return_ty: TyId(1),
      body_start: 1,
      kind: zo_value::FunctionKind::UserDefined,
      pubness: Pubness::No,
      self_kind: SelfKind::None,
      link_name: None,
      owning_pack: None,
      span: Span::ZERO,
      is_test: false,
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

  Dce::new(&mut sir, vec![main], &interner).eliminate();

  // Only FunDef + Return should survive.
  assert_eq!(sir.instructions.len(), 2);
  assert!(matches!(&sir.instructions[0], Insn::FunDef { .. }));
  assert!(matches!(&sir.instructions[1], Insn::Return { .. }));
}

#[test]
fn dead_insn_preserves_calls() {
  let mut interner = Interner::new();
  let main = interner.intern("main");
  let showln = interner.intern("showln");

  let mut sir = make_sir(vec![
    Insn::FunDef {
      name: main,
      params: vec![],
      return_ty: TyId(1),
      body_start: 1,
      kind: zo_value::FunctionKind::UserDefined,
      pubness: Pubness::No,
      self_kind: SelfKind::None,
      link_name: None,
      owning_pack: None,
      span: Span::ZERO,
      is_test: false,
    },
    // Call is impure — must not be removed even if dst unused.
    Insn::Call {
      dst: ValueId(0),
      name: showln,
      callee_pack: None,
      args: vec![],
      ty_id: TyId(1),
    },
    Insn::Return {
      value: None,
      ty_id: TyId(1),
    },
  ]);

  let before = sir.instructions.len();

  Dce::new(&mut sir, vec![main], &interner).eliminate();

  // Nothing removed — Call is impure.
  assert_eq!(sir.instructions.len(), before);
}

#[test]
fn dead_insn_preserves_array_store() {
  let mut interner = Interner::new();
  let main = interner.intern("main");

  let mut sir = make_sir(vec![
    Insn::FunDef {
      name: main,
      params: vec![],
      return_ty: TyId(1),
      body_start: 1,
      kind: zo_value::FunctionKind::UserDefined,
      pubness: Pubness::No,
      self_kind: SelfKind::None,
      link_name: None,
      owning_pack: None,
      span: Span::ZERO,
      is_test: false,
    },
    // ArrayStore is impure — must not be removed.
    Insn::ArrayStore {
      array: ValueId(0),
      index: ValueId(1),
      value: ValueId(2),
      ty_id: TyId(8),
      owner: None,
    },
    Insn::Return {
      value: None,
      ty_id: TyId(1),
    },
  ]);

  let before = sir.instructions.len();

  Dce::new(&mut sir, vec![main], &interner).eliminate();

  assert_eq!(sir.instructions.len(), before);
}

// ===== DEAD VARIABLE (STORE) ELIMINATION =====
// Disabled: insn_var_use needs complete var-use extraction.

#[test]
#[ignore = "dead variable pass disabled — incomplete var-use tracking"]
fn dead_var_unused_store() {
  let mut interner = Interner::new();
  let main = interner.intern("main");
  let x = interner.intern("x");
  let y = interner.intern("y");

  let mut sir = make_sir(vec![
    Insn::FunDef {
      name: main,
      params: vec![],
      return_ty: TyId(1),
      body_start: 1,
      kind: zo_value::FunctionKind::UserDefined,
      pubness: Pubness::No,
      self_kind: SelfKind::None,
      link_name: None,
      owning_pack: None,
      span: Span::ZERO,
      is_test: false,
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

  Dce::new(&mut sir, vec![main], &interner).eliminate();

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
  let mut interner = Interner::new();
  let main = interner.intern("main");
  let x = interner.intern("x");

  let mut sir = make_sir(vec![
    Insn::FunDef {
      name: main,
      params: vec![],
      return_ty: TyId(1),
      body_start: 1,
      kind: zo_value::FunctionKind::UserDefined,
      pubness: Pubness::No,
      self_kind: SelfKind::None,
      link_name: None,
      owning_pack: None,
      span: Span::ZERO,
      is_test: false,
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

  Dce::new(&mut sir, vec![main], &interner).eliminate();

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
  let mut interner = Interner::new();
  let main = interner.intern("main");
  let x = interner.intern("x");

  let mut sir = make_sir(vec![
    Insn::FunDef {
      name: main,
      params: vec![],
      return_ty: TyId(1),
      body_start: 1,
      kind: zo_value::FunctionKind::UserDefined,
      pubness: Pubness::No,
      self_kind: SelfKind::None,
      link_name: None,
      owning_pack: None,
      span: Span::ZERO,
      is_test: false,
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

  Dce::new(&mut sir, vec![main], &interner).eliminate();

  // Only FunDef + Return should remain.
  assert_eq!(sir.instructions.len(), 2);
  assert!(matches!(&sir.instructions[0], Insn::FunDef { .. }));
  assert!(matches!(&sir.instructions[1], Insn::Return { .. }));
}

#[test]
fn unreachable_stops_at_label() {
  let mut interner = Interner::new();
  let main = interner.intern("main");

  let mut sir = make_sir(vec![
    Insn::FunDef {
      name: main,
      params: vec![],
      return_ty: TyId(1),
      body_start: 1,
      kind: zo_value::FunctionKind::UserDefined,
      pubness: Pubness::No,
      self_kind: SelfKind::None,
      link_name: None,
      owning_pack: None,
      span: Span::ZERO,
      is_test: false,
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

  Dce::new(&mut sir, vec![main], &interner).eliminate();

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
  let interner = Interner::new();
  let mut sir = make_sir(vec![]);

  Dce::new(&mut sir, vec![Symbol(0)], &interner).eliminate();

  assert!(sir.instructions.is_empty());
}

// ===== TEMPLATE COMPUTED-BINDING ROOTS =====

#[test]
fn template_computed_binding_pins_closure() {
  // A closure referenced ONLY through `bindings.computed`
  // (not via `UiCommand::Event`) must survive DCE — it's
  // invoked by the runtime on each state change.
  // Without this pin, every `{when …}` interp loses its
  // closure and the runtime sees an empty Text forever.
  use zo_sir::{ComputedBinding, TemplateBindings};
  use zo_ui_protocol::UiCommand;
  use zo_value::FunctionKind;

  let mut interner = Interner::new();
  let main = interner.intern("main");
  let interp = interner.intern("__interp_0");

  let bindings = TemplateBindings {
    text: Vec::new(),
    attrs: Vec::new(),
    computed: vec![(
      0,
      ComputedBinding {
        closure_name: interp,
        captures: Vec::new(),
      },
    )],
    list: Vec::new(),
  };

  let mut insns = vec![Insn::FunDef {
    name: interp,
    params: Vec::new(),
    return_ty: TyId(4),
    body_start: 0,
    kind: FunctionKind::Closure { capture_count: 0 },
    pubness: Pubness::No,
    self_kind: SelfKind::None,
    link_name: None,
    owning_pack: None,
    span: Span::ZERO,
    is_test: false,
  }];

  insns.push(Insn::Return {
    value: None,
    ty_id: TyId(4),
  });

  insns.extend(fun(
    main,
    Pubness::No,
    vec![Insn::Template {
      id: ValueId(0),
      name: None,
      ty_id: TyId(0),
      commands: vec![UiCommand::Text(String::new())],
      bindings,
    }],
  ));

  let mut sir = make_sir(insns);

  Dce::new(&mut sir, vec![main], &interner).eliminate();

  let names = fun_names(&sir);

  assert!(
    names.contains(&interp),
    "computed-binding closure must survive DCE; survivors: {:?}",
    names
      .iter()
      .map(|s| interner.get(*s).to_string())
      .collect::<Vec<_>>()
  );
  assert!(names.contains(&main));
}

// ===== OPTIMISTIC MARK-SWEEP (deep dead chains) =====

fn const_int(dst: u32, value: u64) -> Insn {
  Insn::ConstInt {
    dst: ValueId(dst),
    value,
    ty_id: TyId(9),
  }
}

fn add(dst: u32, lhs: u32, rhs: u32) -> Insn {
  Insn::BinOp {
    dst: ValueId(dst),
    op: BinOp::Add,
    lhs: ValueId(lhs),
    rhs: ValueId(rhs),
    ty_id: TyId(9),
  }
}

fn count_value_insns(sir: &zo_sir::Sir) -> usize {
  sir
    .instructions
    .iter()
    .filter(|i| matches!(i, Insn::ConstInt { .. } | Insn::BinOp { .. }))
    .count()
}

/// `a = b + 1; b = c + 1; c = 42;` with `a` never observed.
/// The optimistic pass collapses the whole chain in ONE sweep,
/// where the prior fixed-point loop needed one pass per link.
#[test]
fn dead_dependency_chain_collapses() {
  let mut interner = Interner::new();
  let main = interner.intern("main");

  // c=V1, one=V2, b=V3, two=V4, a=V5 — none reach the Return.
  let body = vec![
    const_int(1, 42),
    const_int(2, 1),
    add(3, 1, 2),
    const_int(4, 1),
    add(5, 3, 4),
  ];

  let mut sir = make_sir(fun(main, Pubness::No, body));

  Dce::new(&mut sir, vec![main], &interner).eliminate();

  assert_eq!(
    count_value_insns(&sir),
    0,
    "every link of a dead chain must be eliminated"
  );
}

/// `CStr::new`-shaped body: a `StructConstruct` whose result
/// is returned must survive — regression for a mark-sweep bug
/// that dropped it, leaving `ret` on an undefined value.
#[test]
fn struct_construct_feeding_return_survives() {
  let mut interner = Interner::new();
  let main = interner.intern("wrap");
  let cstr = interner.intern("CStr");

  let insns = vec![
    Insn::FunDef {
      name: main,
      params: vec![(interner.intern("s"), TyId(4))],
      return_ty: TyId(21),
      body_start: 0,
      kind: zo_value::FunctionKind::UserDefined,
      pubness: Pubness::No,
      self_kind: SelfKind::None,
      link_name: None,
      owning_pack: None,
      span: Span::ZERO,
      is_test: false,
    },
    Insn::Load {
      dst: ValueId(1),
      src: zo_sir::LoadSource::Param(0),
      ty_id: TyId(4),
    },
    Insn::StructConstruct {
      dst: ValueId(2),
      struct_name: cstr,
      fields: vec![ValueId(1)],
      ty_id: TyId(21),
    },
    Insn::Return {
      value: Some(ValueId(2)),
      ty_id: TyId(21),
    },
  ];

  let mut sir = make_sir(insns);

  Dce::new(&mut sir, vec![main], &interner).eliminate();

  assert!(
    sir
      .instructions
      .iter()
      .any(|i| matches!(i, Insn::StructConstruct { .. })),
    "a StructConstruct feeding the return must survive DCE"
  );
}

/// Same chain, but `a` flows into the `Return`. Liveness
/// propagates backward through every operand, so all five
/// survive.
#[test]
fn live_dependency_chain_survives() {
  let mut interner = Interner::new();
  let main = interner.intern("main");

  let mut insns = vec![Insn::FunDef {
    name: main,
    params: vec![],
    return_ty: TyId(9),
    body_start: 0,
    kind: zo_value::FunctionKind::UserDefined,
    pubness: Pubness::No,
    self_kind: SelfKind::None,
    link_name: None,
    owning_pack: None,
    span: Span::ZERO,
    is_test: false,
  }];

  insns.push(const_int(1, 42));
  insns.push(const_int(2, 1));
  insns.push(add(3, 1, 2));
  insns.push(const_int(4, 1));
  insns.push(add(5, 3, 4));
  insns.push(Insn::Return {
    value: Some(ValueId(5)),
    ty_id: TyId(9),
  });

  let mut sir = make_sir(insns);

  Dce::new(&mut sir, vec![main], &interner).eliminate();

  assert_eq!(
    count_value_insns(&sir),
    5,
    "a value chain feeding the return must survive intact"
  );
}
