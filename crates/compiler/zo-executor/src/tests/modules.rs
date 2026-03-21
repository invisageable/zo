use crate::tests::common::assert_sir_stream;

use zo_interner::Symbol;
use zo_sir::Insn;
use zo_ty::TyId;
use zo_value::{FunctionKind, Pubness};

#[test]
fn test_load_emits_module_load() {
  assert_sir_stream(
    "load std::math;",
    &[Insn::ModuleLoad {
      path: vec![Symbol(25), Symbol(26)],
      imported_symbols: vec![],
    }],
  );
}

#[test]
fn test_load_nested_path() {
  assert_sir_stream(
    "load std::num::ops;",
    &[Insn::ModuleLoad {
      path: vec![Symbol(25), Symbol(26), Symbol(27)],
      imported_symbols: vec![],
    }],
  );
}

#[test]
fn test_pack_emits_pack_decl() {
  assert_sir_stream(
    "pack io;",
    &[Insn::PackDecl {
      name: Symbol(25),
      pubness: Pubness::No,
    }],
  );
}

#[test]
fn test_load_before_function() {
  assert_sir_stream(
    r#"load foo::bar;
fun main() -> int { 42 }"#,
    &[
      Insn::ModuleLoad {
        path: vec![Symbol(25), Symbol(26)],
        imported_symbols: vec![],
      },
      Insn::FunDef {
        name: Symbol(27),
        params: vec![],
        return_ty: TyId(8),
        body_start: 2,
        kind: FunctionKind::UserDefined,
        pubness: Pubness::No,
      },
      Insn::ConstInt {
        value: 42,
        ty_id: TyId(8),
      },
      Insn::Return {
        value: Some(zo_value::ValueId(0)),
        ty_id: TyId(8),
      },
    ],
  );
}

#[test]
fn test_multiple_packs() {
  assert_sir_stream(
    r#"pack io;
pack math;"#,
    &[
      Insn::PackDecl {
        name: Symbol(25),
        pubness: Pubness::No,
      },
      Insn::PackDecl {
        name: Symbol(26),
        pubness: Pubness::No,
      },
    ],
  );
}

#[test]
fn test_empty_body_is_intrinsic() {
  assert_sir_stream(
    "fun noop() {}",
    &[
      Insn::FunDef {
        name: Symbol(25),
        params: vec![],
        return_ty: TyId(1),
        body_start: 1,
        kind: FunctionKind::Intrinsic,
        pubness: Pubness::No,
      },
      Insn::Return {
        value: None,
        ty_id: TyId(1),
      },
    ],
  );
}

#[test]
fn test_non_empty_body_not_intrinsic() {
  assert_sir_stream(
    "fun answer() -> int { 42 }",
    &[
      Insn::FunDef {
        name: Symbol(25),
        params: vec![],
        return_ty: TyId(8),
        body_start: 1,
        kind: FunctionKind::UserDefined,
        pubness: Pubness::No,
      },
      Insn::ConstInt {
        value: 42,
        ty_id: TyId(8),
      },
      Insn::Return {
        value: Some(zo_value::ValueId(0)),
        ty_id: TyId(8),
      },
    ],
  );
}

#[test]
fn test_pub_fun_visibility() {
  assert_sir_stream(
    "pub fun greet() {}",
    &[
      Insn::FunDef {
        name: Symbol(25),
        params: vec![],
        return_ty: TyId(1),
        body_start: 1,
        kind: FunctionKind::Intrinsic,
        pubness: Pubness::Yes,
      },
      Insn::Return {
        value: None,
        ty_id: TyId(1),
      },
    ],
  );
}

#[test]
fn test_pub_pack_visibility() {
  assert_sir_stream(
    "pub pack math;",
    &[Insn::PackDecl {
      name: Symbol(25),
      pubness: Pubness::Yes,
    }],
  );
}
