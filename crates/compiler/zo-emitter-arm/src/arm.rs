use crate::register::Register;

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
    let insn = 0xD2800000 | ((imm as u32) << 5) | (reg.index() as u32);

    self.emit_u32(insn);
  }

  /// MOVK - move 16-bit immediate to register keeping other bits.
  ///
  /// MOVK Xd, #imm16, LSL #shift
  /// Encoding: sf=1 opc=11 100101 hw imm16 Rd
  pub fn emit_movk(&mut self, reg: Register, imm: u16, shift: u8) {
    let hw = (shift / 16) as u32; // hw encodes the 16-bit halfword position.

    let insn =
      0xF2800000 | (hw << 21) | ((imm as u32) << 5) | (reg.index() as u32);

    self.emit_u32(insn);
  }

  /// MOV register to register.
  pub fn emit_mov_reg(&mut self, dst: Register, src: Register) {
    // ORR Xd, XZR, Xm (MOV is an alias)
    // Encoding: sf=1 01 01010 shift=00 0 Rm=src 000000 Rn=11111 Rd=dst
    let insn = 0xAA0003E0 | ((src.index() as u32) << 16) | (dst.index() as u32);

    self.emit_u32(insn);
  }

  // ADR - form PC-relative address.
  pub fn emit_adr(&mut self, reg: Register, offset: i32) {
    // ADR Xd, label
    // Encoding: 0 immlo=2bits 10000 immhi=19bits Rd
    let imm = offset;
    let immlo = (imm & 0x3) as u32;
    let immhi = ((imm >> 2) & 0x7FFFF) as u32;
    let insn = 0x10000000 | (immlo << 29) | (immhi << 5) | (reg.index() as u32);

    self.emit_u32(insn);
  }

  // ADRP - form page address (4KB aligned)
  // This version takes a pre-calculated page offset
  pub fn emit_adrp(&mut self, reg: Register, page_offset: i32) {
    // ADRP Xd, label
    // Encoding: 1 immlo=2bits 10000 immhi=19bits Rd
    // page_offset is already in pages, not bytes
    let imm = page_offset;
    let immlo = (imm & 0x3) as u32;
    let immhi = ((imm >> 2) & 0x7FFFF) as u32;
    let insn = 0x90000000 | (immlo << 29) | (immhi << 5) | (reg.index() as u32);

    self.emit_u32(insn);
  }

  // ADRP with PC and target address calculation.
  pub fn emit_adrp_to_addr(
    &mut self,
    reg: Register,
    current_pc: u64,
    target_addr: u64,
  ) {
    // Calculate page-aligned addresses
    let current_page = current_pc & !0xFFF;
    let target_page = target_addr & !0xFFF;

    // Calculate page offset (signed difference in pages)
    let page_diff = ((target_page as i64) - (current_page as i64)) >> 12;

    self.emit_adrp(reg, page_diff as i32);
  }

  // Store register (64-bit)
  pub fn emit_str(&mut self, reg: Register, base: Register, offset: i16) {
    // STR Xt, [Xn, #offset]
    // Encoding: 1x 111001 00 imm12 Rn Rt
    let imm12 = ((offset as u32) >> 3) & 0xFFF; // Scaled offset

    let insn = 0xF9000000
      | (imm12 << 10)
      | ((base.index() as u32) << 5)
      | (reg.index() as u32);

    self.emit_u32(insn);
  }

  // Store register byte
  pub fn emit_strb(&mut self, reg: Register, base: Register, offset: i16) {
    // STRB Wt, [Xn, #offset]
    // Encoding: 00 111001 00 imm12 Rn Rt
    let imm12 = (offset as u32) & 0xFFF; // Unscaled offset for byte

    let insn = 0x39000000
      | (imm12 << 10)
      | ((base.index() as u32) << 5)
      | (reg.index() as u32);

    self.emit_u32(insn);
  }

  // Loads register (64-bit).
  pub fn emit_ldr(&mut self, reg: Register, base: Register, offset: i16) {
    // LDR Xt, [Xn, #offset]
    // Encoding: 1x 111001 01 imm12 Rn Rt
    let imm12 = ((offset as u32) >> 3) & 0xFFF; // Scaled offset

    let insn = 0xF9400000
      | (imm12 << 10)
      | ((base.index() as u32) << 5)
      | (reg.index() as u32);

    self.emit_u32(insn);
  }

  // Adds immediate.
  pub fn emit_add_imm(&mut self, dst: Register, src: Register, imm: u16) {
    // ADD Xd, Xn, #imm12
    // Encoding: sf=1 00 10001 shift=00 imm12 Rn Rd
    let imm12 = (imm as u32) & 0xFFF;

    let insn = 0x91000000
      | (imm12 << 10)
      | ((src.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  // Subtracts immediate.
  pub fn emit_sub_imm(&mut self, dst: Register, src: Register, imm: u16) {
    // SUB Xd, Xn, #imm12
    // Encoding: sf=1 10 10001 shift=00 imm12 Rn Rd
    let imm12 = (imm as u32) & 0xFFF;

    let insn = 0xD1000000
      | (imm12 << 10)
      | ((src.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  /// Branch (unconditional).
  pub fn emit_b(&mut self, offset: i32) {
    // B label
    // Encoding: 0 00101 imm26
    let imm26 = ((offset >> 2) & 0x3FFFFFF) as u32;
    let insn = 0x14000000 | imm26;

    self.emit_u32(insn);
  }

  /// Branch with link (call).
  pub fn emit_bl(&mut self, offset: i32) {
    // BL label
    // Encoding: 1 00101 imm26
    let imm26 = ((offset >> 2) & 0x3FFFFFF) as u32;
    let insn = 0x94000000 | imm26;

    self.emit_u32(insn);
  }

  /// Emits return.
  pub fn emit_ret(&mut self) {
    // RET (actually RET X30)
    // Encoding: 1101011 0 0 10 11111 0000 0 0 11110 00000
    self.emit_u32(0xD65F03C0);
  }

  /// Emits Supervisor call (system call).
  pub fn emit_svc(&mut self, imm: u16) {
    // SVC #imm16
    // Encoding: 11010100 000 imm16 000 01
    let insn = 0xD4000001 | ((imm as u32) << 5);

    self.emit_u32(insn);
  }

  /// Emits NOP.
  pub fn emit_nop(&mut self) {
    self.emit_u32(0xD503201F);
  }

  /// Emits `ADD` register to register.
  pub fn emit_add(&mut self, dst: Register, src1: Register, src2: Register) {
    // ADD Xd, Xn, Xm
    // Encoding: sf=1 00 01011 shift=00 0 Rm Rn Rd
    let insn = 0x8B000000
      | ((src2.index() as u32) << 16)
      | ((src1.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  /// Emits `SUB` (subtract) register from register.
  pub fn emit_sub(&mut self, dst: Register, src1: Register, src2: Register) {
    // SUB Xd, Xn, Xm
    // Encoding: sf=1 10 01011 shift=00 0 Rm Rn Rd
    let insn = 0xCB000000
      | ((src2.index() as u32) << 16)
      | ((src1.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  /// Emits `MUL` Multiply register by register.
  pub fn emit_mul(&mut self, dst: Register, src1: Register, src2: Register) {
    // MUL Xd, Xn, Xm (alias for MADD Xd, Xn, Xm, XZR)
    // Encoding: sf=1 00 11011 000 Xm 0 11111 Xn Xd
    let insn = 0x9B007C00
      | ((src2.index() as u32) << 16)
      | ((src1.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  /// Emits `SDIV` (Signed divide).
  pub fn emit_sdiv(&mut self, dst: Register, src1: Register, src2: Register) {
    // SDIV Xd, Xn, Xm
    // Encoding: sf=1 00 11010110 Xm 00001 1 Xn Xd
    let insn = 0x9AC00C00
      | ((src2.index() as u32) << 16)
      | ((src1.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  // Unsigned divide
  pub fn emit_udiv(&mut self, dst: Register, src1: Register, src2: Register) {
    // UDIV Xd, Xn, Xm
    // Encoding: sf=1 00 11010110 Xm 00001 0 Xn Xd
    let insn = 0x9AC00800
      | ((src2.index() as u32) << 16)
      | ((src1.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  // Logical AND
  pub fn emit_and(&mut self, dst: Register, src1: Register, src2: Register) {
    // AND Xd, Xn, Xm
    // Encoding: sf=1 00 01010 shift=00 0 Rm Rn Rd
    let insn = 0x8A000000
      | ((src2.index() as u32) << 16)
      | ((src1.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  // Logical OR
  pub fn emit_orr(&mut self, dst: Register, src1: Register, src2: Register) {
    // ORR Xd, Xn, Xm
    // Encoding: sf=1 01 01010 shift=00 0 Rm Rn Rd
    let insn = 0xAA000000
      | ((src2.index() as u32) << 16)
      | ((src1.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  // Logical XOR
  pub fn emit_eor(&mut self, dst: Register, src1: Register, src2: Register) {
    // EOR Xd, Xn, Xm
    // Encoding: sf=1 10 01010 shift=00 0 Rm Rn Rd
    let insn = 0xCA000000
      | ((src2.index() as u32) << 16)
      | ((src1.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  // Compare (sets flags)
  pub fn emit_cmp(&mut self, src1: Register, src2: Register) {
    // CMP Xn, Xm (alias for SUBS XZR, Xn, Xm)
    // Encoding: sf=1 11 01011 shift=00 0 Rm Rn 11111
    let insn =
      0xEB00001F | ((src2.index() as u32) << 16) | ((src1.index() as u32) << 5);

    self.emit_u32(insn);
  }

  // Compare with immediate
  pub fn emit_cmp_imm(&mut self, src: Register, imm: u16) {
    // CMP Xn, #imm12 (alias for SUBS XZR, Xn, #imm12)
    // Encoding: sf=1 11 10001 shift=00 imm12 Rn 11111
    let imm12 = (imm as u32) & 0xFFF;
    let insn = 0xF100001F | (imm12 << 10) | ((src.index() as u32) << 5);

    self.emit_u32(insn);
  }

  // Conditional select (CSEL)
  pub fn emit_csel(
    &mut self,
    dst: Register,
    src1: Register,
    src2: Register,
    cond: u8,
  ) {
    // CSEL Xd, Xn, Xm, cond
    // Encoding: sf=1 00 11010100 Xm cond 00 Xn Xd
    let insn = 0x9A800000
      | ((src2.index() as u32) << 16)
      | ((cond as u32) << 12)
      | ((src1.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  // Branch if equal
  pub fn emit_beq(&mut self, offset: i32) {
    // B.EQ label (condition code 0000)
    self.emit_bcond(0x0, offset);
  }

  // Branch if not equal
  pub fn emit_bne(&mut self, offset: i32) {
    // B.NE label (condition code 0001)
    self.emit_bcond(0x1, offset);
  }

  // Branch if less than (signed)
  pub fn emit_blt(&mut self, offset: i32) {
    // B.LT label (condition code 1011)
    self.emit_bcond(0xB, offset);
  }

  // Branch if greater than (signed)
  pub fn emit_bgt(&mut self, offset: i32) {
    // B.GT label (condition code 1100)
    self.emit_bcond(0xC, offset);
  }

  // Branch if less than or equal (signed)
  pub fn emit_ble(&mut self, offset: i32) {
    // B.LE label (condition code 1101)
    self.emit_bcond(0xD, offset);
  }

  // Branch if greater than or equal (signed)
  pub fn emit_bge(&mut self, offset: i32) {
    // B.GE label (condition code 1010)
    self.emit_bcond(0xA, offset);
  }

  // Generic conditional branch
  fn emit_bcond(&mut self, cond: u8, offset: i32) {
    // B.cond label
    // Encoding: 0101010 0 imm19 0 cond
    let imm19 = ((offset >> 2) & 0x7FFFF) as u32;
    let insn = 0x54000000 | (imm19 << 5) | (cond as u32);

    self.emit_u32(insn);
  }

  // Left shift
  pub fn emit_lsl(&mut self, dst: Register, src: Register, shift: u8) {
    // LSL Xd, Xn, #shift (alias for UBFM)
    // Encoding: sf=1 10 100110 1 immr imms Rn Rd
    let immr = (64 - (shift as u32)) & 0x3F;
    let imms = 63 - (shift as u32);

    let insn = 0xD3400000
      | (immr << 16)
      | (imms << 10)
      | ((src.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  // Logical right shift
  pub fn emit_lsr(&mut self, dst: Register, src: Register, shift: u8) {
    // LSR Xd, Xn, #shift (alias for UBFM)
    // Encoding: sf=1 10 100110 1 immr imms Rn Rd
    let immr = shift as u32;
    let imms = 0x3F;

    let insn = 0xD3400000
      | (immr << 16)
      | (imms << 10)
      | ((src.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  // Arithmetic right shift
  pub fn emit_asr(&mut self, dst: Register, src: Register, shift: u8) {
    // ASR Xd, Xn, #shift (alias for SBFM)
    // Encoding: sf=1 00 100110 1 immr imms Rn Rd
    let immr = shift as u32;
    let imms = 0x3F;

    let insn = 0x93400000
      | (immr << 16)
      | (imms << 10)
      | ((src.index() as u32) << 5)
      | (dst.index() as u32);

    self.emit_u32(insn);
  }

  // Store pair of registers
  pub fn emit_stp(
    &mut self,
    rt1: Register,
    rt2: Register,
    base: Register,
    offset: i16,
  ) {
    // STP Xt1, Xt2, [Xn, #offset]!
    // Pre-index: Encoding: 10 101001 11 imm7 Rt2 Rn Rt1
    let imm7 = ((offset >> 3) & 0x7F) as u32;

    let insn = 0xA9800000
      | (imm7 << 15)
      | ((rt2.index() as u32) << 10)
      | ((base.index() as u32) << 5)
      | (rt1.index() as u32);

    self.emit_u32(insn);
  }

  // Load pair of registers
  pub fn emit_ldp(
    &mut self,
    rt1: Register,
    rt2: Register,
    base: Register,
    offset: i16,
  ) {
    // LDP Xt1, Xt2, [Xn], #offset
    // Post-index: Encoding: 10 101000 11 imm7 Rt2 Rn Rt1
    let imm7 = ((offset >> 3) & 0x7F) as u32;

    let insn = 0xA8C00000
      | (imm7 << 15)
      | ((rt2.index() as u32) << 10)
      | ((base.index() as u32) << 5)
      | (rt1.index() as u32);

    self.emit_u32(insn);
  }
}
impl Default for ARM64Emitter {
  fn default() -> Self {
    Self::new()
  }
}
