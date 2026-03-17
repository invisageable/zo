use crate::tests::common::assert_sir_stream;

use zo_interner::Symbol;
use zo_sir::Insn;
use zo_ty::TyId;

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
      is_pub: false,
    }],
  );
}

#[test]
#[ignore = "implicit return without type annotation not yet implemented"]
fn test_load_before_function() {
  assert_sir_stream(
    r#"load foo::bar;
fun main() { 42 }"#,
    &[
      Insn::ModuleLoad {
        path: vec![Symbol(25), Symbol(26)],
        imported_symbols: vec![],
      },
      Insn::FunDef {
        name: Symbol(27),
        params: vec![],
        return_ty: zo_ty::TyId(0),
        body_start: 2,
        is_intrinsic: false,
      },
      Insn::ConstInt {
        value: 42,
        ty_id: zo_ty::TyId(1),
      },
      Insn::Return {
        value: Some(zo_value::ValueId(0)),
        ty_id: zo_ty::TyId(1),
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
        is_pub: false,
      },
      Insn::PackDecl {
        name: Symbol(26),
        is_pub: false,
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
        return_ty: TyId(0),
        body_start: 1,
        is_intrinsic: true,
      },
      Insn::Return {
        value: None,
        ty_id: TyId(0),
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
        return_ty: TyId(1),
        body_start: 1,
        is_intrinsic: false,
      },
      Insn::ConstInt {
        value: 42,
        ty_id: TyId(1),
      },
      Insn::Return {
        value: Some(zo_value::ValueId(0)),
        ty_id: TyId(1),
      },
    ],
  );
}
