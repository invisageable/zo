//! Unit tests for the x86-64 emitter — byte-level assertions
//! against encodings re-derived from the Intel SDM so a drift
//! in any opcode, REX prefix, or ModRM byte fails loudly here
//! rather than downstream in codegen.

use crate::{COND_NE, R8, RAX, RCX, RBP, X64Emitter};

#[test]
fn ret_is_single_byte_c3() {
  let mut emitter = X64Emitter::new();

  emitter.emit_ret();

  assert_eq!(emitter.code(), vec![0xC3]);
}

#[test]
fn push_pop_rbp_need_no_rex() {
  let mut emitter = X64Emitter::new();

  emitter.emit_push_reg(RBP);
  emitter.emit_pop_reg(RBP);

  // push rbp = 0x55, pop rbp = 0x5D.
  assert_eq!(emitter.code(), vec![0x55, 0x5D]);
}

#[test]
fn push_extended_reg_takes_rex_b() {
  let mut emitter = X64Emitter::new();

  emitter.emit_push_reg(R8);

  // push r8 = REX.B (0x41) + 0x50.
  assert_eq!(emitter.code(), vec![0x41, 0x50]);
}

#[test]
fn mov_rax_rcx_is_rexw_89_c8() {
  let mut emitter = X64Emitter::new();

  // mov rax, rcx : dst=rax(rm), src=rcx(reg).
  emitter.emit_mov_reg_reg(RAX, RCX);

  assert_eq!(emitter.code(), vec![0x48, 0x89, 0xC8]);
}

#[test]
fn jmp_rel32_back_patches_to_target() {
  let mut emitter = X64Emitter::new();

  let site = emitter.emit_jmp();

  // Target the byte right after the 5-byte JMP: rel = 0.
  emitter.patch_rel32(site, 5);

  assert_eq!(emitter.code(), vec![0xE9, 0x00, 0x00, 0x00, 0x00]);
}

#[test]
fn jcc_rel32_encodes_two_byte_escape() {
  let mut emitter = X64Emitter::new();

  let site = emitter.emit_jcc(COND_NE);

  emitter.patch_rel32(site, 6);

  // jne rel32 = 0x0F 0x85 + rel(0) to the next instruction.
  assert_eq!(emitter.code(), vec![0x0F, 0x85, 0x00, 0x00, 0x00, 0x00]);
}
