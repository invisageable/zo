pub(crate) mod common;

use crate::{Inline, Release};

use common::{
  calls, double_and_caller, max_and_caller, struct_ctor_and_caller,
  struct_shorthand_ctor_and_caller,
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
