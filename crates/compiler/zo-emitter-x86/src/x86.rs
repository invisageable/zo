use crate::register::Register;

/// REX prefix base; REX.W (bit 3) selects 64-bit operand size.
const REX_W: u8 = 0x48;
/// REX.B extension for a register encoded in the opcode byte.
const REX_B: u8 = 0x41;
/// `MOV r/m64, r64` opcode (Intel SDM Vol 2, MOV).
const OPCODE_MOV_RM_R: u8 = 0x89;
/// `PUSH r64` opcode base (`0x50 + rd`).
const OPCODE_PUSH_BASE: u8 = 0x50;
/// `POP r64` opcode base (`0x58 + rd`).
const OPCODE_POP_BASE: u8 = 0x58;
/// `JMP rel32` opcode.
const OPCODE_JMP_REL32: u8 = 0xE9;
/// Two-byte `Jcc rel32` escape; OR with the `tttn` code below.
const OPCODE_JCC_REL32: u8 = 0x80;
/// Two-byte opcode escape prefix.
const ESCAPE_0F: u8 = 0x0F;
/// `RET` near return.
const OPCODE_RET: u8 = 0xC3;
/// One-byte `NOP`.
const OPCODE_NOP: u8 = 0x90;
/// ModRM mod field for register-direct addressing (`11`).
const MODRM_MOD_REG: u8 = 0b11;
/// Width in bytes of a rel32 displacement.
const REL32_WIDTH: u32 = 4;

/// Condition codes (`tttn`) for the two-byte `Jcc` forms.
pub const COND_E: u8 = 0x4;
pub const COND_NE: u8 = 0x5;
pub const COND_B: u8 = 0x2;
pub const COND_AE: u8 = 0x3;
pub const COND_L: u8 = 0xC;
pub const COND_GE: u8 = 0xD;
pub const COND_LE: u8 = 0xE;
pub const COND_G: u8 = 0xF;

/// A deferred rel32 branch site to back-patch once its target
/// offset is known. Mirrors the AArch64 emitter's patch model.
pub struct PatchSite {
  /// File offset of the rel32 displacement to overwrite.
  patch_pos: u32,
}

/// Represents an [`X64Emitter`] instance.
pub struct X64Emitter {
  code: Vec<u8>,
}

impl X64Emitter {
  /// Creates a new [`X64Emitter`] instance.
  pub fn new() -> Self {
    Self { code: Vec::new() }
  }

  /// Gets the generated binary code.
  pub fn code(&self) -> Vec<u8> {
    self.code.clone()
  }

  /// Gets the current code position for PC-relative math.
  pub fn current_offset(&self) -> u32 {
    self.code.len() as u32
  }

  /// Emits one raw byte.
  fn emit_u8(&mut self, byte: u8) {
    self.code.push(byte);
  }

  /// Emits a 32-bit immediate in little-endian.
  fn emit_u32(&mut self, value: u32) {
    self.code.extend(&value.to_le_bytes());
  }

  /// Builds a ModRM byte from mod, reg, and rm fields.
  fn modrm(mode: u8, reg: u8, rm: u8) -> u8 {
    (mode << 6) | (reg << 3) | rm
  }

  /// `RET` — near return to caller.
  pub fn emit_ret(&mut self) {
    self.emit_u8(OPCODE_RET);
  }

  /// `NOP` — one-byte no-op.
  pub fn emit_nop(&mut self) {
    self.emit_u8(OPCODE_NOP);
  }

  /// `PUSH r64` — `0x50 + rd`, REX.B when `reg` is R8-R15.
  pub fn emit_push_reg(&mut self, reg: Register) {
    if reg.is_extended() {
      self.emit_u8(REX_B);
    }

    self.emit_u8(OPCODE_PUSH_BASE | reg.low3());
  }

  /// `POP r64` — `0x58 + rd`, REX.B when `reg` is R8-R15.
  pub fn emit_pop_reg(&mut self, reg: Register) {
    if reg.is_extended() {
      self.emit_u8(REX_B);
    }

    self.emit_u8(OPCODE_POP_BASE | reg.low3());
  }

  /// `MOV dst, src` (64-bit) — REX.W + `0x89 /r`.
  ///
  /// Opcode 0x89 places the source in ModRM.reg and the
  /// destination in ModRM.rm, so extended source flips REX.R
  /// and extended destination flips REX.B.
  pub fn emit_mov_reg_reg(&mut self, dst: Register, src: Register) {
    self.emit_u8(REX_W | src.rex_r() | dst.rex_b());
    self.emit_u8(OPCODE_MOV_RM_R);
    self.emit_u8(Self::modrm(MODRM_MOD_REG, src.low3(), dst.low3()));
  }

  /// `JMP rel32` with a placeholder displacement; returns the
  /// site to patch once the destination offset is known.
  pub fn emit_jmp(&mut self) -> PatchSite {
    self.emit_u8(OPCODE_JMP_REL32);

    let patch_pos = self.current_offset();

    self.emit_u32(0);

    PatchSite { patch_pos }
  }

  /// `Jcc rel32` for condition `cond` (a `COND_*` code).
  pub fn emit_jcc(&mut self, cond: u8) -> PatchSite {
    self.emit_u8(ESCAPE_0F);
    self.emit_u8(OPCODE_JCC_REL32 | cond);

    let patch_pos = self.current_offset();

    self.emit_u32(0);

    PatchSite { patch_pos }
  }

  /// Back-patches a rel32 `site` to reach `target` (a code
  /// offset). The displacement is measured from the end of the
  /// 4-byte immediate.
  pub fn patch_rel32(&mut self, site: PatchSite, target: u32) {
    let next = site.patch_pos + REL32_WIDTH;
    let rel = (i64::from(target) - i64::from(next)) as i32;
    let at = site.patch_pos as usize;

    self.code[at..at + REL32_WIDTH as usize]
      .copy_from_slice(&rel.to_le_bytes());
  }
}

impl Default for X64Emitter {
  fn default() -> Self {
    Self::new()
  }
}
