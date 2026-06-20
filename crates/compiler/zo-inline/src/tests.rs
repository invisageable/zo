pub(crate) mod common;

use crate::{Inline, Release};

use common::{calls, double_and_caller, max_and_caller};

use zo_interner::Interner;
use zo_sir::{BinOp, Insn, LoadSource};
use zo_value::ValueId;

#[test]
fn release_inlines_pure_leaf_call() {
  let mut interner = Interner::new();
  let (mut sir, double) = double_and_caller(&mut interner);

  Inline::new(&mut sir, &mut interner, Release::Yes).inline();

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

  Inline::new(&mut sir, &mut interner, Release::No).inline();

  assert_eq!(calls(&sir, double), 1);
}

#[test]
fn release_inlines_branchy_call() {
  let mut interner = Interner::new();
  let (mut sir, max) = max_and_caller(&mut interner);

  Inline::new(&mut sir, &mut interner, Release::Yes).inline();

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
