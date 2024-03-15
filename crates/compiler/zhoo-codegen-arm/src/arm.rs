#![allow(dead_code)]
#![allow(unused_variables)]

use zo_core::impl_const_instance;

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct Arm {
  emitter: Emitter,
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct Emitter;

/// arm registers.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Reg {
  // general purpose.
  R0,
  R1,
  R2,
  R3,
  R4,
  R5,
  R6,
  R8,
  R9,
  R10,

  // frame pointer.
  R11,

  // holds syscall number.
  R7,

  // special purpose.
  R12,  // intra procedure call.
  R13,  // stack pointer.
  R14,  // link register.
  R15,  // program counter.
  Cpsr, // current program status register.
}

/// arm flags.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Flag {
  N, // negative.
  Z, // zero.
  C, // carry.
  V, // overflow.
  E, // endian-bit.
  T, // thumb-bit.
  M, // mode-bits.
  J, // jazelle.
}

/// instruction.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct Inst {
  pub kind: InstKind,
}

impl Inst {
  pub fn of(kind: InstKind) -> Self {
    Self { kind }
  }

  impl_const_instance! {
    mov InstKind::Mov,
    mvn InstKind::Mvn,
    add InstKind::Add,
  }
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum InstKind {
  Mov,  // move data — Mov(Value, Value).
  Mvn,  // move and negate.
  Add,  // addition.
  Sub,  // substraction.
  Mul,  // multiplication.
  Lsl,  // logical shift left.
  Lsr,  // logical shift right.
  Asr,  // arithmetic shift right.
  Ror,  // rotate right.
  Cmp,  // compare.
  And,  // bitwize and.
  Orr,  // bitwize or.
  Eor,  // bitwize xor.
  Ldr,  // load.
  Str,  // store.
  Ldm,  // load multiple.
  Stm,  // store multiple.
  Push, // push on Stack.
  Pop,  // pop on Stack.
  B,    // branch.
  Bl,   // branch with link.
  Bx,   // branch and exchange.
  Blx,  // branch with link and exchange.
  Swi,  // system call.
  Svc,  // system call.
}

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn link(o_name: &str, out_name: &str) -> std::io::Result<Option<i32>> {
  let o_file = std::fs::File::open(o_name)?;

  Ok(
    std::process::Command::new("ld")
      .arg("-o")
      .arg(out_name)
      .arg("/dev/stdin")
      .stdin(o_file)
      .spawn()?
      .wait()?
      .code(),
  )
}
