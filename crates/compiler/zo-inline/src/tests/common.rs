use zo_interner::{Interner, Symbol};
use zo_sir::{BinOp, Insn, LoadSource, Sir};
use zo_span::Span;
use zo_ty::{SelfKind, TyId};
use zo_value::{FunctionKind, Pubness, ValueId};

/// A `FunDef` introducer with `param_count` parameters.
pub fn fundef(name: Symbol, param_count: usize) -> Insn {
  Insn::FunDef {
    name,
    params: (0..param_count).map(|_| (name, TyId(1))).collect(),
    return_ty: TyId(1),
    body_start: 0,
    kind: FunctionKind::UserDefined,
    pubness: Pubness::No,
    self_kind: SelfKind::None,
    link_name: None,
    owning_pack: None,
    span: Span::ZERO,
    is_test: false,
  }
}

/// Builds a test SIR from `instructions`.
pub fn make_sir(instructions: Vec<Insn>) -> Sir {
  let spans = vec![Span::ZERO; instructions.len()];

  Sir {
    instructions,
    spans,
    node_idxs: Vec::new(),
    next_value_id: 100,
    next_label_id: 0,
    node_cursor: 0,
    vec_elem_tys: std::collections::HashMap::new(),
    int_bases: std::collections::HashMap::new(),
  }
}

/// A `double(x) { x + x }` candidate and a `main` that calls it.
pub fn double_and_caller(interner: &mut Interner) -> (Sir, Symbol) {
  let double = interner.intern("double");
  let main = interner.intern("main");

  let insns = vec![
    fundef(double, 1),
    Insn::Load {
      dst: ValueId(1),
      src: LoadSource::Param(0),
      ty_id: TyId(1),
    },
    Insn::Load {
      dst: ValueId(2),
      src: LoadSource::Param(0),
      ty_id: TyId(1),
    },
    Insn::BinOp {
      dst: ValueId(3),
      op: BinOp::Add,
      lhs: ValueId(1),
      rhs: ValueId(2),
      ty_id: TyId(1),
    },
    Insn::Return {
      value: Some(ValueId(3)),
      ty_id: TyId(1),
    },
    fundef(main, 0),
    Insn::Call {
      dst: ValueId(11),
      name: double,
      callee_pack: None,
      args: vec![ValueId(10)],
      ty_id: TyId(1),
    },
    Insn::Return {
      value: None,
      ty_id: TyId(1),
    },
  ];

  (make_sir(insns), double)
}

/// A branchy `max(a, b)` if/else candidate and a `main` calling it.
pub fn max_and_caller(interner: &mut Interner) -> (Sir, Symbol) {
  let max = interner.intern("max");
  let main = interner.intern("main");

  let insns = vec![
    fundef(max, 2),
    Insn::Load {
      dst: ValueId(1),
      src: LoadSource::Param(0),
      ty_id: TyId(1),
    },
    Insn::Load {
      dst: ValueId(2),
      src: LoadSource::Param(1),
      ty_id: TyId(1),
    },
    Insn::BinOp {
      dst: ValueId(3),
      op: BinOp::Gt,
      lhs: ValueId(1),
      rhs: ValueId(2),
      ty_id: TyId(1),
    },
    Insn::BranchIfNot {
      cond: ValueId(3),
      target: 1,
    },
    Insn::Load {
      dst: ValueId(4),
      src: LoadSource::Param(0),
      ty_id: TyId(1),
    },
    Insn::Return {
      value: Some(ValueId(4)),
      ty_id: TyId(1),
    },
    Insn::Label { id: 1 },
    Insn::Load {
      dst: ValueId(5),
      src: LoadSource::Param(1),
      ty_id: TyId(1),
    },
    Insn::Return {
      value: Some(ValueId(5)),
      ty_id: TyId(1),
    },
    fundef(main, 0),
    Insn::Call {
      dst: ValueId(11),
      name: max,
      callee_pack: None,
      args: vec![ValueId(10), ValueId(20)],
      ty_id: TyId(1),
    },
    Insn::Return {
      value: None,
      ty_id: TyId(1),
    },
  ];

  (make_sir(insns), max)
}

/// A `make(i) { Point { a = i, b = i } }` candidate and a caller.
pub fn struct_ctor_and_caller(interner: &mut Interner) -> (Sir, Symbol) {
  let make = interner.intern("make");
  let point = interner.intern("Point");
  let main = interner.intern("main");

  let insns = vec![
    fundef(make, 1),
    Insn::Load {
      dst: ValueId(1),
      src: LoadSource::Param(0),
      ty_id: TyId(1),
    },
    Insn::StructConstruct {
      dst: ValueId(2),
      struct_name: point,
      fields: vec![ValueId(1), ValueId(1)],
      ty_id: TyId(1),
    },
    Insn::Return {
      value: Some(ValueId(2)),
      ty_id: TyId(1),
    },
    fundef(main, 0),
    Insn::Call {
      dst: ValueId(11),
      name: make,
      callee_pack: None,
      args: vec![ValueId(10)],
      ty_id: TyId(1),
    },
    Insn::Return {
      value: None,
      ty_id: TyId(1),
    },
  ];

  (make_sir(insns), make)
}

/// A `make(x) { Point { x } }` shorthand constructor and a caller.
/// The shorthand field loads its parameter by name
/// (`LoadSource::Local`), not by index — the case that dangled
/// before binding rebound named local loads.
pub fn struct_shorthand_ctor_and_caller(
  interner: &mut Interner,
) -> (Sir, Symbol) {
  let make = interner.intern("make");
  let x = interner.intern("x");
  let point = interner.intern("Point");
  let main = interner.intern("main");

  let mut def = fundef(make, 1);

  if let Insn::FunDef { params, .. } = &mut def {
    params[0].0 = x;
  }

  let insns = vec![
    def,
    Insn::Load {
      dst: ValueId(1),
      src: LoadSource::Local(x),
      ty_id: TyId(1),
    },
    Insn::StructConstruct {
      dst: ValueId(2),
      struct_name: point,
      fields: vec![ValueId(1)],
      ty_id: TyId(1),
    },
    Insn::Return {
      value: Some(ValueId(2)),
      ty_id: TyId(1),
    },
    fundef(main, 0),
    Insn::Call {
      dst: ValueId(11),
      name: make,
      callee_pack: None,
      args: vec![ValueId(10)],
      ty_id: TyId(1),
    },
    Insn::Return {
      value: None,
      ty_id: TyId(1),
    },
  ];

  (make_sir(insns), make)
}

/// Count `Call`s to `name`.
pub fn calls(sir: &Sir, name: Symbol) -> usize {
  sir
    .instructions
    .iter()
    .filter(|insn| matches!(insn, Insn::Call { name: n, .. } if *n == name))
    .count()
}
