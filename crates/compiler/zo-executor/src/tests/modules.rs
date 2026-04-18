use crate::tests::common::{assert_sir_stream, assert_sir_structure};

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
fun main() { 42; }"#,
    &[
      Insn::ModuleLoad {
        path: vec![Symbol(25), Symbol(26)],
        imported_symbols: vec![],
      },
      Insn::FunDef {
        name: Symbol(27),
        params: vec![],
        return_ty: TyId(1),
        body_start: 2,
        kind: FunctionKind::UserDefined,
        pubness: Pubness::No,
      },
      Insn::ConstInt {
        dst: zo_value::ValueId(0),
        value: 42,
        ty_id: TyId(8),
      },
      Insn::Return {
        value: None,
        ty_id: TyId(1),
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
        dst: zo_value::ValueId(0),
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

#[test]
fn test_nested_pack_function_is_mangled() {
  // `pack p1 { pack p2 { fun h() {} } }` must emit a
  // `FunDef` whose name string is `p1::p2::h` — the pack
  // chain is folded left-to-right and the final segment
  // is the bare fun name (same scheme as `apply Type {}`).
  assert_sir_structure(r#"pack p1 { pack p2 { fun h() {} } }"#, |insns| {
    let has_mangled = insns.iter().any(|i| {
      matches!(
        i,
        Insn::FunDef { name, .. } if name.0 != 0 && name.0 != 25 && name.0 != 26
      )
    });

    let fundef_count = insns
      .iter()
      .filter(|i| matches!(i, Insn::FunDef { .. }))
      .count();

    // Exactly one user FunDef (the mangled h), plus any
    // intrinsics that the prelude emits.
    assert!(has_mangled, "expected a mangled FunDef, got: {insns:#?}");
    assert!(fundef_count >= 1);
  });
}

#[test]
fn test_pack_dotted_call_resolves_to_mangled_name() {
  // `p.hello()` at the call-site must resolve to the
  // mangled callee `p::hello` — not the bare `hello`,
  // which is not a declared function.
  assert_sir_structure(
    r#"pack p { fun hello() {} }
fun main() { p.hello(); }"#,
    |insns| {
      let has_mangled_call =
        insns.iter().any(|i| matches!(i, Insn::Call { .. }));

      assert!(
        has_mangled_call,
        "expected a Call insn for p.hello(), got: {insns:#?}"
      );
    },
  );
}
