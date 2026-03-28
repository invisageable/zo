use crate::register::{FpRegister, Register};

// --- ARM64 Instruction Opcodes ---
const MOVZ: u32 = 0xD2800000;
const MOVK: u32 = 0xF2800000;
const MOV_REG: u32 = 0xAA0003E0;
const ADR: u32 = 0x10000000;
const ADRP: u32 = 0x90000000;
const STR: u32 = 0xF9000000;
const STRB: u32 = 0x39000000;
const STRB_POST: u32 = 0x38000400;
const LDR: u32 = 0xF9400000;
const ADD_IMM: u32 = 0x91000000;
const SUB_IMM: u32 = 0xD1000000;
const B: u32 = 0x14000000;
const BL: u32 = 0x94000000;
const RET_INSN: u32 = 0xD65F03C0;
const SVC: u32 = 0xD4000001;
const NOP_INSN: u32 = 0xD503201F;
const ADD_REG: u32 = 0x8B000000;
const SUB_REG: u32 = 0xCB000000;
const MUL: u32 = 0x9B007C00;
const SDIV: u32 = 0x9AC00C00;
const UDIV: u32 = 0x9AC00800;
const AND: u32 = 0x8A000000;
const ORR: u32 = 0xAA000000;
const EOR: u32 = 0xCA000000;
const CMP_REG: u32 = 0xEB00001F;
const CMP_IMM: u32 = 0xF100001F;
const CSEL: u32 = 0x9A800000;
const CBZ: u32 = 0xB4000000;
const CBNZ: u32 = 0xB5000000;
const MSUB: u32 = 0x9B008000;
const BCOND: u32 = 0x54000000;
const UBFM: u32 = 0xD3400000;
const SBFM: u32 = 0x93400000;
const STP_PRE: u32 = 0xA9800000;
const LDP_POST: u32 = 0xA8C00000;
const FMOV_GP_FP: u32 = 0x9E670000;
const FMOV_FP_FP: u32 = 0x1E604000;
const FADD: u32 = 0x1E602800;
const FSUB: u32 = 0x1E603800;
const FMUL: u32 = 0x1E600800;
const FDIV: u32 = 0x1E601800;
const FCMP: u32 = 0x1E602000;
const FSQRT: u32 = 0x1E61C000;
const FRINTM: u32 = 0x1E654000; // floor
const FRINTP: u32 = 0x1E64C000; // ceil
const FRINTZ: u32 = 0x1E65C000; // trunc
const FRINTN: u32 = 0x1E644000; // round nearest
const FCVTZS: u32 = 0x9E780000;
const SCVTF: u32 = 0x9E620000;
const LDR_FP: u32 = 0xFD400000;
const STR_FP: u32 = 0xFD000000;

// --- Encoding Masks ---
const IMM2_MASK: u32 = 0x3;
const IMM6_MASK: u32 = 0x3F;
const IMM7_MASK: u32 = 0x7F;
const IMM9_NEG1: u32 = 0x1FF;
const IMM12_MASK: u32 = 0xFFF;
const IMM19_MASK: u32 = 0x7FFFF;
const IMM26_MASK: u32 = 0x3FFFFFF;
const PAGE_MASK: u64 = 0xFFF;
const PAGE_SHIFT: u32 = 12;

// --- Condition Codes ---
pub const COND_EQ: u8 = 0x0;
pub const COND_NE: u8 = 0x1;
pub const COND_GE: u8 = 0xA;
pub const COND_LT: u8 = 0xB;
pub const COND_GT: u8 = 0xC;
pub const COND_LE: u8 = 0xD;

/// Represents an [`ARM64Emitter`] instance.
pub struct ARM64Emitter {
  code: Vec<u8>,
}
impl ARM64Emitter {
  /// Creates a new [`ARM64Emitter`] instance.
  pub fn new() -> Self {
    Self { code: Vec::new() }
  }

  /// Gets the generated binary code.
  pub fn code(&self) -> Vec<u8> {
    self.code.clone()
  }

  /// Gets current code position (PC-relative calculations).
  pub fn current_offset(&self) -> u32 {
    self.code.len() as u32
  }

  // Emits a 32-bit instruction in little-endian.
  fn emit_u32(&mut self, insn: u32) {
    self.code.extend(&insn.to_le_bytes());
  }

  /// MOV immediate (actually MOVZ for 16-bit immediates).
  ///
  /// MOVZ Xd, #imm16, LSL #0
  /// Encoding: sf=1 opc=10 100101 hw=00 imm16 Rd
  pub fn emit_mov_imm(&mut self, reg: Register, imm: u16) {
    let insn = MOVZ | ((imm as u32) << 5) | (reg.index() as u32);

    self.emit_u32(insn);
  }

  /// MOVK - move 16-bit immediate to register keeping
  /// other bits.
  ///
  /// MOVK Xd, #imm16, LSL #shift
  /// Encoding: sf=1 opc=11 100101 hw imm16 Rd
  pub fn emit_movk(&mut self, reg: Register, imm: u16, shift: u8) {
    let hw = (shift / 16) as u32;

    let insn = MOVK | (hw << 21) | ((imm as u32) << 5) | (reg.index() as u32);

    self.emit_u32(insn);
  }

  /// MOV register to register.
  pub fn emit_mov_reg(&mut self, dst: Register, src: Register) {
    let insn = MOV_REG | ((src.index() as u32) << 16) | (dst.index() as u32);

    self.emit_u32(insn);
  }

  // ADR - form PC-relative address.
  pub fn emit_adr(&mut self, reg: Register, offset: i32) {
    let imm = offset;
    let immlo = (imm as u32) & IMM2_MASK;
    let immhi = ((imm >> 2) as u32) & IMM19_MASK;
    let insn = ADR | (immlo << 29) | (immhi << 5) | (reg.index() as u32);

    self.emit_u32(insn);
  }

  // ADRP - form page address (4KB aligned).
  pub fn emit_adrp(&mut self, reg: Register, page_offset: i32) {
    let imm = page_offset;
    let immlo = (imm as u32) & IMM2_MASK;
    let immhi = ((imm >> 2) as u32) & IMM19_MASK;
    let insn = ADRP | (immlo << 29) | (immhi << 5) | (reg.index() as u32);

    self.emit_u32(insn);
  }

  // ADRP with PC and target address calculation.
  pub fn emit_adrp_to_addr(
    &mut self,
    reg: Register,
    current_pc: u64,
    target_addr: u64,
  ) {
    let current_page = current_pc & !PAGE_MASK;
    let target_page = target_addr & !PAGE_MASK;
    let page_diff =
      ((target_page as i64) - (current_page as i64)) >> PAGE_SHIFT;

    self.emit_adrp(reg, page_diff as i32);
  }

  // Store register (64-bit).
  pub fn emit_str(&mut self, reg: Register, base: Register, offset: i16) {
    let imm12 = ((offset as u32) >> 3) & IMM12_MASK;

    let insn =
      STR | (imm12 << 10) | ((base.index() as u32) << 5) | (reg.index() as u32);

    self.emit_u32(insn);
  }

  // Store register byte.
  pub fn emit_strb(&mut self, reg: Register, base: Register, offset: i16) {
    let imm12 = (offset as u32) & IMM12_MASK;

    let insn = STRB
      | (imm12 << 10)
      | ((base.index() as u32) << 5)
      | (reg.index() as u32);

    self.emit_u32(insn);
  }

  // Loads register (64-bit).
  pub fn emit_ldr(&mut self, reg: Register, base: Register, offset: i16) {
    let imm12 = ((offset as u32) >> 3) & IMM12_MASK;

    let insn =
      LDR | (imm12 << 10) | ((base.index() as u32) << 5) | (reg.index() as u32);

    self.emit_u32(insn);
  }

  // Adds immediate.
  pub fn emit_add_imm(&mut self, dst: Register, src: Register, imm: u16) {
    let imm12 = (imm as u32) & IMM12_MASK;

    let insn = ADD_IMM
      | (imm12 << 10)
      | ((src.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  // Subtracts immediate.
  pub fn emit_sub_imm(&mut self, dst: Register, src: Register, imm: u16) {
    let imm12 = (imm as u32) & IMM12_MASK;

    let insn = SUB_IMM
      | (imm12 << 10)
      | ((src.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  /// Branch (unconditional).
  pub fn emit_b(&mut self, offset: i32) {
    let imm26 = ((offset >> 2) as u32) & IMM26_MASK;
    let insn = B | imm26;

    self.emit_u32(insn);
  }

  /// Branch with link (call).
  pub fn emit_bl(&mut self, offset: i32) {
    let imm26 = ((offset >> 2) as u32) & IMM26_MASK;
    let insn = BL | imm26;

    self.emit_u32(insn);
  }

  /// Emits return.
  pub fn emit_ret(&mut self) {
    self.emit_u32(RET_INSN);
  }

  /// Emits Supervisor call (system call).
  pub fn emit_svc(&mut self, imm: u16) {
    let insn = SVC | ((imm as u32) << 5);

    self.emit_u32(insn);
  }

  /// Emits NOP.
  pub fn emit_nop(&mut self) {
    self.emit_u32(NOP_INSN);
  }

  /// Emits `ADD` register to register.
  pub fn emit_add(&mut self, dst: Register, src1: Register, src2: Register) {
    let insn = ADD_REG
      | ((src2.index() as u32) << 16)
      | ((src1.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  /// Emits `SUB` (subtract) register from register.
  pub fn emit_sub(&mut self, dst: Register, src1: Register, src2: Register) {
    let insn = SUB_REG
      | ((src2.index() as u32) << 16)
      | ((src1.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  /// Emits `MUL` Multiply register by register.
  pub fn emit_mul(&mut self, dst: Register, src1: Register, src2: Register) {
    let insn = MUL
      | ((src2.index() as u32) << 16)
      | ((src1.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  /// Emits `SDIV` (Signed divide).
  pub fn emit_sdiv(&mut self, dst: Register, src1: Register, src2: Register) {
    let insn = SDIV
      | ((src2.index() as u32) << 16)
      | ((src1.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  // Unsigned divide.
  pub fn emit_udiv(&mut self, dst: Register, src1: Register, src2: Register) {
    let insn = UDIV
      | ((src2.index() as u32) << 16)
      | ((src1.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  // Logical AND.
  pub fn emit_and(&mut self, dst: Register, src1: Register, src2: Register) {
    let insn = AND
      | ((src2.index() as u32) << 16)
      | ((src1.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  // Logical OR.
  pub fn emit_orr(&mut self, dst: Register, src1: Register, src2: Register) {
    let insn = ORR
      | ((src2.index() as u32) << 16)
      | ((src1.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  // Logical XOR.
  pub fn emit_eor(&mut self, dst: Register, src1: Register, src2: Register) {
    let insn = EOR
      | ((src2.index() as u32) << 16)
      | ((src1.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  // Compare (sets flags).
  pub fn emit_cmp(&mut self, src1: Register, src2: Register) {
    let insn =
      CMP_REG | ((src2.index() as u32) << 16) | ((src1.index() as u32) << 5);

    self.emit_u32(insn);
  }

  // Compare with immediate.
  pub fn emit_cmp_imm(&mut self, src: Register, imm: u16) {
    let imm12 = (imm as u32) & IMM12_MASK;
    let insn = CMP_IMM | (imm12 << 10) | ((src.index() as u32) << 5);

    self.emit_u32(insn);
  }

  // Conditional select (CSEL).
  pub fn emit_csel(
    &mut self,
    dst: Register,
    src1: Register,
    src2: Register,
    cond: u8,
  ) {
    let insn = CSEL
      | ((src2.index() as u32) << 16)
      | ((cond as u32) << 12)
      | ((src1.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  // Branch if equal.
  pub fn emit_beq(&mut self, offset: i32) {
    self.emit_bcond(COND_EQ, offset);
  }

  // Branch if not equal.
  pub fn emit_bne(&mut self, offset: i32) {
    self.emit_bcond(COND_NE, offset);
  }

  // Branch if less than (signed).
  pub fn emit_blt(&mut self, offset: i32) {
    self.emit_bcond(COND_LT, offset);
  }

  // Branch if greater than (signed).
  pub fn emit_bgt(&mut self, offset: i32) {
    self.emit_bcond(COND_GT, offset);
  }

  // Branch if less than or equal (signed).
  pub fn emit_ble(&mut self, offset: i32) {
    self.emit_bcond(COND_LE, offset);
  }

  // Branch if greater than or equal (signed).
  pub fn emit_bge(&mut self, offset: i32) {
    self.emit_bcond(COND_GE, offset);
  }

  /// Compare and branch if zero.
  pub fn emit_cbz(&mut self, rt: Register, offset: i32) {
    let imm19 = ((offset >> 2) as u32) & IMM19_MASK;
    let insn = CBZ | (imm19 << 5) | (rt.index() as u32);

    self.emit_u32(insn);
  }

  /// CBNZ Xt, label — branch if register is non-zero.
  pub fn emit_cbnz(&mut self, rt: Register, offset: i32) {
    let imm19 = ((offset >> 2) as u32) & IMM19_MASK;
    let insn = CBNZ | (imm19 << 5) | (rt.index() as u32);

    self.emit_u32(insn);
  }

  /// MSUB Xd, Xn, Xm, Xa — Xd = Xa - Xn * Xm.
  pub fn emit_msub(
    &mut self,
    dst: Register,
    src1: Register,
    src2: Register,
    acc: Register,
  ) {
    let insn = MSUB
      | ((src2.index() as u32) << 16)
      | ((acc.index() as u32) << 10)
      | ((src1.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  /// STRB Wt, [Xn], #-1 — store byte with
  /// post-decrement.
  pub fn emit_strb_post_dec(&mut self, reg: Register, base: Register) {
    let insn = STRB_POST
      | (IMM9_NEG1 << 12)
      | ((base.index() as u32) << 5)
      | (reg.index() as u32);

    self.emit_u32(insn);
  }

  fn emit_bcond(&mut self, cond: u8, offset: i32) {
    let imm19 = ((offset >> 2) as u32) & IMM19_MASK;
    let insn = BCOND | (imm19 << 5) | (cond as u32);

    self.emit_u32(insn);
  }

  // Left shift.
  pub fn emit_lsl(&mut self, dst: Register, src: Register, shift: u8) {
    let immr = (64 - (shift as u32)) & IMM6_MASK;
    let imms = 63 - (shift as u32);

    let insn = UBFM
      | (immr << 16)
      | (imms << 10)
      | ((src.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  // Logical right shift.
  pub fn emit_lsr(&mut self, dst: Register, src: Register, shift: u8) {
    let immr = shift as u32;
    let imms = IMM6_MASK;

    let insn = UBFM
      | (immr << 16)
      | (imms << 10)
      | ((src.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  // Arithmetic right shift.
  pub fn emit_asr(&mut self, dst: Register, src: Register, shift: u8) {
    let immr = shift as u32;
    let imms = IMM6_MASK;

    let insn = SBFM
      | (immr << 16)
      | (imms << 10)
      | ((src.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  // Store pair of registers.
  pub fn emit_stp(
    &mut self,
    rt1: Register,
    rt2: Register,
    base: Register,
    offset: i16,
  ) {
    let imm7 = ((offset >> 3) as u32) & IMM7_MASK;

    let insn = STP_PRE
      | (imm7 << 15)
      | ((rt2.index() as u32) << 10)
      | ((base.index() as u32) << 5)
      | (rt1.index() as u32);

    self.emit_u32(insn);
  }

  // Load pair of registers.
  pub fn emit_ldp(
    &mut self,
    rt1: Register,
    rt2: Register,
    base: Register,
    offset: i16,
  ) {
    let imm7 = ((offset >> 3) as u32) & IMM7_MASK;

    let insn = LDP_POST
      | (imm7 << 15)
      | ((rt2.index() as u32) << 10)
      | ((base.index() as u32) << 5)
      | (rt1.index() as u32);

    self.emit_u32(insn);
  }

  // === FLOATING-POINT INSTRUCTIONS ===

  /// FMOV Dd, Xn — move GP register to FP register.
  pub fn emit_fmov_gp_to_fp(&mut self, dst: FpRegister, src: Register) {
    let insn = FMOV_GP_FP | ((src.index() as u32) << 5) | (dst.index() as u32);

    self.emit_u32(insn);
  }

  /// FMOV Dd, Dn — move between FP registers (double).
  pub fn emit_fmov_fp(&mut self, dst: FpRegister, src: FpRegister) {
    let insn = FMOV_FP_FP | ((src.index() as u32) << 5) | (dst.index() as u32);

    self.emit_u32(insn);
  }

  /// FADD Dd, Dn, Dm — FP add (double).
  pub fn emit_fadd(
    &mut self,
    dst: FpRegister,
    src1: FpRegister,
    src2: FpRegister,
  ) {
    let insn = FADD
      | ((src2.index() as u32) << 16)
      | ((src1.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  /// FSUB Dd, Dn, Dm — FP subtract (double).
  pub fn emit_fsub(
    &mut self,
    dst: FpRegister,
    src1: FpRegister,
    src2: FpRegister,
  ) {
    let insn = FSUB
      | ((src2.index() as u32) << 16)
      | ((src1.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  /// FMUL Dd, Dn, Dm — FP multiply (double).
  pub fn emit_fmul(
    &mut self,
    dst: FpRegister,
    src1: FpRegister,
    src2: FpRegister,
  ) {
    let insn = FMUL
      | ((src2.index() as u32) << 16)
      | ((src1.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  /// FDIV Dd, Dn, Dm — FP divide (double).
  pub fn emit_fdiv(
    &mut self,
    dst: FpRegister,
    src1: FpRegister,
    src2: FpRegister,
  ) {
    let insn = FDIV
      | ((src2.index() as u32) << 16)
      | ((src1.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  /// FCMP Dn, Dm — FP compare (sets NZCV flags).
  pub fn emit_fcmp(&mut self, src1: FpRegister, src2: FpRegister) {
    let insn =
      FCMP | ((src2.index() as u32) << 16) | ((src1.index() as u32) << 5);

    self.emit_u32(insn);
  }

  /// FCVTZS Xd, Dn — convert double to signed int
  /// (truncate toward zero).
  pub fn emit_fcvtzs(&mut self, dst: Register, src: FpRegister) {
    let insn = FCVTZS | ((src.index() as u32) << 5) | (dst.index() as u32);

    self.emit_u32(insn);
  }

  /// SCVTF Dd, Xn — convert signed int to double.
  pub fn emit_scvtf(&mut self, dst: FpRegister, src: Register) {
    let insn = SCVTF | ((src.index() as u32) << 5) | (dst.index() as u32);

    self.emit_u32(insn);
  }

  /// FSQRT Dd, Dn — FP square root (double).
  pub fn emit_fsqrt(&mut self, dst: FpRegister, src: FpRegister) {
    let insn = FSQRT | ((src.index() as u32) << 5) | (dst.index() as u32);

    self.emit_u32(insn);
  }

  /// FRINTM Dd, Dn — round toward minus infinity (floor).
  pub fn emit_frintm(&mut self, dst: FpRegister, src: FpRegister) {
    let insn = FRINTM | ((src.index() as u32) << 5) | (dst.index() as u32);

    self.emit_u32(insn);
  }

  /// FRINTP Dd, Dn — round toward plus infinity (ceil).
  pub fn emit_frintp(&mut self, dst: FpRegister, src: FpRegister) {
    let insn = FRINTP | ((src.index() as u32) << 5) | (dst.index() as u32);

    self.emit_u32(insn);
  }

  /// FRINTZ Dd, Dn — round toward zero (trunc).
  pub fn emit_frintz(&mut self, dst: FpRegister, src: FpRegister) {
    let insn = FRINTZ | ((src.index() as u32) << 5) | (dst.index() as u32);

    self.emit_u32(insn);
  }

  /// FRINTN Dd, Dn — round to nearest (round).
  pub fn emit_frintn(&mut self, dst: FpRegister, src: FpRegister) {
    let insn = FRINTN | ((src.index() as u32) << 5) | (dst.index() as u32);

    self.emit_u32(insn);
  }

  /// LDR Dt, [Xn, #offset] — load double from memory.
  pub fn emit_ldr_fp(&mut self, dst: FpRegister, base: Register, offset: u16) {
    let imm12 = ((offset / 8) as u32) & IMM12_MASK;
    let insn = LDR_FP
      | (imm12 << 10)
      | ((base.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  /// STR Dt, [Xn, #offset] — store double to memory.
  pub fn emit_str_fp(&mut self, src: FpRegister, base: Register, offset: u16) {
    let imm12 = ((offset / 8) as u32) & IMM12_MASK;
    let insn = STR_FP
      | (imm12 << 10)
      | ((base.index() as u32) << 5)
      | (src.index() as u32);

    self.emit_u32(insn);
  }
}
impl Default for ARM64Emitter {
  fn default() -> Self {
    Self::new()
  }
}
