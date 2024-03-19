use super::asm;

impl std::fmt::Display for asm::Asm {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    writeln!(
      f,
      "{}",
      ".section .text \
      .global _start  \
      \
      _start:         \
    "
    )?;

    writeln!(f, "\tsyscall")?;

    Ok(())
  }
}

impl std::fmt::Display for asm::Reg {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      asm::Reg::Rax => write!(f, "rax"),
      asm::Reg::Rbx => write!(f, "rbx"),
      asm::Reg::Rcx => write!(f, "rcx"),
      asm::Reg::Rdx => write!(f, "rdx"),
      asm::Reg::Rsi => write!(f, "rsi"),
      asm::Reg::Rdi => write!(f, "rdi"),
      asm::Reg::R8 => write!(f, "r8"),
      asm::Reg::R9 => write!(f, "r9"),
      asm::Reg::R10 => write!(f, "r10"),
      asm::Reg::R11 => write!(f, "r11"),
      asm::Reg::R12 => write!(f, "r12"),
      asm::Reg::R13 => write!(f, "r13"),
      asm::Reg::R14 => write!(f, "r14"),
      asm::Reg::R15 => write!(f, "r15"),
      asm::Reg::Rsp => write!(f, "rsp"),
      asm::Reg::Rbp => write!(f, "rbp"),
      asm::Reg::Cl => write!(f, "cl"),
    }
  }
}

impl std::fmt::Display for asm::Inst {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", self.kind)
  }
}

impl std::fmt::Display for asm::InstKind {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      asm::InstKind::Mov => write!(f, "MOV"),
      asm::InstKind::Mvn => write!(f, "MVN"),
      asm::InstKind::Add => write!(f, "ADD"),
      asm::InstKind::Sub => write!(f, "SUB"),
      asm::InstKind::Mul => write!(f, "MUL"),
      asm::InstKind::Lsl => write!(f, "LSL"),
      asm::InstKind::Lsr => write!(f, "LSR"),
      asm::InstKind::Asr => write!(f, "ASR"),
      asm::InstKind::Ror => write!(f, "ROR"),
      asm::InstKind::Cmp => write!(f, "CMP"),
      asm::InstKind::And => write!(f, "AND"),
      asm::InstKind::Orr => write!(f, "ORR"),
      asm::InstKind::Eor => write!(f, "EOR"),
      asm::InstKind::Ldr => write!(f, "LDR"),
      asm::InstKind::Str => write!(f, "STR"),
      asm::InstKind::Ldm => write!(f, "LDM"),
      asm::InstKind::Stm => write!(f, "STM"),
      asm::InstKind::Push => write!(f, "PUSH"),
      asm::InstKind::Pop => write!(f, "POP"),
      asm::InstKind::B => write!(f, "B"),
      asm::InstKind::Bl => write!(f, "BL"),
      asm::InstKind::Bx => write!(f, "BX"),
      asm::InstKind::Blx => write!(f, "BLX"),
      asm::InstKind::Swi => write!(f, "SWI"),
      asm::InstKind::Svc => write!(f, "SVC"),
    }
  }
}
