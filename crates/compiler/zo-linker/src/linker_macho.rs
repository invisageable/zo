//! Mach-O linker for the ARM64 backend.
//!
//! Consumes a [`MachoLinkObject`] produced by `ARM64Gen` —
//! raw machine code plus symbol / fixup tables — and emits a
//! self-contained executable. The work split is:
//!
//! 1. Lay out the GOT in `__DATA`, one slot per extern C
//!    symbol the program references (libm, libSystem,
//!    libzo_runtime).
//! 2. Patch each extern stub's `ADRP X16; LDR X16, [X16,#off]`
//!    pair to point at its GOT slot.
//! 3. Build the dyld bind opcodes, routing each symbol to
//!    its owning dylib ordinal (libSystem vs libzo_runtime).
//! 4. Assemble the segments (`__TEXT`, `__DATA`,
//!    `__LINKEDIT`), write the symbol table, finalize with a
//!    code signature.
//!
//! The body matches the previous `ARM64Gen::generate_macho`
//! 1:1 — only the input shape changed (fields read off
//! `MachoLinkObject` instead of `&mut self`). Constants
//! (`TEXT_SECTION_BASE`, `PAGE_MASK`, dylib ordinals, ...)
//! moved to `zo-writer-macho` so this crate and the
//! ARM emitter tests can share them.

use zo_codegen_backend::MachoLinkObject;
use zo_emitter_arm::X16;
use zo_writer_macho::{
  CODE_OFFSET, DATA_SEGMENT_INDEX, DATA_VM_ADDR, LIBSYSTEM_DYLIB_ORDINAL,
  MachO, PAGE_MASK, TEXT_SECTION_BASE, ZO_RUNTIME_DYLIB_ORDINAL,
  ZO_RUNTIME_SYMBOL_PREFIX,
};

/// Assemble a mach-o executable from the codegen's
/// `LinkObject`. Returns the executable bytes ready to be
/// written to disk.
pub fn link_macho(link_obj: MachoLinkObject) -> Vec<u8> {
  let mut macho = MachO::new();
  let mut code = link_obj.code;

  // --- Libm GOT + stub patching ---
  // Each libm function gets one 8-byte GOT slot in __DATA
  // and one 12-byte stub in __TEXT. The stub does:
  //   ADRP X16, got_page
  //   LDR  X16, [X16, #got_page_off]
  //   BR   X16
  // dyld fills the GOT slot at load time via bind opcodes.
  let n_got = link_obj.extern_used.len();
  let mut got_data = Vec::with_capacity(n_got * 8);
  let mut bind_entries: Vec<(&str, u8, u64, u8)> = Vec::new();
  // Track whether any symbol needs the zo runtime
  // dylib so we only register it when a program
  // actually uses concurrency.
  let mut needs_runtime_dylib = false;

  for (i, c_sym) in link_obj.extern_used.iter().enumerate() {
    let got_offset_in_data = (i * 8) as u64;
    let got_vm_addr = DATA_VM_ADDR + got_offset_in_data;

    // Populate GOT slot with zero (dyld overwrites).
    got_data.extend_from_slice(&[0u8; 8]);

    // Patch the stub: ADRP X16, page_diff; LDR X16,
    // [X16, #page_off]; BR X16.
    if let Some(&stub_off) = link_obj.extern_stub_offsets.get(c_sym) {
      let stub_vm = TEXT_SECTION_BASE + stub_off as u64;
      let stub_page = stub_vm & !PAGE_MASK;
      let got_page = got_vm_addr & !PAGE_MASK;
      let page_diff = ((got_page as i64 - stub_page as i64) >> 12) as i32;
      let page_off = (got_vm_addr & PAGE_MASK) as u32;

      // ADRP X16, page_diff
      let immlo = (page_diff as u32) & 0x3;
      let immhi = ((page_diff >> 2) as u32) & 0x7FFFF;
      let adrp =
        0x90000000u32 | (immlo << 29) | (immhi << 5) | (X16.index() as u32);

      // LDR X16, [X16, #page_off]
      // Unsigned offset: imm12 = page_off / 8
      let imm12 = (page_off >> 3) & 0xFFF;
      let ldr = 0xF9400000u32
        | (imm12 << 10)
        | ((X16.index() as u32) << 5)
        | (X16.index() as u32);

      let pos = stub_off as usize;

      code[pos..pos + 4].copy_from_slice(&adrp.to_le_bytes());
      code[pos + 4..pos + 8].copy_from_slice(&ldr.to_le_bytes());
      // BR X16 is already correct from emit_br().
    }

    // Route each symbol to the right LC_LOAD_DYLIB.
    // Runtime symbols (`_zo_chan_*` / `_zo_task_*`)
    // land in libzo_runtime.dylib; everything else
    // (libm, libSystem) stays on libSystem.
    let ordinal = if c_sym.starts_with(ZO_RUNTIME_SYMBOL_PREFIX) {
      needs_runtime_dylib = true;

      ZO_RUNTIME_DYLIB_ORDINAL
    } else {
      LIBSYSTEM_DYLIB_ORDINAL
    };

    // segment 2 = __DATA (pagezero=0, __TEXT=1, __DATA=2)
    bind_entries.push((c_sym, DATA_SEGMENT_INDEX, got_offset_in_data, ordinal));
  }

  // Build bind opcodes for dyld. Per-entry ordinal lets
  // libSystem and libzo_runtime symbols share one
  // opcode stream.
  if !bind_entries.is_empty() {
    let bind_data = MachO::build_bind_opcodes(&bind_entries);

    macho.set_bind_data(bind_data);
  }

  macho.add_code(code);
  macho.add_data(got_data);

  macho.add_pagezero_segment();
  macho.add_text_segment();
  macho.add_data_segment();

  if let Some(offset) = link_obj.main_offset {
    macho.add_function_symbol(
      "_main",
      1,
      TEXT_SECTION_BASE + offset as u64,
      false,
    );
  }

  if let Some(offset) = link_obj.ui_entry_offset {
    macho.add_function_symbol(
      "_zo_ui_entry_point",
      1,
      TEXT_SECTION_BASE + offset as u64,
      true,
    );
  }

  // Add undefined symbols, routing each to its owning
  // dylib's ordinal so the Mach-O symtab + LC_LOAD_DYLIB
  // entries agree with the bind opcodes above.
  for c_sym in &link_obj.extern_used {
    let ordinal = if c_sym.starts_with(ZO_RUNTIME_SYMBOL_PREFIX) {
      ZO_RUNTIME_DYLIB_ORDINAL
    } else {
      LIBSYSTEM_DYLIB_ORDINAL
    };

    macho.add_undefined_symbol(c_sym, ordinal as u16);
  }

  macho.add_dylinker();
  macho.add_dylib("/usr/lib/libSystem.B.dylib");

  // Register libzo_runtime.dylib as the second
  // LC_LOAD_DYLIB so `_zo_chan_*` / `_zo_task_*` resolve
  // at load time. Users must colocate the dylib with the
  // executable (or point DYLD_LIBRARY_PATH at it) for
  // programs that use concurrency to launch. Non-
  // concurrency programs never touch this entry.
  if needs_runtime_dylib {
    macho.add_dylib("@executable_path/libzo_runtime.dylib");
  }

  macho.add_uuid();
  macho.add_build_version();
  macho.add_source_version();

  // Entry point must point to the actual main function,
  // not always 0x400 (which is only correct when main
  // is the first function in the code section).
  let main_entry = link_obj
    .main_offset
    .map(|off| CODE_OFFSET as u64 + off as u64)
    .unwrap_or(CODE_OFFSET as u64);

  macho.add_main(main_entry);

  macho.add_dyld_info();
  macho.finish_with_signature()
}
