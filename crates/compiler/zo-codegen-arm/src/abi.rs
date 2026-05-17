//! AArch64 / AAPCS64 (Apple ARM64) call-site classifier.
//!
//! Drives FFI codegen from the `pub ffi` declaration's
//! type signature instead of per-symbol handlers. See
//! `PLAN_AAPCS_FFI_FROM_SIGNATURE.md` for the larger
//! design.
//!
//! Pure read-only function over the type system: takes a
//! [`TypeQuery`] view (slice of `Ty` + `&TyTable`) and
//! produces an [`AbiCall`] describing every register /
//! stack placement the call site must materialise.
//!
//! Scope today: classification only. The matching emit
//! step (F2) consumes the [`AbiCall`] and produces the
//! actual ARM64 instructions.

use zo_emitter_arm::{
  D0, D1, D2, D3, D4, D5, D6, D7, FpRegister, Register, X0, X1, X2, X3, X4, X5,
  X6, X7,
};
use zo_ty::{FloatWidth, IntWidth, StructTyId, Ty, TyId, TyTable};

// --- Type-system view ---------------------------------------

/// Pure read-only view of the type system the classifier
/// needs. Lets us test in isolation without spinning up a
/// full `TyChecker` — construct a `Vec<Ty>` + `TyTable` by
/// hand and pass them through.
pub struct TypeQuery<'a> {
  pub tys: &'a [Ty],
  pub ty_table: &'a TyTable,
}

impl<'a> TypeQuery<'a> {
  pub fn resolve(&self, id: TyId) -> Ty {
    self.tys.get(id.0 as usize).copied().unwrap_or(Ty::Error)
  }

  /// Byte size per Apple AArch64 ABI rules. `None` for
  /// types with no defined size at the FFI boundary
  /// (Unit, type variables, errors).
  pub fn size_of(&self, id: TyId) -> Option<u32> {
    match self.resolve(id) {
      Ty::Bool | Ty::Char => Some(1),
      Ty::Int { width, .. } => Some(int_width_bytes(width)),
      Ty::Float(w) => Some(float_width_bytes(w)),
      // `str` and `bytes` reach the FFI boundary as raw
      // pointers (`c_str(...)` strips zo's length prefix).
      // 8 bytes on 64-bit AArch64.
      Ty::Str | Ty::Bytes => Some(8),
      Ty::Struct(sid) => self.struct_size(sid),
      _ => None,
    }
  }

  /// Sum of field sizes. Assumes natural alignment is
  /// already satisfied — true for raylib-style POD
  /// structs (Vector*, Color, Camera3D, Mesh, Material).
  pub fn struct_size(&self, sid: StructTyId) -> Option<u32> {
    let st = self.ty_table.struct_ty(sid)?;
    let mut total = 0u32;

    for field in self.ty_table.struct_fields(st) {
      total += self.size_of(field.ty_id)?;
    }

    Some(total)
  }

  /// HFA classification: 1–4 fields, all of the same FP
  /// scalar type. Returns the (count, width) on success;
  /// `None` if the struct fails any condition.
  pub fn hfa_classify(&self, sid: StructTyId) -> Option<HfaInfo> {
    let st = self.ty_table.struct_ty(sid)?;
    let fields = self.ty_table.struct_fields(st);

    if fields.is_empty() || fields.len() > 4 {
      return None;
    }

    let first_width = match self.resolve(fields[0].ty_id) {
      Ty::Float(w) => w,
      _ => return None,
    };

    for field in &fields[1..] {
      match self.resolve(field.ty_id) {
        Ty::Float(w) if w == first_width => {}
        _ => return None,
      }
    }

    Some(HfaInfo {
      count: fields.len() as u8,
      width: first_width,
    })
  }
}

// --- Result types -------------------------------------------

/// Homogeneous Floating-point Aggregate. 1–4 fields, all
/// the same FP scalar type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HfaInfo {
  pub count: u8,
  pub width: FloatWidth,
}

/// Per-argument placement on the call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbiArg {
  /// Int / pointer in a GP register.
  Gp(Register),

  /// Single FP scalar. `narrow = true` means the C side
  /// of the FFI takes `float` (f32), so we narrow zo's
  /// f64 down to f32 via `FCVT S, D` before the call
  /// lands the value in the low 32 bits of the named V
  /// register. Declared-width-driven: `f32` in the FFI
  /// signature → narrow; `f64` / `float` (f64) → pass
  /// the full D-reg straight through.
  Fp { reg: FpRegister, narrow: bool },

  /// 1–4 FP fields spread across consecutive FP regs.
  Hfa {
    regs: Vec<FpRegister>,
    width: FloatWidth,
  },

  /// Composite ≤ 16B packed into 1–2 GP regs.
  Composite { regs: Vec<Register>, size: u32 },

  /// Composite > 16B. Caller copies `size` bytes onto the
  /// stack at `stack_offset`, then passes
  /// `SP + stack_offset` in `ptr_reg`.
  Indirect {
    stack_offset: u32,
    size: u32,
    ptr_reg: Register,
  },

  /// Argument that overflowed the register file — passed
  /// at `stack_offset` from SP.
  Stack { stack_offset: u32, size: u32 },
}

/// Return-value placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbiRet {
  Void,
  Gp(Register),

  /// `widen = true` means the C side of the FFI returned
  /// `float` (f32) in S0 and we widen it to zo's f64 via
  /// `FCVT D, S` before storing into the dst. Declared-
  /// width-driven: `-> f32` → widen; `-> f64` / `-> float`
  /// (f64) → the full D-reg already holds the result.
  Fp {
    reg: FpRegister,
    widen: bool,
  },

  Hfa {
    regs: Vec<FpRegister>,
    width: FloatWidth,
  },

  Composite {
    regs: Vec<Register>,
    size: u32,
  },

  /// Return > 16B: the caller allocates the slot, hands a
  /// pointer to it via X8 (the AArch64 "indirect result
  /// location" register). The classifier reserves
  /// `size` bytes at `slot_offset`.
  Indirect {
    slot_offset: u32,
    size: u32,
  },
}

/// Full AAPCS classification for one C call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiCall {
  pub args: Vec<AbiArg>,
  pub ret: AbiRet,
  /// Total bytes the caller must reserve on the stack
  /// for indirect args + indirect return slot. Always a
  /// multiple of 16 (AAPCS stack-alignment requirement).
  pub stack_bytes: u32,
}

// --- Classifier ---------------------------------------------

/// Classify the AAPCS call layout for a C function with
/// the given parameter types and return type.
pub fn classify(params: &[TyId], ret_ty: TyId, query: &TypeQuery) -> AbiCall {
  let mut state = ClassState::default();

  // Indirect return reserves its slot first — X8 then
  // holds `SP + slot_offset` and the slot must be live
  // for the entire call.
  let ret = classify_ret(ret_ty, query, &mut state);

  let args = params
    .iter()
    .map(|&p| classify_arg(p, query, &mut state))
    .collect();

  AbiCall {
    args,
    ret,
    stack_bytes: round_up_16(state.stack),
  }
}

#[derive(Default)]
struct ClassState {
  /// Next GP arg slot (0..=7 = X0..X7; 8+ = stack).
  next_gp: usize,
  /// Next FP arg slot (0..=7 = D0/S0..D7/S7; 8+ = stack).
  next_fp: usize,
  /// Bytes of stack used so far for indirect args + slot.
  stack: u32,
}

fn classify_arg(ty: TyId, query: &TypeQuery, state: &mut ClassState) -> AbiArg {
  match query.resolve(ty) {
    Ty::Int { .. } | Ty::Bool | Ty::Char | Ty::Str | Ty::Bytes => {
      classify_gp(state, query.size_of(ty).unwrap_or(8))
    }
    Ty::Float(w) => classify_fp_scalar(w, state),
    Ty::Struct(sid) => classify_struct(sid, query, state),
    _ => {
      // Fallback for types we don't expect at the FFI
      // boundary today (Unit, Tuple, Array, generics).
      // Treat as 8-byte GP — caller-side validation
      // should reject before we get here.
      classify_gp(state, 8)
    }
  }
}

fn classify_gp(state: &mut ClassState, _size: u32) -> AbiArg {
  if state.next_gp < 8 {
    let reg = gp_reg(state.next_gp);
    state.next_gp += 1;
    AbiArg::Gp(reg)
  } else {
    let stack_offset = state.stack;
    state.stack += 8;
    AbiArg::Stack {
      stack_offset,
      size: 8,
    }
  }
}

fn classify_fp_scalar(width: FloatWidth, state: &mut ClassState) -> AbiArg {
  if state.next_fp < 8 {
    let reg = fp_reg(state.next_fp);
    state.next_fp += 1;
    // Declared C-side width drives narrowing:
    //   `f32` in the FFI signature → narrow zo's f64 down
    //   to f32 (FCVT S, D) before the call;
    //   `f64` / `float` (f64) → pass straight through.
    // Bindings to C libraries must declare the actual C
    // parameter width (raylib uses `f32`; libm uses
    // `float` (f64)).
    let narrow = matches!(width, FloatWidth::F32);
    AbiArg::Fp { reg, narrow }
  } else {
    let stack_offset = state.stack;
    state.stack += 8;
    AbiArg::Stack {
      stack_offset,
      size: 8,
    }
  }
}

fn classify_struct(
  sid: StructTyId,
  query: &TypeQuery,
  state: &mut ClassState,
) -> AbiArg {
  // HFA — preferred when it fits in remaining FP regs.
  if let Some(hfa) = query.hfa_classify(sid)
    && state.next_fp + (hfa.count as usize) <= 8
  {
    let regs: Vec<FpRegister> = (0..hfa.count)
      .map(|i| fp_reg(state.next_fp + i as usize))
      .collect();
    state.next_fp += hfa.count as usize;
    return AbiArg::Hfa {
      regs,
      width: hfa.width,
    };
  }

  let size = query.struct_size(sid).unwrap_or(0);

  // ≤ 16 bytes → packed into 1–2 GP regs.
  if size <= 16 && state.next_gp + n_gp_for_size(size) <= 8 {
    let nregs = n_gp_for_size(size);
    let regs: Vec<Register> =
      (0..nregs).map(|i| gp_reg(state.next_gp + i)).collect();
    state.next_gp += nregs;
    return AbiArg::Composite { regs, size };
  }

  // > 16 bytes → caller copies onto stack, passes ptr in
  // the next GP. AAPCS says the indirect pointer counts
  // against the GP arg budget.
  let aligned_size = round_up_8(size);
  let stack_offset = state.stack;
  state.stack += aligned_size;

  let ptr_reg = if state.next_gp < 8 {
    let r = gp_reg(state.next_gp);
    state.next_gp += 1;
    r
  } else {
    // Pointer-to-arg also overflows. Real handling would
    // place the pointer on the stack — out of scope for
    // raylib/misato signatures, panic loudly so we catch
    // it if it ever happens.
    panic!("AAPCS: indirect-arg pointer overflowed GP regs");
  };

  AbiArg::Indirect {
    stack_offset,
    size,
    ptr_reg,
  }
}

fn classify_ret(
  ret_ty: TyId,
  query: &TypeQuery,
  state: &mut ClassState,
) -> AbiRet {
  match query.resolve(ret_ty) {
    Ty::Unit | Ty::Error => AbiRet::Void,
    Ty::Int { .. } | Ty::Bool | Ty::Char | Ty::Str | Ty::Bytes => {
      AbiRet::Gp(X0)
    }
    Ty::Float(w) => AbiRet::Fp {
      reg: D0,
      // f32 returns arrive in S0; widen to zo's internal
      // f64 here so the SIR-level `Cast f32 → f64` stays a
      // no-op (FP regs are uniformly 64-bit internally).
      widen: matches!(w, FloatWidth::F32),
    },
    Ty::Struct(sid) => classify_struct_ret(sid, query, state),
    _ => AbiRet::Gp(X0),
  }
}

fn classify_struct_ret(
  sid: StructTyId,
  query: &TypeQuery,
  state: &mut ClassState,
) -> AbiRet {
  // HFA return → S0..S3 / D0..D3.
  if let Some(hfa) = query.hfa_classify(sid) {
    let regs: Vec<FpRegister> =
      (0..hfa.count).map(|i| fp_reg(i as usize)).collect();
    return AbiRet::Hfa {
      regs,
      width: hfa.width,
    };
  }

  let size = query.struct_size(sid).unwrap_or(0);

  // ≤ 16 bytes → packed into X0 / X1.
  if size <= 16 {
    let nregs = n_gp_for_size(size);
    let regs: Vec<Register> = (0..nregs).map(gp_reg).collect();
    return AbiRet::Composite { regs, size };
  }

  // > 16 bytes → caller-allocated slot, pointer in X8.
  // Reserve the slot *before* arg classification so the
  // arg layout knows about it.
  let aligned_size = round_up_8(size);
  let slot_offset = state.stack;
  state.stack += aligned_size;

  AbiRet::Indirect { slot_offset, size }
}

// --- Helpers ------------------------------------------------

fn gp_reg(idx: usize) -> Register {
  match idx {
    0 => X0,
    1 => X1,
    2 => X2,
    3 => X3,
    4 => X4,
    5 => X5,
    6 => X6,
    7 => X7,
    _ => unreachable!("classify_gp must guard idx < 8"),
  }
}

fn fp_reg(idx: usize) -> FpRegister {
  // We always name the D register; the emitter uses the
  // S form (low half of V) when the AbiArg's `narrow`
  // flag is set or when the field width is F32.
  match idx {
    0 => D0,
    1 => D1,
    2 => D2,
    3 => D3,
    4 => D4,
    5 => D5,
    6 => D6,
    7 => D7,
    _ => unreachable!("classify_fp_scalar must guard idx < 8"),
  }
}

fn n_gp_for_size(size: u32) -> usize {
  size.div_ceil(8) as usize
}

fn round_up_8(n: u32) -> u32 {
  (n + 7) & !7
}

fn round_up_16(n: u32) -> u32 {
  (n + 15) & !15
}

fn int_width_bytes(w: IntWidth) -> u32 {
  match w {
    IntWidth::S8 | IntWidth::U8 => 1,
    IntWidth::S16 | IntWidth::U16 => 2,
    IntWidth::S32 | IntWidth::U32 => 4,
    IntWidth::S64 | IntWidth::U64 | IntWidth::Arch => 8,
  }
}

fn float_width_bytes(w: FloatWidth) -> u32 {
  match w {
    FloatWidth::F32 => 4,
    FloatWidth::F64 | FloatWidth::Arch => 8,
  }
}

#[cfg(test)]
mod tests;
