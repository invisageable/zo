use crate::eliminate_dead_functions;

use zo_interner::{Interner, Symbol};
use zo_sir::{Insn, Sir};
use zo_ty::TyId;
use zo_value::ValueId;

fn make_sir(instructions: Vec<Insn>) -> Sir {
  let next_value_id = instructions.len() as u32;

  Sir {
    instructions,
    next_value_id,
    next_label_id: 0,
  }
}

#[test]
fn test_removes_uncalled_function() {
  let mut interner = Interner::new();
  let show = interner.intern("show");
  let showln = interner.intern("showln");
  let main = interner.intern("main");
  let hello = interner.intern("hello");

  let mut sir = make_sir(vec![
    // show — never called, should be removed.
    Insn::FunDef {
      name: show,
      params: vec![],
      return_ty: TyId(1),
      body_start: 1,
      is_intrinsic: true,
      is_pub: true,
    },
    Insn::Return {
      value: None,
      ty_id: TyId(1),
    },
    // showln — called by main, should be kept.
    Insn::FunDef {
      name: showln,
      params: vec![],
      return_ty: TyId(1),
      body_start: 3,
      is_intrinsic: true,
      is_pub: true,
    },
    Insn::Return {
      value: None,
      ty_id: TyId(1),
    },
    // main — entry point, always kept.
    Insn::FunDef {
      name: main,
      params: vec![],
      return_ty: TyId(1),
      body_start: 5,
      is_intrinsic: false,
      is_pub: false,
    },
    Insn::ConstString {
      symbol: hello,
      ty_id: TyId(4),
    },
    Insn::Call {
      name: showln,
      args: vec![ValueId(0)],
      ty_id: TyId(1),
    },
    Insn::Return {
      value: None,
      ty_id: TyId(1),
    },
  ]);

  eliminate_dead_functions(&mut sir);

  // show should be gone. showln + main should remain.
  let fun_names = sir
    .instructions
    .iter()
    .filter_map(|i| {
      if let Insn::FunDef { name, .. } = i {
        Some(*name)
      } else {
        None
      }
    })
    .collect::<Vec<_>>();

  assert_eq!(fun_names, vec![showln, main]);
}

#[test]
fn test_keeps_all_when_all_called() {
  let mut interner = Interner::new();
  let foo = interner.intern("foo");
  let bar = interner.intern("bar");
  let main = interner.intern("main");

  let mut sir = make_sir(vec![
    Insn::FunDef {
      name: foo,
      params: vec![],
      return_ty: TyId(1),
      body_start: 1,
      is_intrinsic: true,
      is_pub: true,
    },
    Insn::Return {
      value: None,
      ty_id: TyId(1),
    },
    Insn::FunDef {
      name: bar,
      params: vec![],
      return_ty: TyId(1),
      body_start: 3,
      is_intrinsic: true,
      is_pub: true,
    },
    Insn::Return {
      value: None,
      ty_id: TyId(1),
    },
    Insn::FunDef {
      name: main,
      params: vec![],
      return_ty: TyId(1),
      body_start: 5,
      is_intrinsic: false,
      is_pub: false,
    },
    Insn::Call {
      name: foo,
      args: vec![],
      ty_id: TyId(1),
    },
    Insn::Call {
      name: bar,
      args: vec![],
      ty_id: TyId(1),
    },
    Insn::Return {
      value: None,
      ty_id: TyId(1),
    },
  ]);

  eliminate_dead_functions(&mut sir);

  let fun_names: Vec<Symbol> = sir
    .instructions
    .iter()
    .filter_map(|i| {
      if let Insn::FunDef { name, .. } = i {
        Some(*name)
      } else {
        None
      }
    })
    .collect();

  assert_eq!(fun_names, vec![foo, bar, main]);
}

#[test]
fn test_empty_sir_is_noop() {
  let mut sir = make_sir(vec![]);

  eliminate_dead_functions(&mut sir);

  assert!(sir.instructions.is_empty());
}

#[test]
fn test_no_functions_is_noop() {
  let mut sir = make_sir(vec![Insn::ModuleLoad {
    path: vec![],
    imported_symbols: vec![],
  }]);

  eliminate_dead_functions(&mut sir);

  assert_eq!(sir.instructions.len(), 1);
}
