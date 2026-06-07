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
use zo_codegen_backend::Target;
use zo_emitter_arm::X16;
use zo_writer_macho::{
  CODE_OFFSET, DATA_SEGMENT_INDEX, LIBSYSTEM_DYLIB_ORDINAL, MachO, PAGE_MASK,
  Simulator, TEXT_SECTION_BASE, UI_EXCLUSIVE_RUNTIME_SYMBOLS, VM_BASE,
  ZO_RUNTIME_SYMBOL_PREFIX, round_up_segment,
};

/// Which `libzo_runtime.dylib` flavor a linked binary
/// needs, derived from the runtime symbols it actually
/// imports.
///
/// The binary records one runtime `LC_LOAD_DYLIB` (a `deps/`
/// sibling on desktop, an `App.app/Frameworks/` entry on
/// iOS); only the file the compiler stages there differs by
/// flavor. The full dylib is a superset of the lean core, so binding
/// resolution is unaffected by the choice — a program that
/// needs only lean symbols runs against either, but staging
/// the 1.3 MB lean core avoids cold-loading the 9.9 MB GPU
/// / webview / image / TLS tree.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RuntimeKind {
  /// No `_zo_*` runtime symbol imported — stage nothing.
  None,
  /// Every imported runtime symbol is in the lean core —
  /// stage `libzo_runtime_core.dylib`.
  Lean,
  /// At least one [`UI_EXCLUSIVE_RUNTIME_SYMBOLS`] import —
  /// stage the full `libzo_runtime_ui.dylib`.
  Full,
}

impl RuntimeKind {
  /// Strongest flavor of two: `Full` dominates `Lean`,
  /// which dominates `None`. One UI-exclusive import in a
  /// program forces the full dylib for the whole binary.
  fn max_with(self, other: RuntimeKind) -> RuntimeKind {
    match (self, other) {
      (RuntimeKind::Full, _) | (_, RuntimeKind::Full) => RuntimeKind::Full,
      (RuntimeKind::Lean, _) | (_, RuntimeKind::Lean) => RuntimeKind::Lean,
      _ => RuntimeKind::None,
    }
  }
}

/// Classify one imported runtime symbol: `Full` iff it is
/// exported only by the UI runtime, else `Lean`. Caller has
/// already established `c_sym` is a runtime symbol.
fn classify(c_sym: &str) -> RuntimeKind {
  if UI_EXCLUSIVE_RUNTIME_SYMBOLS.contains(&c_sym) {
    RuntimeKind::Full
  } else {
    RuntimeKind::Lean
  }
}

/// Result of assembling a mach-o executable: the bytes
/// ready to write, plus the runtime flavor the compiler
/// must stage next to the binary.
pub struct LinkOutput {
  /// Final executable bytes, signed and ready for disk.
  pub executable: Vec<u8>,
  /// Runtime dylib the binary's `LC_LOAD_DYLIB` resolves
  /// against at load time.
  pub runtime: RuntimeKind,
}

/// Assemble a mach-o executable from the codegen's
/// `LinkObject`. Returns the executable bytes plus the
/// runtime flavor to stage — see [`LinkOutput`].
pub fn link_macho(link_obj: MachoLinkObject, target: Target) -> LinkOutput {
  let mut macho = MachO::new();
  let mut code = link_obj.code;

  // iOS embeds the runtime in `App.app/Frameworks/` next to the
  // flat `App.app/<binary>`; desktop stages it in a sibling
  // `deps/`. Both resolve relative to the binary's own directory,
  // so the only difference is the subdirectory.
  let is_ios =
    matches!(target, Target::Arm64AppleIos | Target::Arm64AppleIosSim);
  let runtime_subdir = if is_ios {
    "@executable_path/Frameworks"
  } else {
    "@loader_path/deps"
  };
  let runtime_load_path = format!("{runtime_subdir}/libzo_runtime.dylib");

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
  // constants misalign as soon as one optional dylib is
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
  //
  // A `#link` path is keyed by its FINAL load-command form
  // (`@executable_path/<name>` rewrites to
  // `@loader_path/deps/<name>`; absolute system paths pass
  // through). Deduping on that form collapses a `#link`
  // reference to `libzo_runtime.dylib` into the single
  // runtime entry: `core/io.zo` declares such a `#link`, so
  // a program using both buffered I/O and any `_zo_*` symbol
  // would otherwise emit two identical `LC_LOAD_DYLIB`s.
  let final_load_path = |path: &str| -> String {
    if let Some(name) = path.strip_prefix("@executable_path/") {
      format!("{runtime_subdir}/{name}")
    } else {
      path.to_owned()
    }
  };

  let mut needs_runtime_dylib = false;
  let mut runtime_kind = RuntimeKind::None;
  let mut link_paths: Vec<String> = Vec::new();
  let mut seen_path: HashSet<String> = HashSet::default();

  for c_sym in &link_obj.extern_used {
    if let Some(path) = link_obj.extern_dylib_paths.get(c_sym) {
      let resolved = final_load_path(path);

      // A `#link` symbol that resolves to the runtime path
      // is a runtime symbol — fold it into the single
      // runtime entry rather than a parallel ordinal.
      if resolved == runtime_load_path {
        needs_runtime_dylib = true;
        runtime_kind = runtime_kind.max_with(classify(c_sym));
      } else if seen_path.insert(resolved.clone()) {
        link_paths.push(resolved);
      }
    } else if c_sym.starts_with(ZO_RUNTIME_SYMBOL_PREFIX) {
      needs_runtime_dylib = true;
      runtime_kind = runtime_kind.max_with(classify(c_sym));
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

  // The runtime path is reached two ways — a bare `_zo_*`
  // symbol or a `#link` entry that rewrites to it — and
  // both must bind to the one runtime ordinal, never a
  // `path_ord` slot. Resolve `#link`-routed symbols against
  // their final path first, falling back to the runtime
  // ordinal when that path IS the runtime.
  let ordinal_for = |c_sym: &str| -> u8 {
    if let Some(path) = link_obj.extern_dylib_paths.get(c_sym) {
      let resolved = final_load_path(path);

      if let Some(ord) = path_ord.get(&resolved) {
        *ord
      } else {
        runtime_ord.expect("runtime-routed link symbol with no runtime dylib")
      }
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
    macho.add_dylib(&runtime_load_path);
  }

  // Each `#link { macos: ... }` path declared by a `pack`
  // referenced through `pub ffi` lands here in the same
  // order ordinals were assigned. `link_paths` already
  // holds the FINAL load form (`@executable_path/<name>`
  // rewritten to `@loader_path/deps/<name>`) and is already
  // deduped against the runtime entry above, so this loop
  // adds dylibs verbatim — the add order matches the
  // ordinal assignment 1:1.
  for path in &link_paths {
    macho.add_dylib(path);
  }

  macho.add_uuid();

  if is_ios {
    let simulator = if matches!(target, Target::Arm64AppleIosSim) {
      Simulator::Yes
    } else {
      Simulator::No
    };

    macho.add_build_version_ios(simulator);
  } else {
    macho.add_build_version();
  }

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

  LinkOutput {
    executable: macho.finish_with_signature(),
    runtime: runtime_kind,
  }
}
