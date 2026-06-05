use crate::PrettyPrinter;

use zo_interner::Interner;
use zo_sir::{Insn, Sir};
use zo_ty::{FloatWidth, IntWidth, Ty, TyId};
use zo_value::ValueId;

/// The SIR dump prints each constant's real type, resolved from
/// its `ty_id`, never a hardcoded width. Regression for the
/// `f64` literal that read `f32` and masked a codegen bug for a
/// whole debugging session.
#[test]
fn const_types_resolve_from_ty_id_not_hardcoded() {
  let tys = [
    Ty::Float(FloatWidth::F64),
    Ty::Int {
      signed: true,
      width: IntWidth::S64,
    },
    Ty::Float(FloatWidth::F32),
    Ty::Int {
      signed: false,
      width: IntWidth::U8,
    },
  ];

  let mut sir = Sir::new();

  sir.emit(Insn::ConstFloat {
    dst: ValueId(0),
    value: 1.5,
    ty_id: TyId(0),
  });
  sir.emit(Insn::ConstInt {
    dst: ValueId(1),
    value: 42,
    ty_id: TyId(1),
  });
  sir.emit(Insn::ConstFloat {
    dst: ValueId(2),
    value: 0.25,
    ty_id: TyId(2),
  });
  sir.emit(Insn::ConstInt {
    dst: ValueId(3),
    value: 7,
    ty_id: TyId(3),
  });

  let interner = Interner::new();
  let mut pp = PrettyPrinter::new();

  pp.format_sir(&sir, &interner, &tys);

  let out = String::from_utf8(pp.finish()).unwrap();

  assert!(
    out.contains("1.5 : f64"),
    "f64 literal must read f64: {out}"
  );
  assert!(out.contains("42 : i64"), "i64 literal must read i64: {out}");
  assert!(
    out.contains("0.25 : f32"),
    "f32 literal must read f32: {out}"
  );
  assert!(out.contains("7 : u8"), "u8 literal must read u8: {out}");
  // The hardcoded-`f32`-for-every-float bug must stay dead.
  assert!(!out.contains("1.5 : f32"), "f64 must not read f32: {out}");
}
