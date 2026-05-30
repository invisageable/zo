// System V AMD64 integer argument order: RDI, RSI, RDX, RCX,
// R8, R9 — then the stack. Integer returns land in RAX (and
// RDX for the high half). Callee-saved: RBX, RBP, R12-R15.
// The register index doubles as the 4-bit encoding used by
// ModRM/SIB; bit 3 spills into the REX prefix (REX.R / REX.B
// / REX.X), leaving the low 3 bits in the instruction byte.

/// Represents an x86-64 [`Register`] instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Register {
  /// The 4-bit register number (0..16).
  index: u8,
}

impl Register {
  /// Creates a new [`Register`] instance.
  pub const fn new(index: u8) -> Self {
    assert!(index < 16);

    Self { index }
  }

  /// Gets the 4-bit register number.
  #[inline(always)]
  pub fn index(&self) -> u8 {
    self.index
  }

  /// Gets the low 3 bits encoded directly in ModRM/SIB/opcode.
  #[inline(always)]
  pub fn low3(&self) -> u8 {
    self.index & 0x7
  }

  /// Reports whether bit 3 is set (needs a REX extension bit).
  #[inline(always)]
  pub fn is_extended(&self) -> bool {
    self.index >= 8
  }

  /// REX.R contribution when this register sits in ModRM.reg.
  #[inline(always)]
  pub fn rex_r(&self) -> u8 {
    if self.is_extended() { 0x04 } else { 0 }
  }

  /// REX.B contribution when this register sits in ModRM.rm.
  #[inline(always)]
  pub fn rex_b(&self) -> u8 {
    if self.is_extended() { 0x01 } else { 0 }
  }
}

pub const RAX: Register = Register::new(0);
pub const RCX: Register = Register::new(1);
pub const RDX: Register = Register::new(2);
pub const RBX: Register = Register::new(3);
pub const RSP: Register = Register::new(4);
pub const RBP: Register = Register::new(5);
pub const RSI: Register = Register::new(6);
pub const RDI: Register = Register::new(7);
pub const R8: Register = Register::new(8);
pub const R9: Register = Register::new(9);
pub const R10: Register = Register::new(10);
pub const R11: Register = Register::new(11);
pub const R12: Register = Register::new(12);
pub const R13: Register = Register::new(13);
pub const R14: Register = Register::new(14);
pub const R15: Register = Register::new(15);
