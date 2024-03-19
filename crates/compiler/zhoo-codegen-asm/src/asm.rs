use zo_core::impl_const_instance;

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct Asm {
  insts: Vec<Inst>,
}

// arm registers.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Reg {
  // general purpose.
  Rax,
  Rbx,
  Rcx,
  Rdx,
  Rsi,
  Rdi,
  R8,
  R9,
  R10,
  R11,
  R12,
  R13,
  R14,
  R15,

  // stack management.
  Rsp,
  Rbp,

  // left shift
  Cl,
}

// arm flags.
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

// instructions.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct Inst {
  pub kind: InstKind,
  // span: Span,
}

impl Inst {
  pub fn of(kind: InstKind) -> Self {
    Self { kind }
  }

  impl_const_instance! {
    mov   InstKind::Mov,
    mvn   InstKind::Mvn,
    add   InstKind::Add,
    sub   InstKind::Sub,
    mul   InstKind::Mul,
    lsl   InstKind::Lsl,
    lsr   InstKind::Lsr,
    asr   InstKind::Asr,
    ror   InstKind::Ror,
    cmp   InstKind::Cmp,
    and   InstKind::And,
    orr   InstKind::Orr,
    eor   InstKind::Eor,
    ldr   InstKind::Ldr,
    str   InstKind::Str,
    ldm   InstKind::Ldm,
    stm   InstKind::Stm,
    push  InstKind::Push,
    pop   InstKind::Pop,
    b     InstKind::B,
    bl    InstKind::Bl,
    bx    InstKind::Bx,
    blx   InstKind::Blx,
    swi   InstKind::Swi,
    svc   InstKind::Svc,
  }
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum InstKind {
  Mov,  // move data.
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
