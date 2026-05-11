//! Mach-O linker for the ARM64 backend.
//!
//! Consumes a [`MachoLinkObject`] produced by `ARM64Gen` —
//! raw machine code plus symbol / fixup tables — and emits
//! a self-contained executable:
//!
//! 1. Lay out the GOT in `__DATA`, one slot per extern C
//!    symbol the program references.
//! 2. Patch each extern stub's `ADRP X16; LDR X16, [X16,#off]`
//!    pair to point at its GOT slot.
//! 3. Build the dyld bind opcodes, routing each symbol to
//!    its owning dylib ordinal (libSystem / libzo_runtime /
//!    libraylib / libzo_misato).
//! 4. Assemble `__TEXT` / `__DATA` / `__LINKEDIT`, write
//!    the symbol table, finalize with a code signature.

use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

use zo_codegen_backend::MachoLinkObject;
use zo_emitter_arm::X16;
use zo_writer_macho::{
  CODE_OFFSET, DATA_SEGMENT_INDEX, LIBSYSTEM_DYLIB_ORDINAL, MachO, PAGE_MASK,
  TEXT_SECTION_BASE, VM_BASE, ZO_RUNTIME_SYMBOL_PREFIX, round_up_segment,
};

/// Assemble a mach-o executable from the codegen's
/// `LinkObject`. Returns the executable bytes ready to be
/// written to disk.
pub fn link_macho(link_obj: MachoLinkObject) -> Vec<u8> {
  let mut macho = MachO::new();
  let mut code = link_obj.code;

  // The mach-o `__DATA` segment starts immediately after
  // the page-rounded `__TEXT` segment. We compute the
  // text segment size up front from the final code length
  // so the stub-patching loop below can compute correct
  // VM addresses for the GOT slots — without this, the
  // patcher would need a back-reference into `MachO`'s
  // internal layout state.
  let text_segment_size = round_up_segment(CODE_OFFSET + code.len() as u32);
  let data_vm_addr = VM_BASE + text_segment_size as u64;

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

  // Dyld treats ordinals as the 1-based index into the
  // binary's LC_LOAD_DYLIB sequence — a binary with 3
  // dylibs can only reference ordinals 1..=3. Hardcoded
  // constants mis-align as soon as one optional dylib is
  // absent (later slots slide forward), surfacing as
  // `dyld: unknown library ordinal N`. Pre-scan first,
  // then assign ordinals positionally below.
  //
  // Routing is `#link`-driven: codegen built
  // `extern_dylib_paths` (c_sym → resolved dylib path)
  // from per-pack `#link { macos: ..., linux: ..., }`
  // metadata. Each unique path gets its own ordinal slot
  // here, in first-seen order. Symbols with no `#link`
  // entry fall through to `libzo_runtime.dylib` (zo's own
  // runtime symbols) or libSystem (libc / libm).
  let mut needs_runtime_dylib = false;
  let mut link_paths: Vec<String> = Vec::new();
  let mut seen_path: HashSet<String> = HashSet::default();

  for c_sym in &link_obj.extern_used {
    if let Some(path) = link_obj.extern_dylib_paths.get(c_sym) {
      if seen_path.insert(path.clone()) {
        link_paths.push(path.clone());
      }
    } else if c_sym.starts_with(ZO_RUNTIME_SYMBOL_PREFIX) {
      needs_runtime_dylib = true;
    }
  }

  let libsystem_ord = LIBSYSTEM_DYLIB_ORDINAL;
  let mut next_ordinal: u8 = libsystem_ord + 1;
  let runtime_ord = if needs_runtime_dylib {
    let o = next_ordinal;
    next_ordinal += 1;
    Some(o)
  } else {
    None
  };

  let mut path_ord: HashMap<String, u8> = HashMap::default();

  for path in &link_paths {
    path_ord.insert(path.clone(), next_ordinal);
    next_ordinal += 1;
  }

  let ordinal_for = |c_sym: &str| -> u8 {
    if let Some(path) = link_obj.extern_dylib_paths.get(c_sym) {
      *path_ord
        .get(path)
        .expect("link path missing from ordinal table")
    } else if c_sym.starts_with(ZO_RUNTIME_SYMBOL_PREFIX) {
      runtime_ord.expect("runtime symbol with no runtime dylib")
    } else {
      libsystem_ord
    }
  };

  for (i, c_sym) in link_obj.extern_used.iter().enumerate() {
    let got_offset_in_data = (i * 8) as u64;
    let got_vm_addr = data_vm_addr + got_offset_in_data;

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

    bind_entries.push((
      c_sym,
      DATA_SEGMENT_INDEX,
      got_offset_in_data,
      ordinal_for(c_sym),
    ));
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

  // The symtab ordinals must agree with the bind opcodes
  // above — same `ordinal_for` mapping reused.
  for c_sym in &link_obj.extern_used {
    macho.add_undefined_symbol(c_sym, ordinal_for(c_sym) as u16);
  }

  // Dylib add order MUST match the ordinal assignment
  // above (libSystem → runtime → each `#link` path in
  // first-seen order), because dyld's bind ordinals are
  // the 1-based index into this sequence.
  macho.add_dylinker();
  macho.add_dylib("/usr/lib/libSystem.B.dylib");

  if needs_runtime_dylib {
    macho.add_dylib("@executable_path/libzo_runtime.dylib");
  }

  // Each `#link { macos: ... }` path declared by a `pack`
  // referenced through `pub ffi` lands here in the same
  // order ordinals were assigned. The compiler's
  // `stage_runtime_artifacts` step copies any
  // `@executable_path/...` dylib next to the user binary.
  for path in &link_paths {
    macho.add_dylib(path);
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
