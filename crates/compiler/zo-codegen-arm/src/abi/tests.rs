//! AAPCS classifier test matrix. Constructs minimal
//! `TyTable` + `Vec<Ty>` views by hand so each test is
//! independent of the rest of the compiler.

use super::*;

use zo_interner::{Interner, Symbol};

// --- Test scaffolding ---------------------------------------

/// Minimal type-table fixture. Bypasses `TyChecker` so a
/// classifier test doesn't need the analyzer crate.
struct Q {
  tys: Vec<Ty>,
  ty_table: TyTable,
  interner: Interner,
}

impl Q {
  fn new() -> Self {
    Self {
      tys: Vec::new(),
      ty_table: TyTable::new(),
      interner: Interner::new(),
    }
  }

  /// Intern a `Ty`, returning its `TyId` (== position
  /// in the `tys` vec). No deduplication — tests don't
  /// need it.
  fn ty(&mut self, ty: Ty) -> TyId {
    let id = TyId(self.tys.len() as u32);
    self.tys.push(ty);
    id
  }

  fn unit(&mut self) -> TyId {
    self.ty(Ty::Unit)
  }

  fn int64(&mut self) -> TyId {
    self.ty(Ty::Int {
      signed: true,
      width: IntWidth::S64,
    })
  }

  fn f32(&mut self) -> TyId {
    self.ty(Ty::Float(FloatWidth::F32))
  }

  fn f64(&mut self) -> TyId {
    self.ty(Ty::Float(FloatWidth::F64))
  }

  fn bool(&mut self) -> TyId {
    self.ty(Ty::Bool)
  }

  /// Intern a struct with the given (name, ty_id) field
  /// list, then a `Ty::Struct(...)` pointing at it.
  fn struct_(&mut self, name: &str, fields: &[TyId]) -> TyId {
    let name_sym = self.interner.intern(name);
    let field_specs: Vec<(Symbol, TyId, bool)> = fields
      .iter()
      .enumerate()
      .map(|(i, &ty_id)| {
        let fname = self.interner.intern(&format!("f{i}"));
        (fname, ty_id, false)
      })
      .collect();
    let sid = self.ty_table.intern_struct(name_sym, &field_specs);
    self.ty(Ty::Struct(sid))
  }

  fn query(&self) -> TypeQuery<'_> {
    TypeQuery {
      tys: &self.tys,
      ty_table: &self.ty_table,
    }
  }
}

// --- Scalar ints --------------------------------------------

#[test]
fn one_int_lands_in_x0() {
  let mut q = Q::new();
  let int = q.int64();
  let unit = q.unit();
  let abi = classify(&[int], unit, &q.query());

  assert_eq!(abi.args, vec![AbiArg::Gp(X0)]);
  assert_eq!(abi.ret, AbiRet::Void);
  assert_eq!(abi.stack_bytes, 0);
}

#[test]
fn eight_ints_fill_x0_to_x7() {
  let mut q = Q::new();
  let int = q.int64();
  let unit = q.unit();
  let abi =
    classify(&[int, int, int, int, int, int, int, int], unit, &q.query());

  assert_eq!(
    abi.args,
    vec![
      AbiArg::Gp(X0),
      AbiArg::Gp(X1),
      AbiArg::Gp(X2),
      AbiArg::Gp(X3),
      AbiArg::Gp(X4),
      AbiArg::Gp(X5),
      AbiArg::Gp(X6),
      AbiArg::Gp(X7),
    ]
  );
  assert_eq!(abi.stack_bytes, 0);
}

#[test]
fn nine_ints_overflow_to_stack() {
  let mut q = Q::new();
  let int = q.int64();
  let unit = q.unit();
  let abi = classify(
    &[int, int, int, int, int, int, int, int, int],
    unit,
    &q.query(),
  );

  assert_eq!(abi.args.len(), 9);
  assert_eq!(
    abi.args[8],
    AbiArg::Stack {
      stack_offset: 0,
      size: 8
    }
  );
  // Single 8-byte spill, rounded up to 16-byte alignment.
  assert_eq!(abi.stack_bytes, 16);
}

#[test]
fn bool_takes_a_gp_reg() {
  let mut q = Q::new();
  let b = q.bool();
  let unit = q.unit();
  let abi = classify(&[b], unit, &q.query());

  assert_eq!(abi.args, vec![AbiArg::Gp(X0)]);
}

// --- Scalar floats ------------------------------------------

// The four float-direction tests below reflect the
// raylib-mode classifier: `f32` arg/return narrows or
// widens (because zo's `float` runtime carries f64 and
// the C function expects f32 bits), `f64` passes through
// unchanged. When the proper float-coercion plan
// (`PLAN_FLOAT_COERCION.md`) lands the semantics flip
// back — the tests + classifier flip together.
#[test]
fn one_f64_lands_in_d0_no_narrow() {
  let mut q = Q::new();
  let f = q.f64();
  let unit = q.unit();
  let abi = classify(&[f], unit, &q.query());

  assert_eq!(
    abi.args,
    vec![AbiArg::Fp {
      reg: D0,
      narrow: false
    }]
  );
}

#[test]
fn one_f32_lands_in_d0_with_narrow() {
  let mut q = Q::new();
  let f = q.f32();
  let unit = q.unit();
  let abi = classify(&[f], unit, &q.query());

  assert_eq!(
    abi.args,
    vec![AbiArg::Fp {
      reg: D0,
      narrow: true
    }]
  );
}

#[test]
fn eight_f64s_fill_d0_to_d7() {
  let mut q = Q::new();
  let f = q.f64();
  let unit = q.unit();
  let abi = classify(&[f, f, f, f, f, f, f, f], unit, &q.query());

  let regs: Vec<FpRegister> = abi
    .args
    .iter()
    .map(|a| match a {
      AbiArg::Fp { reg, .. } => *reg,
      _ => panic!("expected Fp"),
    })
    .collect();
  assert_eq!(regs, vec![D0, D1, D2, D3, D4, D5, D6, D7]);
}

#[test]
fn ninth_float_spills_to_stack() {
  let mut q = Q::new();
  let f = q.f64();
  let unit = q.unit();
  let abi = classify(&[f, f, f, f, f, f, f, f, f], unit, &q.query());

  assert_eq!(
    abi.args[8],
    AbiArg::Stack {
      stack_offset: 0,
      size: 8
    }
  );
}

// --- HFA structs --------------------------------------------

#[test]
fn vector2_hfa_lands_in_s0_s1() {
  let mut q = Q::new();
  let f = q.f32();
  let v2 = q.struct_("Vector2", &[f, f]);
  let unit = q.unit();
  let abi = classify(&[v2], unit, &q.query());

  assert_eq!(
    abi.args,
    vec![AbiArg::Hfa {
      regs: vec![D0, D1],
      width: FloatWidth::F32,
    }]
  );
}

#[test]
fn vector3_hfa_lands_in_s0_s1_s2() {
  let mut q = Q::new();
  let f = q.f32();
  let v3 = q.struct_("Vector3", &[f, f, f]);
  let unit = q.unit();
  let abi = classify(&[v3], unit, &q.query());

  assert_eq!(
    abi.args,
    vec![AbiArg::Hfa {
      regs: vec![D0, D1, D2],
      width: FloatWidth::F32,
    }]
  );
}

#[test]
fn vector4_hfa_lands_in_s0_to_s3() {
  let mut q = Q::new();
  let f = q.f32();
  let v4 = q.struct_("Vector4", &[f, f, f, f]);
  let unit = q.unit();
  let abi = classify(&[v4], unit, &q.query());

  assert_eq!(
    abi.args,
    vec![AbiArg::Hfa {
      regs: vec![D0, D1, D2, D3],
      width: FloatWidth::F32,
    }]
  );
}

#[test]
fn five_float_fields_disqualify_hfa() {
  let mut q = Q::new();
  let f = q.f32();
  // 5×4B = 20B → not HFA, > 16B → indirect.
  let big = q.struct_("Big5f", &[f, f, f, f, f]);
  let unit = q.unit();
  let abi = classify(&[big], unit, &q.query());

  match &abi.args[0] {
    AbiArg::Indirect { size, ptr_reg, .. } => {
      assert_eq!(*size, 20);
      assert_eq!(*ptr_reg, X0);
    }
    other => panic!("expected Indirect, got {other:?}"),
  }
}

#[test]
fn mixed_field_types_disqualify_hfa() {
  let mut q = Q::new();
  let f = q.f32();
  let i = q.int64();
  // f32 + i64 mixed → not HFA; 4 + 8 = 12B → composite.
  let mix = q.struct_("Mix", &[f, i]);
  let unit = q.unit();
  let abi = classify(&[mix], unit, &q.query());

  match &abi.args[0] {
    AbiArg::Composite { regs, size } => {
      assert_eq!(*size, 12);
      assert_eq!(regs.len(), 2);
    }
    other => panic!("expected Composite, got {other:?}"),
  }
}

#[test]
fn hfa_double_vs_float_widths_distinct() {
  let mut q = Q::new();
  let f64_ty = q.f64();
  let v2d = q.struct_("Vector2d", &[f64_ty, f64_ty]);
  let unit = q.unit();
  let abi = classify(&[v2d], unit, &q.query());

  assert_eq!(
    abi.args,
    vec![AbiArg::Hfa {
      regs: vec![D0, D1],
      width: FloatWidth::F64,
    }]
  );
}

// --- Composite ≤ 16B (not HFA) ------------------------------

#[test]
fn struct_4b_lands_in_one_gp() {
  let mut q = Q::new();
  let i = q.int64(); // placeholder for "any int field"
  // A single i64 = 8B — packed into one GP.
  let small = q.struct_("Small", &[i]);
  let unit = q.unit();
  let abi = classify(&[small], unit, &q.query());

  match &abi.args[0] {
    AbiArg::Composite { regs, size } => {
      assert_eq!(*size, 8);
      assert_eq!(regs, &vec![X0]);
    }
    other => panic!("expected Composite, got {other:?}"),
  }
}

#[test]
fn struct_16b_lands_in_two_gp() {
  let mut q = Q::new();
  let i = q.int64();
  let pair = q.struct_("Pair", &[i, i]);
  let unit = q.unit();
  let abi = classify(&[pair], unit, &q.query());

  match &abi.args[0] {
    AbiArg::Composite { regs, size } => {
      assert_eq!(*size, 16);
      assert_eq!(regs, &vec![X0, X1]);
    }
    other => panic!("expected Composite, got {other:?}"),
  }
}

// --- Composite > 16B (indirect) -----------------------------

#[test]
fn camera3d_44b_passes_indirect() {
  let mut q = Q::new();
  let f = q.f32();
  let i = q.int64();
  let v3 = q.struct_("Vector3", &[f, f, f]); // 12B
  // raylib's Camera3D ≈ position(12) + target(12) + up(12)
  // + fovy(4) + projection(4) = 44B.
  let cam = q.struct_("Camera3D", &[v3, v3, v3, f, i]);
  let unit = q.unit();
  let abi = classify(&[cam], unit, &q.query());

  match &abi.args[0] {
    AbiArg::Indirect {
      stack_offset,
      size,
      ptr_reg,
    } => {
      assert_eq!(*stack_offset, 0);
      assert!(*size >= 40, "got size {size}");
      assert_eq!(*ptr_reg, X0);
    }
    other => panic!("expected Indirect, got {other:?}"),
  }
  assert_eq!(abi.stack_bytes, 48); // round_up_16(44)
}

// --- Mixed signatures ---------------------------------------

#[test]
fn draw_circle_int_int_float_int() {
  // raylib's `DrawCircle(int, int, float, Color)` — but
  // simplified: ints + one float, all in scalar regs.
  // `f32` here mirrors raylib's wire-level `float` —
  // narrows from zo's runtime f64 representation.
  let mut q = Q::new();
  let i = q.int64();
  let f = q.f32();
  let unit = q.unit();
  let abi = classify(&[i, i, f, i], unit, &q.query());

  assert_eq!(
    abi.args,
    vec![
      AbiArg::Gp(X0),
      AbiArg::Gp(X1),
      AbiArg::Fp {
        reg: D0,
        narrow: true
      },
      AbiArg::Gp(X2),
    ]
  );
}

#[test]
fn draw_circle_v_vector2_float_int() {
  // raylib's `DrawCircleV(Vector2, float, Color)`. The
  // HFA Vector2 occupies S0+S1, then float in S2, then
  // color in X0 — independent register banks. `f32`
  // (not `f64`) mirrors raylib's wire `float` so the
  // classifier emits `narrow: true`.
  let mut q = Q::new();
  let f32_ty = q.f32();
  let i = q.int64();
  let v2 = q.struct_("Vector2", &[f32_ty, f32_ty]);
  let unit = q.unit();
  let abi = classify(&[v2, f32_ty, i], unit, &q.query());

  assert_eq!(
    abi.args,
    vec![
      AbiArg::Hfa {
        regs: vec![D0, D1],
        width: FloatWidth::F32,
      },
      AbiArg::Fp {
        reg: D2,
        narrow: true
      },
      AbiArg::Gp(X0),
    ]
  );
}

#[test]
fn spawn_box_mesh_two_ints_three_floats() {
  // misato's `zo_misato_spawn_box_mesh(geom, mat, x, y, z)`.
  // The wire signature takes `f32` for x/y/z so the
  // classifier emits `narrow: true` for each.
  let mut q = Q::new();
  let i = q.int64();
  let f = q.f32();
  let unit = q.unit();
  let abi = classify(&[i, i, f, f, f], unit, &q.query());

  assert_eq!(
    abi.args,
    vec![
      AbiArg::Gp(X0),
      AbiArg::Gp(X1),
      AbiArg::Fp {
        reg: D0,
        narrow: true
      },
      AbiArg::Fp {
        reg: D1,
        narrow: true
      },
      AbiArg::Fp {
        reg: D2,
        narrow: true
      },
    ]
  );
}

// --- Returns ------------------------------------------------

#[test]
fn ret_int_lands_in_x0() {
  let mut q = Q::new();
  let i = q.int64();
  let abi = classify(&[], i, &q.query());

  assert!(matches!(abi.ret, AbiRet::Gp { reg: X0, .. }));
}

#[test]
fn ret_f64_lands_in_d0_no_widen() {
  let mut q = Q::new();
  let f = q.f64();
  let abi = classify(&[], f, &q.query());

  assert_eq!(
    abi.ret,
    AbiRet::Fp {
      reg: D0,
      widen: false
    }
  );
}

#[test]
fn ret_f32_lands_in_d0_with_widen() {
  let mut q = Q::new();
  let f = q.f32();
  let abi = classify(&[], f, &q.query());

  assert_eq!(
    abi.ret,
    AbiRet::Fp {
      reg: D0,
      widen: true
    }
  );
}

#[test]
fn ret_vector2_hfa_in_s0_s1() {
  let mut q = Q::new();
  let f = q.f32();
  let v2 = q.struct_("Vector2", &[f, f]);
  let abi = classify(&[], v2, &q.query());

  assert_eq!(
    abi.ret,
    AbiRet::Hfa {
      regs: vec![D0, D1],
      width: FloatWidth::F32,
    }
  );
}

#[test]
fn ret_unit_is_void() {
  let mut q = Q::new();
  let unit = q.unit();
  let abi = classify(&[], unit, &q.query());

  assert_eq!(abi.ret, AbiRet::Void);
}

#[test]
fn ret_large_struct_uses_indirect_slot() {
  let mut q = Q::new();
  let f = q.f32();
  let i = q.int64();
  let v3 = q.struct_("Vector3", &[f, f, f]);
  let cam = q.struct_("Camera3D", &[v3, v3, v3, f, i]);
  let abi = classify(&[], cam, &q.query());

  match abi.ret {
    AbiRet::Indirect { slot_offset, size } => {
      assert_eq!(slot_offset, 0);
      assert!(size >= 40);
    }
    other => panic!("expected Indirect, got {other:?}"),
  }
  assert!(abi.stack_bytes >= 48);
}

#[test]
fn ret_composite_16b_in_two_gp() {
  let mut q = Q::new();
  let i = q.int64();
  let pair = q.struct_("Pair", &[i, i]);
  let abi = classify(&[], pair, &q.query());

  match abi.ret {
    AbiRet::Composite { regs, size } => {
      assert_eq!(size, 16);
      assert_eq!(regs, vec![X0, X1]);
    }
    other => panic!("expected Composite, got {other:?}"),
  }
}

// --- Independence of GP / FP banks --------------------------

#[test]
fn int_and_float_args_use_separate_banks() {
  // Float args don't consume GP slots, and vice versa.
  // Verify by passing 8 ints + 4 floats — all in regs,
  // no spill.
  let mut q = Q::new();
  let i = q.int64();
  let f = q.f64();
  let unit = q.unit();
  let abi = classify(&[i, i, i, i, i, i, i, i, f, f, f, f], unit, &q.query());

  let any_stack = abi.args.iter().any(|a| matches!(a, AbiArg::Stack { .. }));
  assert!(!any_stack, "no arg should spill: {:?}", abi.args);
  assert_eq!(abi.stack_bytes, 0);
}

// --- Stack alignment ----------------------------------------

#[test]
fn stack_bytes_always_16_aligned() {
  // Force a single 8-byte spill — should round up to 16.
  let mut q = Q::new();
  let i = q.int64();
  let unit = q.unit();
  let abi = classify(&[i, i, i, i, i, i, i, i, i], unit, &q.query());

  assert_eq!(abi.stack_bytes % 16, 0);
}
