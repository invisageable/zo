// https://learn.microsoft.com/en-us/cpp/build/arm64-windows-abi-conventions?view=msvc-170

/// Represents the ARM64 [`Register`] instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Register {
  /// The index of the register.
  index: u8,
}

impl Register {
  /// Creates a new [`Register`] instance.
  pub const fn new(index: u8) -> Self {
    assert!(index < 32);

    Self { index }
  }

  /// Gets the index of the [`Register`].
  #[inline(always)]
  pub fn index(&self) -> u8 {
    self.index
  }
}

// General-purpose registers (64-bit)
pub const X0: Register = Register::new(0);
pub const X1: Register = Register::new(1);
pub const X2: Register = Register::new(2);
pub const X3: Register = Register::new(3);
pub const X4: Register = Register::new(4);
pub const X5: Register = Register::new(5);
pub const X6: Register = Register::new(6);
pub const X7: Register = Register::new(7);
pub const X8: Register = Register::new(8);
pub const X9: Register = Register::new(9);
pub const X10: Register = Register::new(10);
pub const X11: Register = Register::new(11);
pub const X12: Register = Register::new(12);
pub const X13: Register = Register::new(13);
pub const X14: Register = Register::new(14);
pub const X15: Register = Register::new(15);
pub const X16: Register = Register::new(16);
pub const X17: Register = Register::new(17);
pub const X18: Register = Register::new(18);
pub const X19: Register = Register::new(19);
pub const X20: Register = Register::new(20);
pub const X21: Register = Register::new(21);
pub const X22: Register = Register::new(22);
pub const X23: Register = Register::new(23);
pub const X24: Register = Register::new(24);
pub const X25: Register = Register::new(25);
pub const X26: Register = Register::new(26);
pub const X27: Register = Register::new(27);
pub const X28: Register = Register::new(28);
pub const X29: Register = Register::new(29); // Frame pointer
pub const X30: Register = Register::new(30); // Link register
pub const XZR: Register = Register::new(31); // Zero register
pub const SP: Register = Register::new(31); // Stack pointer (same encoding as XZR)

// Special register aliases
pub const FP: Register = X29; // Frame pointer
pub const LR: Register = X30; // Link register
