//! Codegen output ready to be linked into an executable.
//!
//! `LinkObject` is the data hand-off between the codegen
//! and linker phases. Each backend produces the variant
//! that matches its target's link model — ARM emits a
//! mach-o-bound intermediate (raw code + symbol/fixup
//! tables); CLIF emits a relocatable object file ready
//! for `cc`.
//!
//! The linker consumes a `LinkObject` and writes an
//! executable file. No backend state crosses the phase
//! boundary — every field the linker needs is materialized
//! into the `LinkObject` at the codegen-phase boundary.

use rustc_hash::FxHashMap;

use zo_interner::Symbol;

/// Codegen output, input to the linker.
///
/// `MachoLinkObject` is boxed so the enum stays compact —
/// the mach-o variant carries ~280 bytes of fixup tables
/// while the CLIF variant is just an object-file `Vec<u8>`.
pub enum LinkObject {
  /// ARM mach-o intermediate: raw machine code plus the
  /// symbol / fixup tables the mach-o assembler needs.
  Macho(Box<MachoLinkObject>),
  /// CLIF relocatable object file, ready for `cc`.
  Object(Vec<u8>),
}

/// State extracted from `ARM64Gen` at the end of codegen,
/// carrying everything the mach-o linker needs to assemble
/// the final executable. Fields mirror the previous private
/// state on `ARM64Gen` — see `zo-linker::linker_macho`
/// for how each is consumed.
pub struct MachoLinkObject {
  /// Raw machine code bytes from the emitter, with stub
  /// regions and string/template trailing data already
  /// concatenated.
  pub code: Vec<u8>,
  /// Function `Symbol` → byte offset within `code`.
  pub functions: FxHashMap<Symbol, u32>,
  /// Per-string symbol blob, in registration order. The
  /// linker emits these into the rodata section.
  pub string_data: Vec<(Symbol, Vec<u8>)>,
  /// Code offsets that load a string address — patched at
  /// link time to point into the rodata section.
  pub string_fixups: Vec<(u32, Symbol)>,
  /// Code offsets that load a function pointer — patched
  /// at link time to point at the function's TEXT offset.
  pub function_addr_fixups: Vec<(u32, Symbol)>,
  /// Per-template blob, same shape as `string_data`.
  pub template_data: Vec<(Symbol, Vec<u8>)>,
  /// True iff the program emitted at least one template;
  /// gates the synthetic `_zo_ui_entry_point` symbol.
  pub has_templates: bool,
  /// External C symbols this program references (libm,
  /// libSystem, libzo_runtime). Order is the GOT layout.
  pub extern_used: Vec<String>,
  /// Extern C symbol → stub code offset within `code`.
  /// The stub is the `ADRP X16; LDR X16, [X16,#off]; BR X16`
  /// trampoline patched in by the linker once the GOT
  /// layout is known.
  pub extern_stub_offsets: FxHashMap<String, u32>,
  /// Code offsets that branch to an extern stub — patched
  /// after the stubs are placed.
  pub extern_fixups: Vec<(u32, String)>,
  /// Code offsets that branch to a user function — patched
  /// after every function offset is known.
  pub call_fixups: Vec<(u32, Symbol)>,
  /// Byte offset of the `main` function within `code`.
  /// `None` for libraries / programs without a main entry.
  /// Resolved here so the linker doesn't need an interner
  /// handle to look up the `"main"` symbol.
  pub main_offset: Option<u32>,
  /// Byte offset of the synthetic `_zo_ui_entry_point`
  /// function within `code`, when the program emitted
  /// templates. `None` for non-template programs.
  pub ui_entry_offset: Option<u32>,
}
