pub(crate) mod common;

use crate::{Inline, Release};

use common::{
  body_local_and_caller, calls, comparison_and_caller, double_and_caller,
  enum_ctor_and_caller, max_and_caller, nested_and_caller,
  struct_ctor_and_caller, struct_shorthand_ctor_and_caller,
};

use zo_interner::Interner;
use zo_sir::{BinOp, Insn, LoadSource};
use zo_value::ValueId;

#[test]
fn release_inlines_pure_leaf_call() {
  let mut interner = Interner::new();
  let (mut sir, double) = double_and_caller(&mut interner);

  Inline::new(&mut sir, &mut interner, &[], Release::Yes).inline();

  // The call to `double` is gone — replaced by its body.
  assert_eq!(calls(&sir, double), 0);

  // The param bound to the arg: the inlined `add` reads %10 twice.
  assert!(sir.instructions.iter().any(|insn| matches!(
    insn,
    Insn::BinOp { op: BinOp::Add, lhs, rhs, .. }
      if *lhs == ValueId(10) && *rhs == ValueId(10)
  )));
}

#[test]
fn debug_leaves_call_untouched() {
  let mut interner = Interner::new();
  let (mut sir, double) = double_and_caller(&mut interner);

  Inline::new(&mut sir, &mut interner, &[], Release::No).inline();

  assert_eq!(calls(&sir, double), 1);
}

#[test]
fn release_inlines_struct_constructor() {
  let mut interner = Interner::new();
  let (mut sir, make) = struct_ctor_and_caller(&mut interner);

  Inline::new(&mut sir, &mut interner, &[], Release::Yes).inline();

  // The call is gone — the struct is built inline, eliding the
  // caller-side deep copy. Each field binds to the arg %10.
  assert_eq!(calls(&sir, make), 0);
  assert!(sir.instructions.iter().any(|insn| matches!(
    insn,
    Insn::StructConstruct { fields, .. }
      if !fields.is_empty() && fields.iter().all(|f| *f == ValueId(10))
  )));
}

#[test]
fn release_binds_shorthand_struct_field_param() {
  let mut interner = Interner::new();
  let (mut sir, make) = struct_shorthand_ctor_and_caller(&mut interner);

  Inline::new(&mut sir, &mut interner, &[], Release::Yes).inline();

  // The shorthand field's named local load binds to the arg %10:
  // the inlined struct's field is the arg itself, not a dangling
  // local. (The un-inlined `make` definition keeps its own load.)
  assert_eq!(calls(&sir, make), 0);
  assert!(sir.instructions.iter().any(|insn| matches!(
    insn,
    Insn::StructConstruct { fields, .. }
      if fields == &vec![ValueId(10)]
  )));
}

#[test]
fn release_inlines_enum_constructor() {
  let mut interner = Interner::new();
  let (mut sir, wrap) = enum_ctor_and_caller(&mut interner);

  Inline::new(&mut sir, &mut interner, &[], Release::Yes).inline();

  // The call is gone — the enum is built inline, eliding the
  // return copy. The payload field binds to the arg %10.
  assert_eq!(calls(&sir, wrap), 0);
  assert!(sir.instructions.iter().any(|insn| matches!(
    insn,
    Insn::EnumConstruct { fields, variant: 0, .. }
      if fields == &vec![ValueId(10)]
  )));
}

#[test]
fn release_routes_type_mismatch_return_through_slot() {
  let mut interner = Interner::new();
  let (mut sir, is_pos) = comparison_and_caller(&mut interner);

  Inline::new(&mut sir, &mut interner, &[], Release::Yes).inline();

  // The comparison's operand type differs from the bool return, so
  // the call inlines through a slot (Store + Local load), not a
  // direct substitution that would drop the bool.
  assert_eq!(calls(&sir, is_pos), 0);
  assert!(
    sir
      .instructions
      .iter()
      .any(|insn| matches!(insn, Insn::Store { .. }))
  );
  assert!(sir.instructions.iter().any(|insn| matches!(
    insn,
    Insn::Load {
      src: LoadSource::Local(_),
      ..
    }
  )));
}

#[test]
fn release_chains_nested_inline_substitutions() {
  let mut interner = Interner::new();
  let (mut sir, fwd) = nested_and_caller(&mut interner);

  Inline::new(&mut sir, &mut interner, &[], Release::Yes).inline();

  // Both calls inlined; the nested result chains all the way to the
  // original arg %10, not a dangling intermediate %11.
  assert_eq!(calls(&sir, fwd), 0);
  assert!(sir.instructions.iter().any(|insn| matches!(
    insn,
    Insn::Return { value: Some(v), .. } if *v == ValueId(10)
  )));
}

#[test]
fn release_inlines_and_renames_body_local() {
  let mut interner = Interner::new();
  let (mut sir, f) = body_local_and_caller(&mut interner);

  Inline::new(&mut sir, &mut interner, &[], Release::Yes).inline();

  // The call is gone and the spliced body local `d` was renamed to
  // a call-site-unique name, so it can't clash with a caller local.
  // (The un-inlined `f` definition keeps its own `d`.)
  assert_eq!(calls(&sir, f), 0);
  assert!(sir.instructions.iter().any(|insn| matches!(
    insn,
    Insn::VarDef { name, .. } if interner.get(*name).starts_with("__inl")
  )));
}

#[test]
fn release_inlines_branchy_call() {
  let mut interner = Interner::new();
  let (mut sir, max) = max_and_caller(&mut interner);

  Inline::new(&mut sir, &mut interner, &[], Release::Yes).inline();

  // Inlined: no call, slot store + slot load in its place.
  assert_eq!(calls(&sir, max), 0);
  assert!(
    sir
      .instructions
      .iter()
      .any(|insn| matches!(insn, Insn::Store { .. }))
  );
  assert!(sir.instructions.iter().any(|insn| matches!(
    insn,
    Insn::Load {
      src: LoadSource::Local(_),
      ..
    }
  )));
}
