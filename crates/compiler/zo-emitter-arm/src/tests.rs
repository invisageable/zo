//! Unit tests for the ARM64 emitter — focused on the branch
//! fix-up primitives that the enum pretty-printer in
//! `zo-codegen-arm` depends on. Decoded byte-level assertions
//! so a regression in the patch math is caught at this layer,
//! not downstream in codegen.

use crate::{
  ARM64Emitter, COND_EQ, COND_NE, D0, D1, D2, D3, D4, D5, D6, D7, FmaOperands,
  FpRegister, Register, X0,
};

// Expected encodings (re-derived from the ARM64 reference so
// the tests fail loudly if the emitter constants drift):
const BCOND_OPCODE: u32 = 0x54000000;
const B_OPCODE: u32 = 0x14000000;
const IMM19_MASK: u32 = 0x7FFFF;
const IMM26_MASK: u32 = 0x3FFFFFF;

/// Read one little-endian u32 instruction at `pos` bytes.
fn read_insn(code: &[u8], pos: usize) -> u32 {
  u32::from_le_bytes([code[pos], code[pos + 1], code[pos + 2], code[pos + 3]])
}

#[test]
fn patch_bcond_at_preserves_condition_code() {
  // Emit `b.ne 0` so the condition code bits are NE.
  let mut emitter = ARM64Emitter::new();
  let pos = emitter.current_offset() as usize;

  emitter.emit_bne(0);

  // Patch with a 32-byte forward offset. Condition must
  // survive the rewrite — otherwise the patcher is blowing
  // the cmp-chain's control flow by rewriting NE as EQ.
  emitter.patch_bcond_at(pos, 32);

  let insn = read_insn(&emitter.code(), pos);
  let cond = insn & 0xF;

  assert_eq!(
    cond, COND_NE as u32,
    "patch_bcond_at clobbered the condition code: got {cond:#x}, expected NE ({:#x})",
    COND_NE as u32,
  );
}

#[test]
fn patch_bcond_at_writes_correct_imm19() {
  let mut emitter = ARM64Emitter::new();
  let pos = emitter.current_offset() as usize;

  emitter.emit_beq(0);
  emitter.patch_bcond_at(pos, 64);

  let insn = read_insn(&emitter.code(), pos);
  let expected_imm19 = ((64_i32 >> 2) as u32) & IMM19_MASK;
  let got_imm19 = (insn >> 5) & IMM19_MASK;

  assert_eq!(got_imm19, expected_imm19);

  // And the cond must still be EQ.
  assert_eq!(insn & 0xF, COND_EQ as u32);

  // And the top opcode bits must still identify a B.cond.
  assert_eq!(insn & 0xFF000000, BCOND_OPCODE & 0xFF000000);
}

#[test]
fn patch_bcond_at_handles_negative_offset() {
  // Backward branches (to a label earlier in the code) use
  // negative offsets. The emitter masks into 19 bits, so the
  // sign bit flows into imm19 via the mask — verify a round
  // trip matches what `emit_bcond` would have produced directly.
  let mut emitter = ARM64Emitter::new();

  // Emit some filler so the branch has somewhere to jump back.
  for _ in 0..4 {
    emitter.emit_mov_reg(X0, X0);
  }

  let pos = emitter.current_offset() as usize;

  emitter.emit_bne(0);
  emitter.patch_bcond_at(pos, -16);

  let got = read_insn(&emitter.code(), pos);

  // What a freshly-emitted bne(-16) would have produced at the
  // same position.
  let mut reference = ARM64Emitter::new();

  for _ in 0..4 {
    reference.emit_mov_reg(X0, X0);
  }

  reference.emit_bne(-16);

  let expected = read_insn(&reference.code(), pos);

  assert_eq!(got, expected);
}

#[test]
fn patch_b_at_writes_correct_imm26() {
  let mut emitter = ARM64Emitter::new();
  let pos = emitter.current_offset() as usize;

  emitter.emit_b(0);
  emitter.patch_b_at(pos, 128);

  let insn = read_insn(&emitter.code(), pos);
  let expected_imm26 = ((128_i32 >> 2) as u32) & IMM26_MASK;
  let got_imm26 = insn & IMM26_MASK;

  assert_eq!(got_imm26, expected_imm26);
  // Top bits must still identify a plain B (opcode 0x14).
  assert_eq!(insn & 0xFC000000, B_OPCODE);
}

#[test]
fn patch_b_at_matches_direct_emit_b() {
  // `emit_b(N)` and `emit_b(0) + patch_b_at(pos, N)` must be
  // byte-identical — otherwise the enum pretty-printer's jump
  // to `.done` will end up at the wrong address.
  for offset in [0, 4, 64, 1024, -8, -1024] {
    let mut patched = ARM64Emitter::new();
    let pos = patched.current_offset() as usize;

    patched.emit_b(0);
    patched.patch_b_at(pos, offset);

    let mut direct = ARM64Emitter::new();

    direct.emit_b(offset);

    assert_eq!(
      patched.code(),
      direct.code(),
      "patch_b_at diverged from emit_b for offset {offset}",
    );
  }
}

#[test]
fn patch_bcond_at_matches_direct_emit_bcond() {
  // Round-trip for each of the common condition codes that the
  // enum pretty-printer actually uses.
  for (emit_fn, cond) in [
    (
      ARM64Emitter::emit_bne as fn(&mut ARM64Emitter, i32),
      COND_NE,
    ),
    (
      ARM64Emitter::emit_beq as fn(&mut ARM64Emitter, i32),
      COND_EQ,
    ),
  ] {
    for offset in [0, 4, 64, 1024, -16, -1024] {
      let mut patched = ARM64Emitter::new();
      let pos = patched.current_offset() as usize;

      emit_fn(&mut patched, 0);
      patched.patch_bcond_at(pos, offset);

      let mut direct = ARM64Emitter::new();

      emit_fn(&mut direct, offset);

      assert_eq!(
        patched.code(),
        direct.code(),
        "patch_bcond_at diverged from emit_b.cond ({cond}) for offset {offset}",
      );
    }
  }
}

/// BLR Xn pins the encoding `1101011_0001_11111_000000_Xn_00000` —
/// 0xD63F0000 | (Xn << 5). A regression here would silently
/// reroute every dynamic-dispatch site, so check every Xn in
/// the standard call-clobbered range.
#[test]
fn emit_blr_encoding_per_register() {
  for n in 0u8..=16 {
    let mut emitter = ARM64Emitter::new();
    let reg = Register::new(n);

    emitter.emit_blr(reg);

    let insn = read_insn(&emitter.code(), 0);
    let expected = 0xD63F_0000 | ((n as u32) << 5);

    assert_eq!(
      insn, expected,
      "BLR x{n}: got {insn:#010x}, expected {expected:#010x}",
    );
  }
}

/// One fused-multiply-add emitter method, for table-driven
/// encoding checks.
type FmaEmit = fn(&mut ARM64Emitter, FmaOperands);

/// Exact words for the four fused multiply-add forms with all
/// operands `d0`. Reference taken from the system assembler
/// (`clang -c` + `otool -t`): each is a distinct word in the
/// FP data-processing 3-source family.
#[test]
fn emit_fma_zero_register_encodings() {
  let ops = FmaOperands {
    dst: D0,
    mul_lhs: D0,
    mul_rhs: D0,
    addend: D0,
  };

  let cases: [(FmaEmit, u32, &str); 4] = [
    (ARM64Emitter::emit_fmadd, 0x1F40_0000, "fmadd"),
    (ARM64Emitter::emit_fmsub, 0x1F40_8000, "fmsub"),
    (ARM64Emitter::emit_fnmadd, 0x1F60_0000, "fnmadd"),
    (ARM64Emitter::emit_fnmsub, 0x1F60_8000, "fnmsub"),
  ];

  for (emit, expected, name) in cases {
    let mut emitter = ARM64Emitter::new();

    emit(&mut emitter, ops);

    let insn = read_insn(&emitter.code(), 0);

    assert_eq!(
      insn, expected,
      "{name} d0,d0,d0,d0: got {insn:#010x}, expected {expected:#010x}",
    );
  }
}

/// Distinct registers in every field, checked against the
/// assembler reference `fmadd d3, d1, d2, d4 = 1f421023` and
/// `fmadd d5, d6, d7, d8 = 1f4720c5`. Confirms the field
/// placement: Rd[0..4], Rn[5..9], Ra[10..14], Rm[16..20].
#[test]
fn emit_fmadd_distinct_registers() {
  let mut emitter = ARM64Emitter::new();

  emitter.emit_fmadd(FmaOperands {
    dst: D3,
    mul_lhs: D1,
    mul_rhs: D2,
    addend: D4,
  });
  emitter.emit_fmadd(FmaOperands {
    dst: D5,
    mul_lhs: D6,
    mul_rhs: D7,
    addend: FpRegister::new(8),
  });

  assert_eq!(read_insn(&emitter.code(), 0), 0x1F42_1023);
  assert_eq!(read_insn(&emitter.code(), 4), 0x1F47_20C5);
}

/// Exact words for SMULH / UMULH against the system assembler
/// (`smulh x0,x0,x0 = 9b407c00`, `umulh x0,x0,x0 = 9bc07c00`,
/// `smulh x3,x1,x2 = 9b427c23`, `umulh x5,x6,x7 = 9bc77cc5`).
/// These back the magic-number divide-by-constant sequence.
#[test]
fn emit_mulh_encodings() {
  let mut emitter = ARM64Emitter::new();

  emitter.emit_smulh(X0, X0, X0);
  emitter.emit_umulh(X0, X0, X0);
  emitter.emit_smulh(Register::new(3), Register::new(1), Register::new(2));
  emitter.emit_umulh(Register::new(5), Register::new(6), Register::new(7));

  assert_eq!(read_insn(&emitter.code(), 0), 0x9B40_7C00);
  assert_eq!(read_insn(&emitter.code(), 4), 0x9BC0_7C00);
  assert_eq!(read_insn(&emitter.code(), 8), 0x9B42_7C23);
  assert_eq!(read_insn(&emitter.code(), 12), 0x9BC7_7CC5);
}
