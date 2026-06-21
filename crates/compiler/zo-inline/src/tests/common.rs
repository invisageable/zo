use zo_interner::{Interner, Symbol};
use zo_sir::{BinOp, Insn, LoadSource, Sir};
use zo_span::Span;
use zo_ty::{Mutability, SelfKind, TyId};
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

/// A `wrap(i) { Opt::Some(i) }` enum constructor and a caller.
pub fn enum_ctor_and_caller(interner: &mut Interner) -> (Sir, Symbol) {
  let wrap = interner.intern("wrap");
  let opt = interner.intern("Opt");
  let main = interner.intern("main");

  let insns = vec![
    fundef(wrap, 1),
    Insn::Load {
      dst: ValueId(1),
      src: LoadSource::Param(0),
      ty_id: TyId(1),
    },
    Insn::EnumConstruct {
      dst: ValueId(2),
      enum_name: opt,
      variant: 0,
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
      name: wrap,
      callee_pack: None,
      args: vec![ValueId(10)],
      ty_id: TyId(1),
    },
    Insn::Return {
      value: None,
      ty_id: TyId(1),
    },
  ];

  (make_sir(insns), wrap)
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

/// A `is_pos(x) { x > 0 }` predicate whose comparison `BinOp`
/// carries the operand type (int = `TyId(2)`), not the `bool`
/// (`TyId(3)`) it returns — the case that must route through a
/// slot so the bool return type survives.
pub fn comparison_and_caller(interner: &mut Interner) -> (Sir, Symbol) {
  let is_pos = interner.intern("is_pos");
  let main = interner.intern("main");
  let int_ty = TyId(2);
  let bool_ty = TyId(3);

  let insns = vec![
    fundef(is_pos, 1),
    Insn::Load {
      dst: ValueId(1),
      src: LoadSource::Param(0),
      ty_id: int_ty,
    },
    Insn::BinOp {
      dst: ValueId(2),
      op: BinOp::Gt,
      lhs: ValueId(1),
      rhs: ValueId(1),
      ty_id: int_ty,
    },
    Insn::Return {
      value: Some(ValueId(2)),
      ty_id: bool_ty,
    },
    fundef(main, 0),
    Insn::Call {
      dst: ValueId(11),
      name: is_pos,
      callee_pack: None,
      args: vec![ValueId(10)],
      ty_id: bool_ty,
    },
    Insn::Return {
      value: None,
      ty_id: TyId(1),
    },
  ];

  (make_sir(insns), is_pos)
}

/// A forwarder `fwd(x) { x }` called nested as `fwd(fwd(%10))`,
/// so the inner inline's substitution must chain through the
/// outer's to reach the original arg.
pub fn nested_and_caller(interner: &mut Interner) -> (Sir, Symbol) {
  let fwd = interner.intern("fwd");
  let main = interner.intern("main");

  let insns = vec![
    fundef(fwd, 1),
    Insn::Load {
      dst: ValueId(1),
      src: LoadSource::Param(0),
      ty_id: TyId(1),
    },
    Insn::Return {
      value: Some(ValueId(1)),
      ty_id: TyId(1),
    },
    fundef(main, 0),
    Insn::Call {
      dst: ValueId(11),
      name: fwd,
      callee_pack: None,
      args: vec![ValueId(10)],
      ty_id: TyId(1),
    },
    Insn::Call {
      dst: ValueId(12),
      name: fwd,
      callee_pack: None,
      args: vec![ValueId(11)],
      ty_id: TyId(1),
    },
    Insn::Return {
      value: Some(ValueId(12)),
      ty_id: TyId(1),
    },
  ];

  (make_sir(insns), fwd)
}

/// A `f(x) { imu d = x; d }` forwarder that routes through a body
/// local `d`, plus a caller. The local must be renamed to a
/// call-site-unique name on splice so it can't clash with a caller
/// local of the same name.
pub fn body_local_and_caller(interner: &mut Interner) -> (Sir, Symbol) {
  let f = interner.intern("f");
  let d = interner.intern("d");
  let main = interner.intern("main");

  let insns = vec![
    fundef(f, 1),
    Insn::Load {
      dst: ValueId(1),
      src: LoadSource::Param(0),
      ty_id: TyId(1),
    },
    Insn::VarDef {
      name: d,
      ty_id: TyId(1),
      init: Some(ValueId(1)),
      mutability: Mutability::No,
      pubness: Pubness::No,
    },
    Insn::Store {
      name: d,
      value: ValueId(1),
      ty_id: TyId(1),
    },
    Insn::Load {
      dst: ValueId(2),
      src: LoadSource::Local(d),
      ty_id: TyId(1),
    },
    Insn::Return {
      value: Some(ValueId(2)),
      ty_id: TyId(1),
    },
    fundef(main, 0),
    Insn::Call {
      dst: ValueId(11),
      name: f,
      callee_pack: None,
      args: vec![ValueId(10)],
      ty_id: TyId(1),
    },
    Insn::Return {
      value: None,
      ty_id: TyId(1),
    },
  ];

  (make_sir(insns), f)
}

/// Count `Call`s to `name`.
pub fn calls(sir: &Sir, name: Symbol) -> usize {
  sir
    .instructions
    .iter()
    .filter(|insn| matches!(insn, Insn::Call { name: n, .. } if *n == name))
    .count()
}
