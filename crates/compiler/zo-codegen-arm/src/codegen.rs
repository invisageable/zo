pub(crate) mod template;

use zo_buffer::Buffer;
use zo_codegen_backend::Artifact;
use zo_emitter_arm::{
  ARM64Emitter, COND_CC, COND_CS, COND_EQ, COND_GE, COND_GT, COND_HI, COND_LE,
  COND_LS, COND_LT, COND_NE, COND_VC, COND_VS, D0, D1, FpRegister, Register,
  SP, X0, X1, X2, X3, X9, X16, X17, X29, X30, XZR,
};
use zo_interner::{Interner, Symbol};
use zo_register_allocation::{EmitTiming, RegAlloc, RegisterClass, SpillKind};
use zo_sir::{BinOp, Insn, LoadSource, Sir, SpawnKind, UnOp};
use zo_ty::TyId;
use zo_value::ValueId;
use zo_writer_macho::{DATA_VM_ADDR, DebugFrameEntry, MachO};

use rustc_hash::FxHashMap as HashMap;

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

// --- macOS ARM64 System Calls ---
const SYS_EXIT: u16 = 1;
const SYS_READ: u16 = 3;
const SYS_WRITE: u16 = 4;
const SYS_OPEN: u16 = 5;
const SYS_CLOSE: u16 = 6;
const SYS_ACCESS: u16 = 33;
const FD_STDOUT: u16 = 1;
const FD_STDERR: u16 = 2;
const O_READ_ONLY: u16 = 0;
const O_WRITE_ONLY_CREATE_TRUNCATE: u16 = 0x601;
const O_WRITE_ONLY_CREATE_APPEND: u16 = 0x209;
const FILE_MODE_644: u16 = 0o644;
const READ_FILE_BUF_SIZE: u16 = 4096;

// --- ASCII Constants ---
const ASCII_NEWLINE: u16 = 10;
const ASCII_ZERO: u16 = 48;

// --- Stack Frame Layout ---
const STACK_SLOT_SIZE: u32 = 8;
const FP_LR_SAVE_OFFSET: i16 = -16;
const FP_LR_LOAD_OFFSET: i16 = 16;
// 7 caller-saved temp regs (X9-X15) * 8 bytes each.
const CALLER_SAVE_RESERVE: u32 = 56;
const CALLER_SAVE_COUNT: usize = 7;
const CALLER_SAVE_START: u8 = 9; // X9..X17
const FRAME_ALIGN_MASK: u32 = 15;
const MAX_REG_ARGS: usize = 8;

// --- Buffer Sizes ---
const ITOA_BUFFER_SIZE: u16 = 32;
const ITOA_BUFFER_END: u16 = 31;
const NEWLINE_BUFFER_OFFSET: u16 = 16;

// --- Array Layout ---
const ARRAY_ELEMENT_SHIFT: u8 = 3;
const ARRAY_HEADER_SIZE: u16 = 16; // [len:8][cap:8]
// --- Type Detection ---
// These TyIds must match TyChecker::new() registration
// order. If that order ever changes, these break silently.
// str=4 is hardcoded inline in is_string_value/emit_field_write.
const BOOL_TYPE_ID: u32 = 2; // TyChecker: Bool @ index 2
const CHAR_TYPE_ID: u32 = 3; // TyChecker: Char @ index 3
const STR_TYPE_ID: u32 = 4; // TyChecker: Str @ index 4
const FLOAT_TYPE_ID_MIN: u32 = 15; // TyChecker: F32 @ index 15
const FLOAT_TYPE_ID_MAX: u32 = 17; // TyChecker: F64 @ index 17

// --- Mach-O Layout ---
const TEXT_SECTION_BASE: u64 = 0x100000400;
const CODE_OFFSET: u64 = 0x400;
pub(super) const UI_ENTRY_SYMBOL: u32 = 0xFFFF;
pub(super) const TEMPLATE_SYMBOL_OFFSET: u32 = 0x1000;

// --- Branch Fixup Masks ---
const BL_OPCODE: u32 = 0x94000000;
const B_FIXUP_MASK: u32 = 0xFC000000;
const B_FIXUP_OPCODE: u32 = 0x14000000;
const CBZ_FIXUP_MASK: u32 = 0x7E000000;
const CBZ_FIXUP_OPCODE: u32 = 0x34000000;
const INSN_RD_MASK: u32 = 0x1F;
const FIXUP_IMM26_MASK: u32 = 0x3FFFFFF;
const FIXUP_IMM19_MASK: u32 = 0x7FFFF;
const FIXUP_ADR: u32 = 0x10000000;
const FIXUP_ADR_IMMLO: u32 = 0x3;
const FIXUP_ADR_IMMHI: u32 = 0x7FFFF;

// --- Template Layout ---
pub(super) const TEMPLATE_HEADER_SIZE: usize = 8;
pub(super) const TEMPLATE_CMD_SIZE: usize = 16;

// --- Hello World ---
const HELLO_STR_OFFSET: i32 = 0x18;
const HELLO_STR_LEN: u16 = 14;
const CFA_FP_REG: u8 = 31;

// --- Page Layout ---
const PAGE_MASK: u64 = 0xFFF;

// --- Dynamic Linking ---
const LIBSYSTEM_DYLIB_ORDINAL: u8 = 1;
/// libzo_runtime.dylib is registered as the second
/// `LC_LOAD_DYLIB`, so its ordinal is 2. `_zo_chan_*`
/// / `_zo_task_*` bindings route here; everything
/// else stays on libSystem.
const ZO_RUNTIME_DYLIB_ORDINAL: u8 = 2;

/// Prefix identifying symbols that the runtime dylib
/// exports. Classifier for routing extern symbols to the
/// right `LC_LOAD_DYLIB` ordinal.
const ZO_RUNTIME_SYMBOL_PREFIX: &str = "_zo_";
const DATA_SEGMENT_INDEX: u8 = 2;

// --- Libm Functions ---

/// Maps a zo function name to its C library symbol.
fn libm_c_symbol(name: &str) -> String {
  format!("_{name}")
}

/// Returns the number of float arguments a libm function
/// takes. All return a single f64 in D0.
fn libm_arg_count(name: &str) -> usize {
  match name {
    "pow" => 2,
    _ => 1,
  }
}

/// Represents the [`ARM64Gen`] code generation instance.
pub struct ARM64Gen<'a> {
  /// The [`ARM64Emitter`].
  pub(super) emitter: ARM64Emitter,
  /// String interner for resolving symbols.
  interner: &'a Interner,
  /// Function labels (name -> code offset).
  pub(super) functions: HashMap<Symbol, u32>,
  /// String data to emit at end.
  string_data: Vec<(Symbol, Vec<u8>)>,
  /// Current function context.
  current_function: Option<Symbol>,
  /// Fixups for string references (position in code -> symbol).
  pub(super) string_fixups: Vec<(u32, Symbol)>,
  /// ADR fixups that load the address of a user
  /// function — used by `TaskSpawn` to pass the
  /// callee's function pointer to the runtime's
  /// `_zo_task_spawn(callee)` ABI. Resolved in the
  /// same post-pass as string fixups, indexing
  /// `self.functions` for the callee's code offset.
  function_addr_fixups: Vec<(u32, Symbol)>,
  /// Template data sections (symbol -> data).
  pub(super) template_data: Vec<(Symbol, Vec<u8>)>,
  /// Whether we have templates that need the entry point.
  pub has_templates: bool,
  /// The label offsets: label_id → byte offset in code.
  labels: HashMap<u32, u32>,
  /// The branch fixups: (code_offset, target_label_id).
  branch_fixups: Vec<(u32, u32)>,
  /// Register allocation result.
  reg_alloc: Option<RegAlloc>,
  /// Current function's start index into SIR instructions.
  current_fn_start: Option<usize>,
  /// Mutable variable stack slots: name → offset from SP.
  mutable_slots: HashMap<u32, u32>,
  /// Parameter spill slots: param_index → offset from SP.
  param_slots: HashMap<u32, u32>,
  /// Parameter spill slots keyed by the parameter's symbol.
  /// Mirrors `param_slots` but indexed by the name the SIR
  /// uses in `Insn::Load { src: LoadSource::Local(sym) }`.
  /// Required because the executor sometimes lowers an
  /// immutable parameter read as `LoadSource::Local(sym)`
  /// (rather than `LoadSource::Param(idx)`) — without this
  /// map the codegen would emit no LDR at all, leaving the
  /// destination register holding whatever the caller left
  /// behind (e.g. a stale arg from a previous call).
  param_sym_slots: HashMap<u32, (u32, bool)>,
  /// Base offset for caller-save spill area.
  caller_save_base: u32,
  /// Next mutable variable slot.
  next_mut_slot: u32,
  /// Base offset for struct allocations in the frame.
  struct_base: u32,
  /// Offset from SP of the 16-byte channel-op scratch
  /// slot. `ChannelSend` stores the value here before
  /// the call reads it by pointer; `ChannelRecv`
  /// reserves it for the runtime's output write which
  /// is then loaded into the destination register.
  chan_scratch_base: u32,
  /// Offset from SP of the select-wait scratch area.
  /// Layout: `nchans * 8` bytes of `*mut ZoChan`
  /// pointers immediately followed by an `elem_sz`
  /// output buffer that `_zo_select_wait` writes the
  /// received value into. Sized at allocation time
  /// via `FunctionInfo::select_scratch_size`.
  select_scratch_base: u32,
  /// Next struct slot offset (relative to struct_base).
  next_struct_slot: u32,
  /// Functions that return structs: name -> field count.
  struct_return_fns: HashMap<Symbol, u32>,
  /// Set when the last emitted instruction was a math
  /// intrinsic (FSQRT, FRINT*). Result is in D0.
  last_was_math_intrinsic: bool,
  /// External C functions used (ordered, no duplicates).
  /// Each entry is the C symbol name (e.g. "_pow", "_malloc").
  extern_used: Vec<String>,
  /// Code offsets of stubs for each external function.
  /// Populated after all user code is emitted.
  extern_stub_offsets: HashMap<String, u32>,
  /// BL fixups: (code_offset, c_symbol_name).
  /// Patched in assemble() to point at stubs.
  extern_fixups: Vec<(u32, String)>,
  /// Forward-reference call fixups: (code_offset, func_name).
  /// Used when a Call references a function (e.g., closure)
  /// that appears later in the SIR stream. Patched after
  /// all instructions are translated.
  call_fixups: Vec<(u32, Symbol)>,
  /// Enum metadata keyed by `TyId.0`, populated on each
  /// `Insn::EnumDef`. Drives the pretty-printer in
  /// `emit_enum_write` so `show(Loot::Gold(50))` can produce
  /// `Loot::Gold(...)` instead of leaking a raw pointer.
  enum_metas: HashMap<u32, EnumMeta>,
  /// Counter for synthetic string symbols used by the enum
  /// pretty-printer. Starts at `ENUM_SYNTHETIC_SYM_BASE` to
  /// stay out of the interner's dynamic symbol range. Same
  /// pattern `emit_bool_and_write` already uses.
  next_enum_sym: u32,
  /// ValueId → TyId for O(1) type detection in showln.
  /// Populated during translate_insn for every value-producing
  /// instruction. Replaces the fragile find_producing_insn
  /// backward search.
  value_types: HashMap<u32, TyId>,
  /// Array type → element `TyId`, keyed by the array's
  /// `TyId.0`. Populated by the pre-pass in `generate` from
  /// `Insn::ArrayTyDef`. Drives `emit_array_write` so
  /// `showln(arr)` can dispatch each element through the
  /// element-type's writer (itoa/ftoa/bool/char/str) and
  /// produce `[e0, e1, ...]` instead of leaking a pointer.
  array_metas: HashMap<u32, TyId>,
}

/// Base for synthetic string symbols owned by the enum
/// pretty-printer. Far above `Symbol::FIRST_DYNAMIC` and the
/// interner's observed symbol ids, so it cannot collide with
/// anything the executor emits.
const ENUM_SYNTHETIC_SYM_BASE: u32 = 0xE000_0000;

/// Per-enum pretty-printer metadata. One entry per `EnumDef`
/// seen by the codegen.
struct EnumMeta {
  variants: Vec<VariantMeta>,
}

/// Per-variant pretty-printer metadata. `display_sym` owns the
/// pre-baked `"EnumName::Variant"` string in `string_data`.
/// `field_tys` carries the type of each payload field so
/// `emit_enum_write` can print actual values.
struct VariantMeta {
  discriminant: u32,
  field_tys: Vec<TyId>,
  display_sym: Symbol,
}

const ENUM_OPEN_PAREN_SYM: Symbol = Symbol(0xE000_FFFC);
const ENUM_COMMA_SPACE_SYM: Symbol = Symbol(0xE000_FFFD);
const ENUM_CLOSE_PAREN_SYM: Symbol = Symbol(0xE000_FFFE);

const ARRAY_OPEN_BRACKET_SYM: Symbol = Symbol(0xE000_FFF8);
const ARRAY_CLOSE_BRACKET_SYM: Symbol = Symbol(0xE000_FFF9);

impl<'a> ARM64Gen<'a> {
  /// Borrow the list of external symbols referenced by
  /// emitted `BL` placeholders — used by the Mach-O writer
  /// to register relocations and by tests to assert that
  /// concurrency insns (`ChannelCreate` → `_zo_chan_new`,
  /// etc.) lowered through the runtime-call path.
  pub fn extern_used(&self) -> &[String] {
    &self.extern_used
  }

  /// Creates a new [`ARM64Gen`] instance.
  pub fn new(interner: &'a Interner) -> Self {
    Self {
      emitter: ARM64Emitter::new(),
      interner,
      functions: HashMap::default(),
      string_data: Vec::new(),
      current_function: None,
      string_fixups: Vec::new(),
      function_addr_fixups: Vec::new(),
      template_data: Vec::new(),
      has_templates: false,
      labels: HashMap::default(),
      branch_fixups: Vec::new(),
      reg_alloc: None,
      current_fn_start: None,
      mutable_slots: HashMap::default(),
      param_slots: HashMap::default(),
      param_sym_slots: HashMap::default(),
      caller_save_base: 0,
      next_mut_slot: 0,
      struct_base: 0,
      chan_scratch_base: 0,
      select_scratch_base: 0,
      next_struct_slot: 0,
      struct_return_fns: HashMap::default(),
      last_was_math_intrinsic: false,
      extern_used: Vec::new(),
      extern_stub_offsets: HashMap::default(),
      extern_fixups: Vec::new(),
      call_fixups: Vec::new(),
      enum_metas: HashMap::default(),
      next_enum_sym: ENUM_SYNTHETIC_SYM_BASE,
      value_types: HashMap::default(),
      array_metas: HashMap::default(),
    }
  }

  // --- Register allocation helpers ---

  /// Look up the allocated register for a ValueId.
  fn alloc_reg(&self, vid: ValueId) -> Option<Register> {
    self
      .reg_alloc
      .as_ref()
      .and_then(|a| a.get(vid))
      .map(Register::new)
  }

  /// Look up the allocated GP register for the value
  /// defined at instruction `idx`.
  fn reg_for_insn(&self, idx: usize) -> Option<Register> {
    self
      .reg_alloc
      .as_ref()
      .and_then(|a| a.value_id_at(idx))
      .and_then(|vid| self.alloc_reg(vid))
  }

  /// Look up the allocated FP register for a ValueId.
  fn alloc_fp_reg(&self, vid: ValueId) -> Option<FpRegister> {
    self
      .reg_alloc
      .as_ref()
      .and_then(|a| a.get_fp(vid))
      .map(FpRegister::new)
  }

  /// Look up the allocated FP register for the value
  /// defined at instruction `idx`.
  fn fp_reg_for_insn(&self, idx: usize) -> Option<FpRegister> {
    self
      .reg_alloc
      .as_ref()
      .and_then(|a| a.value_id_at(idx))
      .and_then(|vid| self.alloc_fp_reg(vid))
  }

  /// Scan backward from `idx` (exclusive) to find the
  /// nearest instruction with an allocated FP register.
  /// Skips VarDef, Store, Label, and other non-value
  /// instructions. Used when `alloc_fp_reg` fails due to
  /// ValueId mismatch between SIR and register allocator.
  fn scan_fp_reg_back(&self, idx: usize) -> Option<FpRegister> {
    for i in (0..idx).rev() {
      if let Some(fp) = self.fp_reg_for_insn(i) {
        return Some(fp);
      }
    }

    None
  }

  /// O(1) type lookup for a ValueId.
  fn type_of(&self, vid: ValueId) -> Option<TyId> {
    self.value_types.get(&vid.0).copied()
  }

  fn is_string_value(&self, vid: ValueId) -> bool {
    self.type_of(vid).is_some_and(|ty| ty.0 == STR_TYPE_ID)
  }

  fn is_float_value(&self, vid: ValueId) -> bool {
    self
      .type_of(vid)
      .is_some_and(|ty| ty.0 >= FLOAT_TYPE_ID_MIN && ty.0 <= FLOAT_TYPE_ID_MAX)
  }

  fn is_bool_value(&self, vid: ValueId) -> bool {
    self.type_of(vid).is_some_and(|ty| ty.0 == BOOL_TYPE_ID)
  }

  fn is_char_value(&self, vid: ValueId) -> bool {
    self.type_of(vid).is_some_and(|ty| ty.0 == CHAR_TYPE_ID)
  }

  fn is_enum_value(&self, vid: ValueId) -> Option<TyId> {
    let ty = self.type_of(vid)?;

    if self.enum_metas.contains_key(&ty.0) {
      Some(ty)
    } else {
      None
    }
  }

  /// If `vid`'s type is a registered array type, return the
  /// element `TyId`. `None` for non-arrays and for arrays
  /// whose `ArrayTyDef` wasn't surfaced (defensive — every
  /// array type reached by SIR should have one).
  fn is_array_value(&self, vid: ValueId) -> Option<TyId> {
    let ty = self.type_of(vid)?;

    self.array_metas.get(&ty.0).copied()
  }

  /// Emit a single spill operation (GP or FP).
  /// Emit a BL to an external C function (e.g. _malloc,
  /// _realloc). Saves/restores caller-save registers
  /// (X9-X17) around the call. Registers the symbol for
  /// GOT binding.
  fn emit_extern_call(&mut self, c_sym: &str) {
    let base = self.caller_save_base;

    // Save caller-save registers (X9-X17).
    for i in 0..CALLER_SAVE_COUNT {
      let reg = Register::new(CALLER_SAVE_START + i as u8);
      let off = base + i as u32 * STACK_SLOT_SIZE;

      self.emitter.emit_str(reg, SP, off as i16);
    }

    let fixup_pos = self.emitter.current_offset();
    let sym = c_sym.to_owned();

    self.emitter.emit_bl(0);
    self.extern_fixups.push((fixup_pos, sym.clone()));

    if !self.extern_used.contains(&sym) {
      self.extern_used.push(sym);
    }

    // Restore caller-save registers (X9-X17).
    for i in 0..CALLER_SAVE_COUNT {
      let reg = Register::new(CALLER_SAVE_START + i as u8);
      let off = base + i as u32 * STACK_SLOT_SIZE;

      self.emitter.emit_ldr(reg, SP, off as i16);
    }
  }

  fn emit_spill_op(&mut self, kind: &SpillKind) {
    match kind {
      SpillKind::Store {
        reg,
        slot,
        class: RegisterClass::GP,
      } => self.emitter.emit_str(
        Register::new(*reg),
        SP,
        (*slot * STACK_SLOT_SIZE) as i16,
      ),
      SpillKind::Load {
        reg,
        slot,
        class: RegisterClass::GP,
      } => self.emitter.emit_ldr(
        Register::new(*reg),
        SP,
        (*slot * STACK_SLOT_SIZE) as i16,
      ),
      SpillKind::Store {
        reg,
        slot,
        class: RegisterClass::FP,
      } => self.emitter.emit_str_fp(
        FpRegister::new(*reg),
        SP,
        (*slot * STACK_SLOT_SIZE) as u16,
      ),
      SpillKind::Load {
        reg,
        slot,
        class: RegisterClass::FP,
      } => self.emitter.emit_ldr_fp(
        FpRegister::new(*reg),
        SP,
        (*slot * STACK_SLOT_SIZE) as u16,
      ),
    }
  }

  /// Emit spill ops for instruction `idx` with given
  /// timing (before or after).
  fn emit_spills(&mut self, idx: usize, timing: EmitTiming) {
    let Some(alloc) = self.reg_alloc.as_ref() else {
      return;
    };

    // Collect indices first to avoid borrow conflict
    // with self.emit_spill_op.
    let indices = alloc
      .spill_ops
      .iter()
      .enumerate()
      .filter(|(_, op)| op.insn_idx == idx && op.timing == timing)
      .map(|(i, _)| i)
      .collect::<Vec<_>>();

    for i in indices {
      let kind = self.reg_alloc.as_ref().unwrap().spill_ops[i].kind.clone();

      self.emit_spill_op(&kind);
    }
  }

  /// Load a 64-bit immediate into a register using
  /// MOV + MOVK sequence.
  /// Byte-size of a value of the given ty, as the
  /// runtime / ABI sees it. Resolves against the
  /// tychecker's canonical TyId registration order
  /// (see `TyChecker::new`). Pointer-backed types
  /// (strings, tuples, structs, arrays) count as one
  /// 8-byte word — the value in a register is the
  /// pointer itself.
  fn size_of_ty(&self, ty_id: TyId) -> u32 {
    match ty_id.0 {
      1 => 0,     // Unit — no bytes.
      2 => 1,     // Bool.
      3 => 4,     // Char (Unicode scalar, u32).
      4 | 5 => 8, // Str / Bytes — fat pointer collapsed to one word
      // at the channel ABI (producer writes the pointer,
      // consumer reads it; len lives with the heap data).
      6 | 11 => 1,      // S8 / U8.
      7 | 12 => 2,      // S16 / U16.
      8 | 13 | 10 => 4, // S32 / U32 / IntArch (aligned 4-byte default).
      9 | 14 => 8,      // S64 / U64.
      15 => 4,          // F32.
      16 | 17 => 8,     // F64 / FloatArch.
      _ => 8,           // Pointers, enums, struct handles — one word.
    }
  }

  fn emit_mov_imm_64(&mut self, reg: Register, value: u64) {
    if value <= 65535 {
      self.emitter.emit_mov_imm(reg, value as u16);
    } else {
      self.emitter.emit_mov_imm(reg, (value & 0xFFFF) as u16);

      if (value >> 16) & 0xFFFF != 0 {
        self
          .emitter
          .emit_movk(reg, ((value >> 16) & 0xFFFF) as u16, 16);
      }

      if (value >> 32) & 0xFFFF != 0 {
        self
          .emitter
          .emit_movk(reg, ((value >> 32) & 0xFFFF) as u16, 32);
      }

      if (value >> 48) & 0xFFFF != 0 {
        self
          .emitter
          .emit_movk(reg, ((value >> 48) & 0xFFFF) as u16, 48);
      }
    }
  }

  /// Emit `ADD dst, SP, #offset`. Uses X16 as scratch
  /// when the offset doesn't fit in 12 bits.
  fn emit_add_sp_offset(&mut self, dst: Register, offset: u32) {
    if offset <= 4095 {
      self.emitter.emit_add_imm(dst, SP, offset as u16);
    } else {
      self.emit_mov_imm_64(X16, offset as u64);
      self.emitter.emit_add_ext(dst, SP, X16);
    }
  }

  /// Emit `STR src, [SP, #offset]`. Uses X16 as scratch
  /// when the offset doesn't fit in a signed 9-bit imm.
  fn emit_str_sp(&mut self, src: Register, offset: u32) {
    if offset <= 255 {
      self.emitter.emit_str(src, SP, offset as i16);
    } else {
      self.emit_add_sp_offset(X16, offset);
      self.emitter.emit_str(src, X16, 0);
    }
  }

  /// Emit `LDR dst, [SP, #offset]`. Uses X16 as scratch
  /// when the offset doesn't fit in a signed 9-bit imm.
  fn emit_ldr_sp(&mut self, dst: Register, offset: u32) {
    if offset <= 255 {
      self.emitter.emit_ldr(dst, SP, offset as i16);
    } else {
      self.emit_add_sp_offset(X16, offset);
      self.emitter.emit_ldr(dst, X16, 0);
    }
  }

  // --- Code generation ---

  /// Generates `ARM64` code from SIR.
  pub fn generate(&mut self, sir: &Sir) -> Artifact {
    // Run register allocation before codegen.
    self.reg_alloc = Some(RegAlloc::allocate(
      &sir.instructions,
      sir.next_value_id,
      self.interner,
    ));

    let insns = &sir.instructions;

    // Pre-pass: identify functions that return structs.
    // Scan for patterns: FunDef ... StructConstruct ... Return.
    {
      let mut cur_fn: Option<Symbol> = None;
      let mut last_struct_fields: Option<u32> = None;

      for insn in insns.iter() {
        match insn {
          Insn::FunDef { name, .. } => {
            cur_fn = Some(*name);
            last_struct_fields = None;
          }
          Insn::StructConstruct { fields, .. } => {
            last_struct_fields = Some(fields.len() as u32);
          }
          Insn::Return { value: Some(_), .. } => {
            if let (Some(name), Some(n)) = (cur_fn, last_struct_fields) {
              self.struct_return_fns.insert(name, n);
            }

            cur_fn = None;
            last_struct_fields = None;
          }
          _ => {}
        }
      }
    }

    // Pre-pass: collect `ArrayTyDef` metadata so
    // `emit_array_write` can dispatch on the element type
    // regardless of where the definition lands in the stream.
    for insn in insns.iter() {
      if let Insn::ArrayTyDef { array_ty, elem_ty } = insn {
        self.array_metas.insert(array_ty.0, *elem_ty);
      }
    }

    for (idx, insn) in insns.iter().enumerate() {
      self.emit_spills(idx, EmitTiming::Before);
      self.translate_insn(insn, idx, insns);
      self.emit_spills(idx, EmitTiming::After);
    }

    // Patch forward-reference call fixups. Closures may
    // appear after their call sites in the SIR stream.
    for (fixup_pos, func_name) in &self.call_fixups {
      if let Some(&func_offset) = self.functions.get(func_name) {
        let offset = func_offset as i32 - *fixup_pos as i32;

        self.emitter.patch_bl(*fixup_pos, offset);
      }
    }

    // Generate _zo_ui_entry_point if we have templates.
    if self.has_templates {
      self.generate_ui_entry_point();
    }

    // Emit libm stubs at end of code section. Each stub is
    // 12 bytes (3 instructions): ADRP X16, page; LDR X16,
    // [X16, off]; BR X16. The actual page/offset values are
    // placeholders — they get patched in generate_macho()
    // once we know the final GOT layout.
    for i in 0..self.extern_used.len() {
      let sym = self.extern_used[i].clone();
      let offset = self.emitter.current_offset();

      self.extern_stub_offsets.insert(sym, offset);

      // ADRP X16, 0 — placeholder, patched later.
      self.emitter.emit_adrp(X16, 0);
      // LDR X16, [X16, #0] — placeholder, patched later.
      self.emitter.emit_ldr(X16, X16, 0);
      // BR X16
      self.emitter.emit_br(X16);
    }

    // Fix up libm BL instructions to target stubs.
    // Both BL and stub are in the same code section,
    // so this is a simple PC-relative patch.
    let extern_fixups = std::mem::take(&mut self.extern_fixups);

    let mut code = self.emitter.code();

    for (fixup_pos, c_sym) in &extern_fixups {
      if let Some(&stub_off) = self.extern_stub_offsets.get(c_sym) {
        let relative = (stub_off as i32 - *fixup_pos as i32) >> 2;
        let pos = *fixup_pos as usize;
        let insn = BL_OPCODE | ((relative as u32) & FIXUP_IMM26_MASK);

        code[pos..pos + 4].copy_from_slice(&insn.to_le_bytes());
      }
    }

    self.extern_fixups = extern_fixups;
    let mut string_offsets = HashMap::default();
    let mut template_offsets = HashMap::default();
    let mut current_offset = code.len();

    for (symbol, bytes) in &self.string_data {
      string_offsets.insert(*symbol, current_offset);

      current_offset += bytes.len();
    }

    for (symbol, bytes) in &self.template_data {
      template_offsets.insert(*symbol, current_offset);

      current_offset += bytes.len();
    }

    // Apply user-function-address fixups. `TaskSpawn`
    // emits an ADR placeholder to load the callee's
    // address into X0; here we resolve each ADR to
    // the callee function's actual code offset.
    for (fixup_pos, callee) in &self.function_addr_fixups {
      if let Some(&target_offset) = self.functions.get(callee) {
        let relative = (target_offset as i32) - (*fixup_pos as i32);
        let pos = *fixup_pos as usize;
        let existing =
          u32::from_le_bytes(code[pos..pos + 4].try_into().unwrap());
        let rd = existing & INSN_RD_MASK;
        let immlo = (relative as u32) & FIXUP_ADR_IMMLO;
        let immhi = ((relative >> 2) as u32) & FIXUP_ADR_IMMHI;
        let insn = FIXUP_ADR | (immlo << 29) | (immhi << 5) | rd;

        code[pos..pos + 4].copy_from_slice(&insn.to_le_bytes());
      }
    }

    // Apply string fixups.
    for (fixup_pos, symbol) in &self.string_fixups {
      let target_offset = string_offsets
        .get(symbol)
        .or_else(|| template_offsets.get(symbol));

      if let Some(offset) = target_offset {
        let offset = (*offset as i32) - (*fixup_pos as i32);
        let pos = *fixup_pos as usize;
        // Read the destination register from the emitted ADR.
        let existing =
          u32::from_le_bytes(code[pos..pos + 4].try_into().unwrap());
        let rd = existing & INSN_RD_MASK;
        let immlo = (offset as u32) & FIXUP_ADR_IMMLO;
        let immhi = ((offset >> 2) as u32) & FIXUP_ADR_IMMHI;
        let insn = FIXUP_ADR | (immlo << 29) | (immhi << 5) | rd;

        code[pos..pos + 4].copy_from_slice(&insn.to_le_bytes());
      }
    }

    // Apply branch fixups.
    for (fixup_pos, target_label) in &self.branch_fixups {
      if let Some(&label_offset) = self.labels.get(target_label) {
        let relative = (label_offset as i32 - *fixup_pos as i32) >> 2;
        let pos = *fixup_pos as usize;
        let existing =
          u32::from_le_bytes(code[pos..pos + 4].try_into().unwrap());

        let patched = if existing & B_FIXUP_MASK == B_FIXUP_OPCODE {
          B_FIXUP_OPCODE | ((relative as u32) & FIXUP_IMM26_MASK)
        } else if existing & CBZ_FIXUP_MASK == CBZ_FIXUP_OPCODE {
          let sf_and_op = existing & 0xFF000000;
          let rt = existing & INSN_RD_MASK;

          sf_and_op | (((relative as u32) & FIXUP_IMM19_MASK) << 5) | rt
        } else {
          existing
        };

        code[pos..pos + 4].copy_from_slice(&patched.to_le_bytes());
      }
    }

    for (_symbol, bytes) in &self.string_data {
      code.extend_from_slice(bytes);
    }

    for (_symbol, bytes) in &self.template_data {
      code.extend_from_slice(bytes);
    }

    Artifact { code }
  }

  /// Generates Mach-O executable from [`Artifact`].
  pub fn generate_macho(&mut self, artifact: Artifact) -> Vec<u8> {
    let mut macho = MachO::new();
    let mut code = artifact.code;

    // --- Libm GOT + stub patching ---
    // Each libm function gets one 8-byte GOT slot in __DATA
    // and one 12-byte stub in __TEXT. The stub does:
    //   ADRP X16, got_page
    //   LDR  X16, [X16, #got_page_off]
    //   BR   X16
    // dyld fills the GOT slot at load time via bind opcodes.
    let n_got = self.extern_used.len();
    let mut got_data = Vec::with_capacity(n_got * 8);
    let mut bind_entries: Vec<(&str, u8, u64, u8)> = Vec::new();
    // Track whether any symbol needs the zo runtime
    // dylib so we only register it when a program
    // actually uses concurrency.
    let mut needs_runtime_dylib = false;

    for (i, c_sym) in self.extern_used.iter().enumerate() {
      let got_offset_in_data = (i * 8) as u64;
      let got_vm_addr = DATA_VM_ADDR + got_offset_in_data;

      // Populate GOT slot with zero (dyld overwrites).
      got_data.extend_from_slice(&[0u8; 8]);

      // Patch the stub: ADRP X16, page_diff; LDR X16,
      // [X16, #page_off]; BR X16.
      if let Some(&stub_off) = self.extern_stub_offsets.get(c_sym) {
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
      bind_entries.push((
        c_sym,
        DATA_SEGMENT_INDEX,
        got_offset_in_data,
        ordinal,
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

    if let Some(main_sym) = self.interner.symbol("main") {
      let offset = self.functions.get(&main_sym).copied().unwrap_or(0);

      macho.add_function_symbol(
        "_main",
        1,
        TEXT_SECTION_BASE + offset as u64,
        false,
      );
    }

    if self.has_templates {
      let entry_symbol = Symbol(UI_ENTRY_SYMBOL);

      if let Some(&offset) = self.functions.get(&entry_symbol) {
        macho.add_function_symbol(
          "_zo_ui_entry_point",
          1,
          TEXT_SECTION_BASE + offset as u64,
          true,
        );
      }
    }

    // Add undefined symbols, routing each to its owning
    // dylib's ordinal so the Mach-O symtab + LC_LOAD_DYLIB
    // entries agree with the bind opcodes above.
    for c_sym in &self.extern_used {
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
    let main_entry = self
      .interner
      .symbol("main")
      .and_then(|s| self.functions.get(&s).copied())
      .map(|off| CODE_OFFSET + off as u64)
      .unwrap_or(CODE_OFFSET);

    macho.add_main(main_entry);

    macho.add_dyld_info();
    macho.finish_with_signature()
  }

  /// Generate a complete executable from SIR.
  pub fn generate_executable(&mut self, sir: &Sir) -> Vec<u8> {
    let artifact = self.generate(sir);

    self.generate_macho(artifact)
  }

  /// Generates ARM64 assembly text from SIR for display.
  pub fn generate_asm(&mut self, sir: &Sir) -> String {
    let mut asm = String::new();

    asm.push_str(
      "  .section __TEXT,__text,\
       regular,pure_instructions\n",
    );

    asm.push_str("  .build_version macos, 11, 0\n");
    asm.push_str("  .globl _main\n");
    asm.push_str("  .p2align 2\n");

    for insn in &sir.instructions {
      self.translate_insn_to_text(insn, &mut asm);
    }

    asm
  }

  /// Translate a single SIR instruction to assembly text.
  fn translate_insn_to_text(&mut self, insn: &Insn, asm: &mut String) {
    match insn {
      Insn::FunDef { name, .. } => {
        let func_name = self.interner.get(*name);

        asm.push_str(&format!("\n_{}:\n", func_name));

        self.current_function = Some(*name);
      }
      Insn::ConstInt { value, .. } => {
        if *value <= 65535 {
          asm.push_str(&format!("  mov x0, #{}\n", value));
        } else {
          asm.push_str(&format!("  mov x0, #{}\n", value & 0xFFFF));

          if (*value >> 16) & 0xFFFF != 0 {
            asm.push_str(&format!(
              "  movk x0, #{}, lsl #16\n",
              (*value >> 16) & 0xFFFF
            ));
          }

          if (*value >> 32) & 0xFFFF != 0 {
            asm.push_str(&format!(
              "  movk x0, #{}, lsl #32\n",
              (*value >> 32) & 0xFFFF
            ));
          }

          if (*value >> 48) & 0xFFFF != 0 {
            asm.push_str(&format!(
              "  movk x0, #{}, lsl #48\n",
              (*value >> 48) & 0xFFFF
            ));
          }
        }
      }
      Insn::ConstString { symbol, .. } => {
        let string = self.interner.get(*symbol);

        asm.push_str(&format!("  adr x1, .L_str_{}\n", symbol.as_u32()));
        asm.push_str(&format!("  mov x2, #{}\n", string.len()));
      }
      Insn::Call { name, .. } => {
        let func_name = self.interner.get(*name);

        match func_name {
          "show" | "showln" => {
            asm.push_str("  mov x16, #4  ; write syscall\n");
            asm.push_str("  mov x0, #1   ; stdout\n");
            asm.push_str("  svc #0\n");
          }
          "eshow" | "eshowln" => {
            asm.push_str("  mov x16, #4  ; write syscall\n");
            asm.push_str("  mov x0, #2   ; stderr\n");
            asm.push_str("  svc #0\n");
          }
          "flush" => {
            asm.push_str("  ; flush (no-op)\n");
          }
          "exit" => {
            asm.push_str("  ; exit(code) — code already in x0\n");
            asm.push_str("  mov x16, #1   ; SYS_exit\n");
            asm.push_str("  svc #0\n");
          }
          _ => {
            asm.push_str(&format!("  bl _{}\n", func_name));
          }
        }
      }
      Insn::Return { .. } => {
        asm.push_str("  mov x0, #0\n");
        asm.push_str("  ret\n");
      }
      Insn::BinOp { op, .. } => match op {
        BinOp::Add => asm.push_str("  add x0, x0, x1\n"),
        BinOp::Sub => asm.push_str("  sub x0, x0, x1\n"),
        BinOp::Mul => asm.push_str("  mul x0, x0, x1\n"),
        BinOp::Div => asm.push_str("  sdiv x0, x0, x1\n"),
        _ => asm.push_str(&format!("  ; TODO: {:?}\n", op)),
      },
      _ => {
        asm.push_str(&format!("  ; TODO: {:?}\n", insn));
      }
    }
  }

  /// Translate a single SIR instruction to ARM64.
  fn translate_insn(&mut self, insn: &Insn, idx: usize, all_insns: &[Insn]) {
    // Register value types for O(1) type detection.
    match insn {
      Insn::ConstInt { dst, ty_id, .. }
      | Insn::ConstFloat { dst, ty_id, .. }
      | Insn::ConstBool { dst, ty_id, .. }
      | Insn::Load { dst, ty_id, .. }
      | Insn::Call { dst, ty_id, .. }
      | Insn::BinOp { dst, ty_id, .. }
      | Insn::UnOp { dst, ty_id, .. }
      | Insn::ArrayLiteral { dst, ty_id, .. }
      | Insn::ArrayIndex { dst, ty_id, .. }
      | Insn::ArrayLen { dst, ty_id, .. }
      | Insn::ArrayPop { dst, ty_id, .. }
      | Insn::TupleIndex { dst, ty_id, .. }
      | Insn::EnumConstruct { dst, ty_id, .. }
      | Insn::StructConstruct { dst, ty_id, .. } => {
        self.value_types.insert(dst.0, *ty_id);
      }
      Insn::Cast { dst, to_ty, .. } => {
        self.value_types.insert(dst.0, *to_ty);
      }
      Insn::ConstString { dst, .. } => {
        self.value_types.insert(dst.0, TyId(STR_TYPE_ID));
      }
      _ => {}
    }

    match insn {
      Insn::FunDef { name, params, .. } => {
        let offset = self.emitter.current_offset();

        self.functions.insert(*name, offset);
        self.current_function = Some(*name);
        self.current_fn_start = Some(idx);
        self.mutable_slots.clear();
        self.next_mut_slot = 0;
        self.next_struct_slot = 0;
        self.param_slots.clear();
        self.param_sym_slots.clear();

        // Function prologue: save FP/LR if non-leaf.
        let fn_info = self
          .reg_alloc
          .as_ref()
          .and_then(|a| a.function_info.get(&idx))
          .map(|info| {
            (
              info.has_calls,
              info.spill_size,
              info.struct_size,
              info.mutable_size,
              info.chan_scratch_size,
              info.select_scratch_size,
            )
          });

        if let Some((
          has_calls,
          spill_size,
          struct_size,
          mut_size,
          chan_scratch_size,
          select_scratch_size,
        )) = fn_info
        {
          if has_calls {
            self.emitter.emit_stp(X29, X30, SP, FP_LR_SAVE_OFFSET);
          }

          let param_reserve = params.len() as u32 * STACK_SLOT_SIZE;
          let caller_save = if has_calls { CALLER_SAVE_RESERVE } else { 0 };
          let frame = (spill_size
            + mut_size
            + param_reserve
            + caller_save
            + struct_size
            + chan_scratch_size
            + select_scratch_size
            + FRAME_ALIGN_MASK)
            & !FRAME_ALIGN_MASK;

          if frame > 0 {
            if frame <= 4095 {
              self.emitter.emit_sub_imm(SP, SP, frame as u16);
            } else {
              self.emit_mov_imm_64(X16, frame as u64);
              self.emitter.emit_sub_ext(SP, SP, X16);
            }
          }

          self.caller_save_base = spill_size + mut_size + param_reserve;

          // Struct base: after caller-save area.
          self.struct_base =
            spill_size + mut_size + param_reserve + caller_save;

          // Channel-scratch base: after struct area.
          // Used by `ChannelSend` / `ChannelRecv` to
          // pass values through an on-stack buffer that
          // `_zo_chan_send` / `_zo_chan_recv` reads /
          // writes by pointer.
          self.chan_scratch_base =
            spill_size + mut_size + param_reserve + caller_save + struct_size;

          // Select-scratch base: after channel-scratch.
          // Holds the on-stack chans array + out_value
          // buffer consumed by `_zo_select_wait`.
          self.select_scratch_base = spill_size
            + mut_size
            + param_reserve
            + caller_save
            + struct_size
            + chan_scratch_size;

          let param_base = spill_size + mut_size;

          for (i, (sym, ty_id)) in params.iter().enumerate() {
            let off = param_base + i as u32 * STACK_SLOT_SIZE;
            let is_fp =
              ty_id.0 >= FLOAT_TYPE_ID_MIN && ty_id.0 <= FLOAT_TYPE_ID_MAX;

            if is_fp {
              let src = FpRegister::new(i as u8);

              self.emitter.emit_str_fp(src, SP, off as u16);
            } else {
              let src = Register::new(i as u8);

              self.emitter.emit_str(src, SP, off as i16);
            }

            self.param_slots.insert(i as u32, off);
            // Also index by the parameter's symbol so
            // `LoadSource::Local(sym)` (emitted for immutable
            // param reads) can resolve the spill slot.
            self.param_sym_slots.insert(sym.as_u32(), (off, is_fp));
          }
        }
      }

      Insn::ConstInt { value, .. } => {
        // Skip module-level constants (val inits) —
        // they have no function context.
        if self.current_function.is_some()
          && let Some(reg) = self.reg_for_insn(idx)
        {
          self.emit_mov_imm_64(reg, *value);
        }
      }

      Insn::ConstFloat { value, .. } => {
        // Load f64 bits into GP scratch, FMOV to FP.
        let fp_dst = self.fp_reg_for_insn(idx).unwrap_or(D0);
        let bits = value.to_bits();

        self.emit_mov_imm_64(X16, bits);
        self.emitter.emit_fmov_gp_to_fp(fp_dst, X16);
      }

      Insn::ConstBool { value, .. } => {
        if let Some(reg) = self.reg_for_insn(idx) {
          self.emitter.emit_mov_imm(reg, *value as u16);
        }
      }

      Insn::ConstString { symbol, .. } => {
        let mut buffer = Buffer::new();
        let string = self.interner.get(*symbol);

        // Length-prefixed layout: [len: u64][bytes][null].
        let len = string.len() as u64;

        buffer.bytes(&len.to_le_bytes());
        buffer.bytes(string.as_bytes());
        buffer.bytes(b"\0");

        self.string_data.push((*symbol, buffer.finish()));

        // String is a single pointer to the struct.
        let ptr_reg = self.reg_for_insn(idx).unwrap_or(X1);
        let fixup_pos = self.emitter.current_offset();

        self.string_fixups.push((fixup_pos, *symbol));
        self.emitter.emit_adr(ptr_reg, 0);
      }

      Insn::Load { dst, src, .. } => match src {
        LoadSource::Local(sym) => {
          let slot = sym.as_u32();

          if let Some(&offset) = self.mutable_slots.get(&slot) {
            if let Some(dst_reg) = self.alloc_reg(*dst) {
              self.emitter.emit_ldr(dst_reg, SP, offset as i16);
            } else if let Some(fp_dst) = self
              .alloc_fp_reg(*dst)
              .or_else(|| self.fp_reg_for_insn(idx))
            {
              // Float local: LDR Dt, [SP, #offset].
              self.emitter.emit_ldr_fp(fp_dst, SP, offset as u16);
            }
          } else if let Some(&(offset, is_fp)) = self.param_sym_slots.get(&slot)
          {
            // Parameter read lowered as `LoadSource::Local`.
            // Fall back to the param spill slot so the value
            // is reloaded from the stack — without this the
            // destination register keeps whatever the caller
            // left behind, which aliases across back-to-back
            // calls (e.g. two struct-returning calls).
            if is_fp {
              if let Some(fp_dst) = self
                .alloc_fp_reg(*dst)
                .or_else(|| self.fp_reg_for_insn(idx))
              {
                self.emitter.emit_ldr_fp(fp_dst, SP, offset as u16);
              }
            } else if let Some(dst_reg) = self.alloc_reg(*dst) {
              self.emitter.emit_ldr(dst_reg, SP, offset as i16);
            }
          }
        }
        LoadSource::Param(idx) => {
          // Load from parameter spill slot (saved in
          // prologue). This is safe even after registers
          // have been reused for other values.
          if let Some(&off) = self.param_slots.get(idx) {
            if let Some(fp_dst) = self.alloc_fp_reg(*dst) {
              // Float param: load from FP spill slot.
              self.emitter.emit_ldr_fp(fp_dst, SP, off as u16);
            } else if let Some(dst_reg) = self.alloc_reg(*dst) {
              // GP param: load from GP spill slot.
              self.emitter.emit_ldr(dst_reg, SP, off as i16);
            }
          } else if let Some(fp_dst) = self.alloc_fp_reg(*dst) {
            // Fallback: no spill slot — read from
            // original register.
            let fp_src = FpRegister::new(*idx as u8);

            if fp_dst != fp_src {
              self.emitter.emit_fmov_fp(fp_dst, fp_src);
            }
          } else if let Some(dst_reg) = self.alloc_reg(*dst) {
            let src_reg = Register::new(*idx as u8);

            if dst_reg != src_reg {
              self.emitter.emit_mov_reg(dst_reg, src_reg);
            }
          }
        }
      },

      Insn::BinOp {
        dst,
        op,
        lhs,
        rhs,
        ty_id,
      } => {
        let is_float =
          ty_id.0 >= FLOAT_TYPE_ID_MIN && ty_id.0 <= FLOAT_TYPE_ID_MAX;

        if is_float {
          let fl = self.alloc_fp_reg(*lhs).unwrap_or(D0);
          let fr = self.alloc_fp_reg(*rhs).unwrap_or(D1);
          match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => {
              let fd = self.alloc_fp_reg(*dst).unwrap_or(D0);

              match op {
                BinOp::Add => self.emitter.emit_fadd(fd, fl, fr),
                BinOp::Sub => self.emitter.emit_fsub(fd, fl, fr),
                BinOp::Mul => self.emitter.emit_fmul(fd, fl, fr),
                BinOp::Div => self.emitter.emit_fdiv(fd, fl, fr),
                _ => unreachable!(),
              }
            }
            BinOp::Lt
            | BinOp::Lte
            | BinOp::Gt
            | BinOp::Gte
            | BinOp::Eq
            | BinOp::Neq => {
              // Float comparison: result is a BOOL (GP),
              // NOT an FP value. The previous code emitted
              // only FCMP (which sets flags) and allocated
              // `fd` as an FP register — leaving the GP
              // destination reg uninitialized. `while x0 <
              // 3.0` then read garbage as its condition and
              // looped forever. Mirror the int path: FCMP
              // sets NZCV, then materialize 0/1 into the
              // GP `dst` via CSEL. For non-NaN operands the
              // signed condition codes (LT/LE/GT/GE/EQ/NE)
              // map correctly onto FCMP's flag layout.
              let d = self.alloc_reg(*dst).unwrap_or(X0);
              let cond = match op {
                BinOp::Lt => COND_LT,
                BinOp::Lte => COND_LE,
                BinOp::Gt => COND_GT,
                BinOp::Gte => COND_GE,
                BinOp::Eq => COND_EQ,
                BinOp::Neq => COND_NE,
                _ => unreachable!(),
              };

              self.emitter.emit_fcmp(fl, fr);
              self.emitter.emit_mov_imm(d, 1);
              self.emitter.emit_mov_imm(X16, 0);
              self.emitter.emit_csel(d, d, X16, cond);
            }
            _ => {}
          }
        } else if ty_id.0 == STR_TYPE_ID && matches!(op, BinOp::Eq | BinOp::Neq)
        {
          // String equality: compare lengths first, then
          // _memcmp on data pointers. String struct: [len:8, ptr:8].
          // zo strings are NOT null-terminated.
          let d = self.alloc_reg(*dst).unwrap_or(X0);
          let l = self.alloc_reg(*lhs).unwrap_or(X0);
          let r = self.alloc_reg(*rhs).unwrap_or(X1);
          let cond = if matches!(op, BinOp::Eq) {
            COND_EQ
          } else {
            COND_NE
          };

          // Load lengths: [lhs+0] and [rhs+0].
          self.emitter.emit_ldr(X16, l, 0);
          self.emitter.emit_ldr(X17, r, 0);

          // Compare lengths — if different, strings differ.
          self.emitter.emit_cmp(X16, X17);

          // If lengths differ, skip memcmp — result is "not equal".
          let len_skip = self.emitter.current_offset();
          self.emitter.emit_bne(0); // patched below

          // Lengths match — call _memcmp(ptr1, ptr2, len).
          // String layout is inline: [len:8][data...][null].
          // Data starts at base + 8, not *(base + 8).
          //
          // Register aliasing: `r` may be allocated to X0 (the
          // destination of the upcoming `add X0, l, 8`). In
          // that case emitting the lhs add first clobbers the
          // rhs pointer before the rhs add reads it. Similarly,
          // `l` may alias X1. Pick a safe ordering — and if
          // both alias simultaneously, stash rhs in a scratch
          // register first.
          if r == X0 && l == X1 {
            self.emitter.emit_mov_reg(X9, r);
            self.emitter.emit_add_imm(X0, l, 8); // ptr1
            self.emitter.emit_add_imm(X1, X9, 8); // ptr2
          } else if r == X0 {
            self.emitter.emit_add_imm(X1, r, 8); // ptr2 first
            self.emitter.emit_add_imm(X0, l, 8); // ptr1
          } else {
            self.emitter.emit_add_imm(X0, l, 8); // ptr1
            self.emitter.emit_add_imm(X1, r, 8); // ptr2
          }
          self.emitter.emit_mov_reg(X2, X16); // len
          self.emit_extern_call("_memcmp");

          // memcmp returns 0 if equal.
          self.emit_cmp_csel(d, X0, XZR, cond);

          // Jump past the "not equal" fallback.
          let done_skip = self.emitter.current_offset();
          self.emitter.emit_b(0); // patched below

          // Patch len_skip B.NE to land here.
          let here = self.emitter.current_offset();
          let len_off = here as i32 - len_skip as i32;
          self.emitter.patch_bcond_at(len_skip as usize, len_off);

          // Lengths differ → set result.
          if matches!(op, BinOp::Eq) {
            self.emitter.emit_mov_imm(d, 0);
          } else {
            self.emitter.emit_mov_imm(d, 1);
          }

          // Patch done_skip B to land here.
          let end = self.emitter.current_offset();
          let done_off = end as i32 - done_skip as i32;
          self.emitter.patch_b_at(done_skip as usize, done_off);
        } else if self.enum_metas.contains_key(&ty_id.0)
          && matches!(op, BinOp::Eq | BinOp::Neq)
        {
          // Enum equality: both operands are pointers to
          // `[tag, ...]` thanks to the uniform representation.
          // Pointer-level cmp would return false for two
          // distinct allocations holding the same variant, so
          // load both tags first and then compare. Other
          // comparison operators (`<`, `<=`, …) are undefined
          // on enum types and fall through to the integer path
          // below as a noop.
          let d = self.alloc_reg(*dst).unwrap_or(X0);
          let l = self.alloc_reg(*lhs).unwrap_or(X0);
          let r = self.alloc_reg(*rhs).unwrap_or(X1);
          let cond = if matches!(op, BinOp::Eq) {
            COND_EQ
          } else {
            COND_NE
          };

          // LDR X16, [l, #0]  ; tag from lhs.
          self.emitter.emit_ldr(X16, l, 0);
          // LDR X17, [r, #0]  ; tag from rhs.
          self.emitter.emit_ldr(X17, r, 0);
          // cmp X16, X17 ; CSET d, cond.
          self.emit_cmp_csel(d, X16, X17, cond);
        } else {
          // Integer: use allocated registers.
          let d = self.alloc_reg(*dst).unwrap_or(X0);
          let l = self.alloc_reg(*lhs).unwrap_or(X0);
          let r = self.alloc_reg(*rhs).unwrap_or(X1);

          // Unsigned integer types occupy TyId 11..=14
          // (u8/u16/u32/u64). Signed are 6..=10. Several
          // ARM64 ops are sign-dependent and used the
          // signed variant unconditionally:
          //   - `<`/`<=`/`>`/`>=` (LT/LE/GT/GE check N
          //     vs V; unsigned wants CC/LS/HI/CS based on
          //     the carry flag).
          //   - `/` and `%` (SDIV sign-extends the
          //     dividend; UDIV zero-extends).
          //   - `>>` (ASR propagates the sign bit; LSR
          //     fills zeros).
          // Misusing the signed forms on `u64` values
          // whose high bit is set silently returns the
          // wrong sign (e.g. `18000000000000000000_u64 >
          // 1` returned 0 because SIGNED compared the
          // u64 as negative).
          let is_unsigned = ty_id.0 >= 11 && ty_id.0 <= 14;

          match op {
            BinOp::Add => self.emitter.emit_add(d, l, r),
            BinOp::Sub => self.emitter.emit_sub(d, l, r),
            BinOp::Mul => self.emitter.emit_mul(d, l, r),
            BinOp::Div => {
              if is_unsigned {
                self.emitter.emit_udiv(d, l, r);
              } else {
                self.emitter.emit_sdiv(d, l, r);
              }
            }
            BinOp::Rem => {
              // dst = lhs - (lhs / rhs) * rhs. Use X16 as
              // scratch. Route through the correct DIV
              // flavour to keep unsigned remainders
              // correct for `u*` types.
              if is_unsigned {
                self.emitter.emit_udiv(X16, l, r);
              } else {
                self.emitter.emit_sdiv(X16, l, r);
              }
              self.emitter.emit_mul(X16, X16, r);
              self.emitter.emit_sub(d, l, X16);
            }
            BinOp::And | BinOp::BitAnd => self.emitter.emit_and(d, l, r),
            BinOp::Or | BinOp::BitOr => self.emitter.emit_orr(d, l, r),
            BinOp::BitXor => self.emitter.emit_eor(d, l, r),
            // `emit_lsl` / `emit_lsr` take an IMMEDIATE
            // shift (encoded via UBFM). The previous code
            // passed the literal `1`, so every runtime
            // shift collapsed to `<< 1` regardless of the
            // source — a shift-by-constant off an Ident
            // (`acc << 4`) silently produced `acc << 1`,
            // while the same expression with a literal LHS
            // worked via const-folding. Use the variable-
            // shift forms (LSLV / LSRV) so the RHS register
            // carries the real count.
            BinOp::Shl => self.emitter.emit_lslv(d, l, r),
            BinOp::Shr => {
              if is_unsigned {
                self.emitter.emit_lsrv(d, l, r);
              } else {
                self.emitter.emit_asrv(d, l, r);
              }
            }
            BinOp::Lt => {
              let c = if is_unsigned { COND_CC } else { COND_LT };
              self.emit_cmp_csel(d, l, r, c);
            }
            BinOp::Lte => {
              let c = if is_unsigned { COND_LS } else { COND_LE };
              self.emit_cmp_csel(d, l, r, c);
            }
            BinOp::Gt => {
              let c = if is_unsigned { COND_HI } else { COND_GT };
              self.emit_cmp_csel(d, l, r, c);
            }
            BinOp::Gte => {
              let c = if is_unsigned { COND_CS } else { COND_GE };
              self.emit_cmp_csel(d, l, r, c);
            }
            BinOp::Eq => self.emit_cmp_csel(d, l, r, COND_EQ),
            BinOp::Neq => self.emit_cmp_csel(d, l, r, COND_NE),
            BinOp::Concat => {
              self.emit_str_concat(d, l, r);
            }
          }
        }
      }

      Insn::UnOp { dst, op, rhs, .. } => {
        let d = self.alloc_reg(*dst).unwrap_or(X0);
        let r = self.alloc_reg(*rhs).unwrap_or(X0);

        match op {
          UnOp::Neg => self.emitter.emit_sub(d, XZR, r),
          UnOp::Not => {
            // !b => b ^ 1 (boolean not).
            self.emitter.emit_mov_imm(X16, 1);
            self.emitter.emit_eor(d, r, X16);
          }
          _ => {}
        }
      }

      Insn::Call { name, args, .. } => {
        match self.interner.get(*name) {
          "show" | "showln" | "eshow" | "eshowln" => {
            let fn_name = self.interner.get(*name);

            let fd = if fn_name.starts_with('e') {
              FD_STDERR
            } else {
              FD_STDOUT
            };

            let arg_vid = if args.is_empty() { None } else { Some(args[0]) };

            self.emit_typed_write(arg_vid, fd);

            if fn_name.ends_with("ln") {
              self.emit_newline(fd);
            }
          }
          "check" => {
            // check(condition: bool) — abort if false.
            // Arg is in allocated register; move to X0.
            let arg_vid = if args.is_empty() { None } else { Some(args[0]) };

            if let Some(src) = arg_vid.and_then(|v| self.alloc_reg(v))
              && src != X0
            {
              self.emitter.emit_mov_reg(X0, src);
            }

            self.emit_check_fail();
          }
          "flush" => {}

          "exit" => {
            // exit(code) — move the code into X0 (the
            // first-arg ABI slot), set X16 to SYS_exit,
            // svc 0. The kernel reaps the process; we
            // never return, so no teardown follows.
            let arg_vid = args.first().copied();

            if let Some(src) = arg_vid.and_then(|v| self.alloc_reg(v))
              && src != X0
            {
              self.emitter.emit_mov_reg(X0, src);
            }

            self.emitter.emit_mov_imm(X16, SYS_EXIT);
            self.emitter.emit_svc(0);
          }

          "exists" => self.emit_io_exists(args, idx),
          "read_file" => self.emit_io_read_file(args, idx),
          "write_file" => self.emit_io_write_file(args, idx),
          "append_file" => self.emit_io_append_file(args, idx),

          // HashMap apply-method dispatch. Names match
          // the executor's `<Type>::<method>` mangling
          // (same convention `apply char` / `apply int`
          // already use). Each handler emits the byte-
          // marshaling sequence around `BL _zo_map_*`.
          //
          // `HashMap::len`, `HashMap::is_empty`, and
          // `HashMap::free` are deliberately absent: they
          // are pure-zo bodies (see `std/map.zo`) that
          // call the non-marshaling raw FFIs below.
          "HashMap::new" => self.emit_map_new(args, idx),
          "HashMap::insert" => self.emit_map_insert(args, idx),
          "HashMap::get" => self.emit_map_get(args, idx),
          "HashMap::contains_key" => self.emit_map_contains(args, idx),
          "HashMap::remove" => self.emit_map_remove(args, idx),

          // Vec apply-method dispatch. Same convention as
          // HashMap: `len`, `is_empty`, `free` are pure-zo
          // bodies that call the raw FFIs below.
          "Vec::new" => self.emit_vec_new(args, idx),
          "Vec::push" => self.emit_vec_push(args, idx),
          "Vec::pop" => self.emit_vec_pop(args, idx),
          "Vec::get" => self.emit_vec_get(args, idx),
          "Vec::set" => self.emit_vec_set(args, idx),

          // HashSet apply-method dispatch. Reuses the
          // `_zo_map_*` runtime allocator with `val_sz=0`.
          "HashSet::new" => self.emit_set_new(args, idx),
          "HashSet::insert" => self.emit_set_insert(args, idx),
          "HashSet::contains" => self.emit_set_contains(args, idx),
          "HashSet::remove" => self.emit_set_remove(args, idx),

          // Non-marshaling raw FFIs. The argument is the
          // already-loaded `*mut ZoMap` / `*mut ZoVec`
          // (from `self.ptr`); pass through to the runtime
          // export with no byte marshaling.
          "__zo_map_len_raw" => self.emit_map_len_raw(args, idx),
          "__zo_map_free_raw" => self.emit_map_free_raw(args, idx),
          "__zo_vec_len_raw" => self.emit_vec_len_raw(args, idx),
          "__zo_vec_free_raw" => self.emit_vec_free_raw(args, idx),
          "__zo_set_len_raw" => self.emit_set_len_raw(args, idx),
          "__zo_set_free_raw" => self.emit_set_free_raw(args, idx),

          // Math intrinsics — ARM64 hardware instructions.
          // The arg is a float in a FP register. Move it
          // to D0, execute the instruction, leave result
          // in D0 for showln/binding to consume.
          "sqrt" | "floor" | "ceil" | "trunc" | "round" => {
            let fn_name = self.interner.get(*name);

            // Use the regular arg passing for FP: move
            // the first arg to D0.
            if let Some(arg) = args.first() {
              if let Some(fp_src) = self.alloc_fp_reg(*arg) {
                if fp_src != D0 {
                  self.emitter.emit_fmov_fp(D0, fp_src);
                }
              } else {
                // Arg might be in GP reg (from
                // ConstFloat via fmov_gp_to_fp).
                // The ConstFloat handler already put
                // it in a FP register — find it by
                // checking all FP allocations.
                if let Some(fp) = self.fp_reg_for_insn(idx.wrapping_sub(1))
                  && fp != D0
                {
                  self.emitter.emit_fmov_fp(D0, fp);
                }
              }
            }

            match fn_name {
              "sqrt" => self.emitter.emit_fsqrt(D0, D0),
              "floor" => self.emitter.emit_frintm(D0, D0),
              "ceil" => self.emitter.emit_frintp(D0, D0),
              "trunc" => self.emitter.emit_frintz(D0, D0),
              "round" => self.emitter.emit_frintn(D0, D0),
              _ => {}
            }

            self.last_was_math_intrinsic = true;
          }

          // Float classification — return bool (0 or 1).
          "is_nan" | "is_finite" => {
            let fn_name = self.interner.get(*name);

            // Move arg to D0.
            if let Some(arg) = args.first() {
              if let Some(fp_src) = self.alloc_fp_reg(*arg) {
                if fp_src != D0 {
                  self.emitter.emit_fmov_fp(D0, fp_src);
                }
              } else if let Some(fp) = self.fp_reg_for_insn(idx.wrapping_sub(1))
                && fp != D0
              {
                self.emitter.emit_fmov_fp(D0, fp);
              }
            }

            // FCMP D0, D0 — NaN != NaN sets V flag.
            self.emitter.emit_fcmp(D0, D0);

            let dst = self.reg_for_insn(idx).unwrap_or(X0);

            match fn_name {
              "is_nan" => {
                // CSET Xd, VS — V flag set means NaN.
                self.emitter.emit_cset(dst, COND_VS);
              }
              "is_finite" => {
                // CSET Xd, VC — V flag clear means
                // not NaN. But infinity also clears V.
                // Use: FCMP D0, D0; CSET tmp, VC
                // Then check for infinity separately.
                // Simplified: finite = !NaN && !Inf.
                // For MVP: just check !NaN (close enough
                // for most use cases).
                self.emitter.emit_cset(dst, COND_VC);
              }
              _ => {}
            }
          }

          // Libm functions — require dynamic library call.
          // Move float args to D0 (and D1 for pow), emit BL
          // to stub (fixup recorded), result arrives in D0.
          //
          // Re-materialize float constants directly into the
          // target registers. The register allocator may
          // assign the same FP register to multiple ConstFloat
          // values preceding a Call (since it sees them as
          // consumed at the same point), causing clobbering.
          // Loading fresh from the constant avoids this.
          "pow" | "sin" | "cos" | "tan" | "log" | "log2" | "log10" | "exp" => {
            let fn_name = self.interner.get(*name).to_string();
            let c_sym = libm_c_symbol(&fn_name);
            let nargs = libm_arg_count(&fn_name);

            // Load each float arg directly into D0..Dn.
            // Scan backwards from the Call to find the
            // producing ConstFloat for each arg. If the
            // producing instruction is not a ConstFloat,
            // fall back to the allocated FP register.
            for (i, _arg) in args.iter().enumerate().take(nargs) {
              let fp_dst = FpRegister::new(i as u8);
              let producing_idx = idx.wrapping_sub(nargs - i);

              if let Some(Insn::ConstFloat { value, .. }) =
                all_insns.get(producing_idx)
              {
                // Re-materialize the constant directly
                // into the target FP register.
                let bits = value.to_bits();

                self.emit_mov_imm_64(X16, bits);
                self.emitter.emit_fmov_gp_to_fp(fp_dst, X16);
              } else if let Some(fp_src) = self.fp_reg_for_insn(producing_idx)
                && fp_src != fp_dst
              {
                self.emitter.emit_fmov_fp(fp_dst, fp_src);
              }
            }

            // Save caller-saved regs before BL.
            let base = self.caller_save_base;

            for i in 0..CALLER_SAVE_COUNT {
              let reg = Register::new(CALLER_SAVE_START + i as u8);
              let off = base + i as u32 * STACK_SLOT_SIZE;

              self.emitter.emit_str(reg, SP, off as i16);
            }

            // Emit BL placeholder (offset 0). Will be
            // patched in assemble() to target the stub.
            let fixup_pos = self.emitter.current_offset();

            self.emitter.emit_bl(0);
            self.extern_fixups.push((fixup_pos, c_sym.clone()));

            // Track used libm functions (no duplicates).
            if !self.extern_used.contains(&c_sym) {
              self.extern_used.push(c_sym);
            }

            // Restore caller-saved regs after BL.
            for i in 0..CALLER_SAVE_COUNT {
              let reg = Register::new(CALLER_SAVE_START + i as u8);
              let off = base + i as u32 * STACK_SLOT_SIZE;

              self.emitter.emit_ldr(reg, SP, off as i16);
            }

            // Result is in D0. Move to allocated FP reg.
            if let Some(fp_result) = self.fp_reg_for_insn(idx)
              && fp_result != D0
            {
              self.emitter.emit_fmov_fp(fp_result, D0);
            }

            self.last_was_math_intrinsic = true;
          }

          _ => {
            // Move args to X0-X7 (GP) or D0-D7 (FP).
            // Collect moves first to detect clobbering:
            // if src of move B == dst of move A, moving A
            // first overwrites B's source. Save conflicting
            // sources to X16 before any moves happen.
            let mut gp_moves: Vec<(Register, Register)> = Vec::new();

            for (i, arg) in args.iter().enumerate() {
              if i >= MAX_REG_ARGS {
                break;
              }

              if let Some(fp_src) = self.alloc_fp_reg(*arg) {
                let fp_dst = FpRegister::new(i as u8);

                if fp_src != fp_dst {
                  self.emitter.emit_fmov_fp(fp_dst, fp_src);
                }
              } else if let Some(src_reg) = self.alloc_reg(*arg) {
                let dst_reg = Register::new(i as u8);

                if src_reg != dst_reg {
                  gp_moves.push((dst_reg, src_reg));
                }
              }
            }

            // Pre-save: if any move's src is also another
            // move's dst, save the src to X16 first. This
            // handles the common case of register overlap
            // in closure calls with captures.
            let mut saved_reg: Option<Register> = None;

            for j in 0..gp_moves.len() {
              let (_, src) = gp_moves[j];

              let is_clobbered = gp_moves
                .iter()
                .enumerate()
                .any(|(k, (dst, _))| k != j && *dst == src);

              if is_clobbered && saved_reg.is_none() {
                self.emitter.emit_mov_reg(X16, src);
                saved_reg = Some(src);
              }
            }

            // Emit moves, replacing saved src with X16.
            for (dst, src) in &gp_moves {
              let actual_src = if Some(*src) == saved_reg { X16 } else { *src };

              self.emitter.emit_mov_reg(*dst, actual_src);
            }

            // Save caller-saved temp regs (X9-X17) before BL.
            // These may hold live values that the callee
            // will clobber (ARM64: X0-X17 are caller-saved).
            let base = self.caller_save_base;

            for i in 0..CALLER_SAVE_COUNT {
              let reg = Register::new(CALLER_SAVE_START + i as u8);
              let off = base + i as u32 * STACK_SLOT_SIZE;

              self.emitter.emit_str(reg, SP, off as i16);
            }

            // BL to user-defined function.
            if let Some(&func_offset) = self.functions.get(name) {
              let current = self.emitter.current_offset();
              let offset = func_offset as i32 - current as i32;

              self.emitter.emit_bl(offset);
            } else {
              // Forward reference (e.g., closure defined
              // after the call site). Emit placeholder BL
              // and record fixup for patching after all
              // functions are registered.
              let fixup_pos = self.emitter.current_offset();

              self.emitter.emit_bl(0);
              self.call_fixups.push((fixup_pos, *name));
            }

            // Restore caller-saved temp regs after BL.
            for i in 0..CALLER_SAVE_COUNT {
              let reg = Register::new(CALLER_SAVE_START + i as u8);
              let off = base + i as u32 * STACK_SLOT_SIZE;

              self.emitter.emit_ldr(reg, SP, off as i16);
            }

            // If callee returns a struct, x0 holds a
            // dangling pointer into the callee's frame.
            // Copy the struct fields into the caller's
            // own struct area before x0 becomes stale.
            if let Some(&field_count) = self.struct_return_fns.get(name) {
              let dst_base = self.struct_base + self.next_struct_slot;

              for i in 0..field_count {
                let src_off = (i * STACK_SLOT_SIZE) as i16;
                let dst_off = dst_base + i * STACK_SLOT_SIZE;

                self.emitter.emit_ldr(X16, X0, src_off);
                self.emitter.emit_str(X16, SP, dst_off as i16);
              }

              // Point result at the caller's copy.
              if let Some(result_reg) = self.reg_for_insn(idx) {
                self.emitter.emit_add_imm(result_reg, SP, dst_base as u16);
              }

              // Also materialize the pointer in X0. The
              // register allocator's spill-around-next-call
              // logic captures the call result's original
              // register (X0) at allocation time, then emits
              // a pre-next-call Store from X0. For scalar
              // calls, X0 already holds the call result. For
              // struct-returning calls, X0 holds the callee's
              // stale frame pointer (used only for the copy
              // loop above), so the Store would spill stale
              // data. Fix: overwrite X0 with the caller's
              // own struct pointer after the copy completes,
              // mirroring the scalar case and keeping the
              // spill-from-X0 invariant valid across
              // chained struct-returning calls (e.g.
              // `(Point::new(..), Point::new(..))`).
              self.emitter.emit_add_imm(X0, SP, dst_base as u16);

              self.next_struct_slot += field_count * STACK_SLOT_SIZE;
            } else if let Some(fp_result) = self.fp_reg_for_insn(idx) {
              // Move result to allocated register.
              // Float results arrive in D0, GP in X0.
              if fp_result != D0 {
                self.emitter.emit_fmov_fp(fp_result, D0);
              }
            } else if let Some(result_reg) = self.reg_for_insn(idx)
              && result_reg != X0
            {
              self.emitter.emit_mov_reg(result_reg, X0);
            }
          }
        }
      }

      Insn::Return { value, ty_id } => {
        // Move return value to X0 (GP) or D0 (FP).
        let is_fp_return =
          ty_id.0 >= FLOAT_TYPE_ID_MIN && ty_id.0 <= FLOAT_TYPE_ID_MAX;

        if let Some(vid) = value {
          if is_fp_return {
            if let Some(fp_src) = self.alloc_fp_reg(*vid)
              && fp_src != D0
            {
              self.emitter.emit_fmov_fp(D0, fp_src);
            }
          } else if let Some(src_reg) = self.alloc_reg(*vid)
            && src_reg != X0
          {
            self.emitter.emit_mov_reg(X0, src_reg);
          }
        } else {
          self.emitter.emit_mov_imm(X0, 0);
        }

        // Function epilogue — frame size must match prologue.
        let epi_info = self.current_fn_start.and_then(|start| {
          self
            .reg_alloc
            .as_ref()
            .and_then(|a| a.function_info.get(&start))
            .map(|info| {
              (
                info.has_calls,
                info.spill_size,
                info.struct_size,
                info.mutable_size,
                info.chan_scratch_size,
                info.select_scratch_size,
              )
            })
        });

        if let Some((
          has_calls,
          spill_size,
          struct_size,
          mut_size,
          chan_scratch_size,
          select_scratch_size,
        )) = epi_info
        {
          let param_reserve = self.param_slots.len() as u32 * STACK_SLOT_SIZE;
          let caller_save = if has_calls { CALLER_SAVE_RESERVE } else { 0 };
          let frame = (spill_size
            + mut_size
            + param_reserve
            + caller_save
            + struct_size
            + chan_scratch_size
            + select_scratch_size
            + FRAME_ALIGN_MASK)
            & !FRAME_ALIGN_MASK;

          if frame > 0 {
            if frame <= 4095 {
              self.emitter.emit_add_imm(SP, SP, frame as u16);
            } else {
              self.emit_mov_imm_64(X16, frame as u64);
              self.emitter.emit_add_ext(SP, SP, X16);
            }
          }

          if has_calls {
            self.emitter.emit_ldp(X29, X30, SP, FP_LR_LOAD_OFFSET);
          }
        }

        self.emitter.emit_ret();
      }

      Insn::VarDef { .. } => {
        // Handled in execution phase.
      }

      Insn::Store { name, value, .. } => {
        // Variable write: STR value to stack slot.
        // Allocate slot on first Store, reuse after.
        let slot_key = name.as_u32();

        let offset = if let Some(&off) = self.mutable_slots.get(&slot_key) {
          off
        } else {
          let base = self
            .current_fn_start
            .and_then(|s| {
              self
                .reg_alloc
                .as_ref()
                .and_then(|a| a.function_info.get(&s))
            })
            .map(|info| info.spill_size)
            .unwrap_or(0);

          let off = base + self.next_mut_slot * STACK_SLOT_SIZE;

          self.mutable_slots.insert(slot_key, off);
          self.next_mut_slot += 1;

          off
        };

        if let Some(src_reg) = self.alloc_reg(*value) {
          self.emitter.emit_str(src_reg, SP, offset as i16);
        } else if let Some(fp_src) = self
          .alloc_fp_reg(*value)
          .or_else(|| self.scan_fp_reg_back(idx))
        {
          // Float variable: STR Dt, [SP, #offset].
          self.emitter.emit_str_fp(fp_src, SP, offset as u16);
        }
      }

      Insn::Template {
        id,
        name: tpl_name,
        commands,
        ..
      } => {
        self.handle_template(*id, *tpl_name, commands);
      }

      Insn::Directive { name, value, .. } => {
        let n = self.interner.get(*name);

        if n == "dom" {
          self.emit_render_call(*value);
        }
      }

      Insn::Label { id } => {
        self.labels.insert(*id, self.emitter.current_offset());
      }

      Insn::Jump { target } => {
        self
          .branch_fixups
          .push((self.emitter.current_offset(), *target));

        self.emitter.emit_b(0);
      }

      Insn::BranchIfNot { cond, target } => {
        let reg = self.alloc_reg(*cond).unwrap_or(X0);

        self
          .branch_fixups
          .push((self.emitter.current_offset(), *target));

        self.emitter.emit_cbz(reg, 0);
      }

      Insn::ArrayLiteral { elements, .. } => {
        if elements.is_empty() {
          // Empty array: heap-allocate via malloc.
          // Layout: [len:8][cap:8][data...]
          let initial_cap: u32 = 1024;
          let alloc_size =
            (ARRAY_HEADER_SIZE as u32 + initial_cap * STACK_SLOT_SIZE) as u64;

          self.emit_mov_imm_64(X0, alloc_size);
          self.emit_extern_call("_malloc");
          // X0 = heap pointer. Store len=0, cap=initial_cap.
          self.emitter.emit_mov_imm(X16, 0);
          self.emitter.emit_str(X16, X0, 0);
          self.emit_mov_imm_64(X16, initial_cap as u64);
          self.emitter.emit_str(X16, X0, 8);

          // Store heap pointer to stack slot so Store/Load
          // can find it.
          let base = self.struct_base + self.next_struct_slot;

          self.emitter.emit_str(X0, SP, base as i16);

          if let Some(dst) = self.reg_for_insn(idx) {
            self.emitter.emit_mov_reg(dst, X0);
          }

          // Only 1 stack slot for the pointer.
          self.next_struct_slot += STACK_SLOT_SIZE;
        } else {
          // Non-empty literal: stack-allocate (unchanged).
          // Layout: [len:8][cap:8][e0:8][e1:8]...[eN:8]
          let base = self.struct_base + self.next_struct_slot;
          let n = elements.len() as u32;

          // Store length at [SP + base].
          self.emitter.emit_mov_imm(X16, n as u16);
          self.emitter.emit_str(X16, SP, base as i16);

          // Store capacity = len (tight, no growth).
          self
            .emitter
            .emit_str(X16, SP, (base + STACK_SLOT_SIZE) as i16);

          // Store each element.
          for (i, elem) in elements.iter().enumerate() {
            let off =
              base + ARRAY_HEADER_SIZE as u32 + i as u32 * STACK_SLOT_SIZE;

            if let Some(fp) = self.alloc_fp_reg(*elem) {
              self.emitter.emit_str_fp(fp, SP, off as u16);
            } else if let Some(reg) = self.alloc_reg(*elem) {
              self.emitter.emit_str(reg, SP, off as i16);
            }
          }

          // Result: pointer to array base.
          if let Some(dst) = self.reg_for_insn(idx) {
            self.emitter.emit_add_imm(dst, SP, base as u16);
          }

          // Advance slot: 2 (header) + N elements.
          self.next_struct_slot += (2 + n) * STACK_SLOT_SIZE;
        }
      }

      Insn::ArrayIndex {
        dst,
        array,
        index,
        ty_id,
      } => {
        let arr_reg = self.alloc_reg(*array).unwrap_or(X0);
        let idx_reg = self.alloc_reg(*index).unwrap_or(X1);
        let is_str_index = ty_id.0 == CHAR_TYPE_ID;

        if is_str_index {
          // String layout: [len: u64][bytes][null].
          // Byte at index i is at base + 8 + i.
          // Bounds check: index < len, else exit(1).
          self.emitter.emit_ldr(X16, arr_reg, 0);
          self.emitter.emit_cmp(idx_reg, X16);
          // B.CC (unsigned <) — in-bounds, skip panic.
          let bcc_pos = self.emitter.current_offset();
          self.emitter.emit_bcc(0); // placeholder
          // Out-of-bounds: exit(1).
          self.emitter.emit_mov_imm(X0, 1);
          self.emitter.emit_mov_imm(X16, SYS_EXIT);
          self.emitter.emit_svc(0);
          // Patch B.CC to jump here (past panic).
          let here = self.emitter.current_offset() as i32;
          self
            .emitter
            .patch_bcond_at(bcc_pos as usize, here - bcc_pos as i32);
          // LDRB: load byte at base + 8 + index.
          self.emitter.emit_add_imm(X16, arr_reg, 8);
          self.emitter.emit_add(X16, X16, idx_reg);

          if let Some(dst_reg) = self.alloc_reg(*dst) {
            self.emitter.emit_ldrb(dst_reg, X16, 0);
          }
        } else {
          // Array layout: [len:8][cap:8][e0:8][e1:8]...
          // Element at index i is at base + 16 + i * 8.
          self.emitter.emit_lsl(X16, idx_reg, ARRAY_ELEMENT_SHIFT);
          self.emitter.emit_add(X16, arr_reg, X16);
          self.emitter.emit_add_imm(X16, X16, ARRAY_HEADER_SIZE);

          let is_flt =
            ty_id.0 >= FLOAT_TYPE_ID_MIN && ty_id.0 <= FLOAT_TYPE_ID_MAX;

          if is_flt {
            let fp_dst = self.fp_reg_for_insn(idx).unwrap_or(D0);
            self.emitter.emit_ldr_fp(fp_dst, X16, 0);
          } else if let Some(dst_reg) = self.alloc_reg(*dst) {
            self.emitter.emit_ldr(dst_reg, X16, 0);
          }
        }
      }

      Insn::ArrayStore {
        array,
        index,
        value,
        ty_id,
      } => {
        // Store value at base + 16 + index * 8.
        let arr_reg = self.alloc_reg(*array).unwrap_or(X0);
        let idx_reg = self.alloc_reg(*index).unwrap_or(X1);

        // Compute element address.
        self.emitter.emit_lsl(X16, idx_reg, ARRAY_ELEMENT_SHIFT);
        self.emitter.emit_add(X16, arr_reg, X16);
        self.emitter.emit_add_imm(X16, X16, ARRAY_HEADER_SIZE);

        let is_flt =
          ty_id.0 >= FLOAT_TYPE_ID_MIN && ty_id.0 <= FLOAT_TYPE_ID_MAX;

        if is_flt {
          let fp = self.alloc_fp_reg(*value).unwrap_or(D0);
          self.emitter.emit_str_fp(fp, X16, 0);
        } else {
          let val_reg = self.alloc_reg(*value).unwrap_or(X2);
          self.emitter.emit_str(val_reg, X16, 0);
        }
      }

      Insn::ArrayLen { dst, array, .. } => {
        // Length at [base + 0].
        if let Some(dst_reg) = self.alloc_reg(*dst) {
          let arr_reg = self.alloc_reg(*array).unwrap_or(X0);

          self.emitter.emit_ldr(dst_reg, arr_reg, 0);
        }
      }

      Insn::ArrayPush {
        array,
        value,
        ty_id,
      } => {
        // Layout: [len:8][cap:8][data...]
        // 1. Load len and cap.
        // 2. If len >= cap: realloc (double cap).
        // 3. Store value at data[len].
        // 4. Increment len.
        let arr_reg = self.alloc_reg(*array).unwrap_or(X0);

        // X16 = len, X17 = cap.
        self.emitter.emit_ldr(X16, arr_reg, 0);
        self.emitter.emit_ldr(X17, arr_reg, 8);

        // If len < cap, skip realloc.
        self.emitter.emit_cmp(X16, X17);
        let bcc_pos = self.emitter.current_offset();
        self.emitter.emit_bcc(0); // B.CC (unsigned <)

        // Realloc path: double capacity.
        // Save value reg before BL clobbers it.
        let val_reg = self.alloc_reg(*value).unwrap_or(X1);

        // new_cap = cap * 2.
        self.emitter.emit_lsl(X17, X17, 1);
        // alloc_size = (2 + new_cap) * 8.
        self.emitter.emit_add_imm(X1, X17, 2);
        self.emitter.emit_lsl(X1, X1, ARRAY_ELEMENT_SHIFT);
        // X0 = old pointer.
        self.emitter.emit_mov_reg(X0, arr_reg);
        // Save new_cap and value PAST the caller-save area
        // so emit_extern_call's X9-X17 save doesn't clobber.
        let extra_base =
          self.caller_save_base + (CALLER_SAVE_COUNT as u32) * STACK_SLOT_SIZE;

        self.emitter.emit_str(X17, SP, extra_base as i16);
        self.emitter.emit_str(val_reg, SP, (extra_base + 8) as i16);
        self.emit_extern_call("_realloc");
        // X0 = new pointer. Restore new_cap + value.
        self.emitter.emit_ldr(X17, SP, extra_base as i16);
        self.emitter.emit_ldr(val_reg, SP, (extra_base + 8) as i16);
        // Store new cap.
        self.emitter.emit_str(X17, X0, 8);
        // Update arr_reg to new pointer.
        self.emitter.emit_mov_reg(arr_reg, X0);
        // Write the new pointer back to the array's local
        // slot. Scan SIR from the current function only.
        let fn_start = self.current_fn_start.unwrap_or(0);

        for insn in all_insns[fn_start..].iter() {
          if let Insn::Load {
            dst,
            src: LoadSource::Local(sym),
            ..
          } = insn
            && *dst == *array
          {
            if let Some(&off) = self.mutable_slots.get(&sym.as_u32()) {
              self.emitter.emit_str(arr_reg, SP, off as i16);
            }

            break;
          }
        }

        // Patch B.CC to skip realloc.
        let here = self.emitter.current_offset() as i32;
        self
          .emitter
          .patch_bcond_at(bcc_pos as usize, here - bcc_pos as i32);

        // Reload len (registers were clobbered by realloc).
        self.emitter.emit_ldr(X16, arr_reg, 0);

        // Store value at data[len]: base + 16 + len * 8.
        self.emitter.emit_lsl(X17, X16, ARRAY_ELEMENT_SHIFT);
        self.emitter.emit_add(X17, arr_reg, X17);
        self.emitter.emit_add_imm(X17, X17, ARRAY_HEADER_SIZE);

        let is_flt =
          ty_id.0 >= FLOAT_TYPE_ID_MIN && ty_id.0 <= FLOAT_TYPE_ID_MAX;

        if is_flt {
          let fp = self.alloc_fp_reg(*value).unwrap_or(D0);
          self.emitter.emit_str_fp(fp, X17, 0);
        } else {
          self.emitter.emit_str(val_reg, X17, 0);
        }

        // Increment len: len + 1 → store back.
        self.emitter.emit_add_imm(X16, X16, 1);
        self.emitter.emit_str(X16, arr_reg, 0);
      }

      Insn::ArrayPop { dst, array, ty_id } => {
        // Layout: [len:8][cap:8][data...]
        // 1. Load len, check > 0.
        // 2. Decrement len, store back.
        // 3. Load data[new_len] into dst.
        let arr_reg = self.alloc_reg(*array).unwrap_or(X0);

        // X16 = len.
        self.emitter.emit_ldr(X16, arr_reg, 0);
        // Check len > 0: CMP len, #0 → B.NE (skip panic).
        self.emitter.emit_cmp_imm(X16, 0);
        let bne_pos = self.emitter.current_offset();
        self.emitter.emit_bne(0); // placeholder
        // Panic: pop on empty array — exit(1).
        self.emitter.emit_mov_imm(X0, 1);
        self.emitter.emit_mov_imm(X16, SYS_EXIT);
        self.emitter.emit_svc(0);
        // Patch B.NE past panic.
        let here = self.emitter.current_offset() as i32;
        self
          .emitter
          .patch_bcond_at(bne_pos as usize, here - bne_pos as i32);

        // Reload len (X16 was clobbered).
        self.emitter.emit_ldr(X16, arr_reg, 0);
        // Decrement: new_len = len - 1.
        self.emitter.emit_sub_imm(X16, X16, 1);
        // Store new len.
        self.emitter.emit_str(X16, arr_reg, 0);

        // Load data[new_len]: base + 16 + new_len * 8.
        self.emitter.emit_lsl(X17, X16, ARRAY_ELEMENT_SHIFT);
        self.emitter.emit_add(X17, arr_reg, X17);
        self.emitter.emit_add_imm(X17, X17, ARRAY_HEADER_SIZE);

        let is_flt =
          ty_id.0 >= FLOAT_TYPE_ID_MIN && ty_id.0 <= FLOAT_TYPE_ID_MAX;

        if is_flt {
          let fp_dst = self.fp_reg_for_insn(idx).unwrap_or(D0);
          self.emitter.emit_ldr_fp(fp_dst, X17, 0);
        } else if let Some(dst_reg) = self.alloc_reg(*dst) {
          self.emitter.emit_ldr(dst_reg, X17, 0);
        }
      }

      // Type definitions — compile-time only for struct/const,
      // but enum declarations also register pretty-printer
      // metadata so `show(Loot::Gold(...))` can emit
      // `Loot::Gold(...)` instead of leaking a raw pointer.
      Insn::EnumDef {
        name,
        ty_id,
        variants,
        ..
      } => {
        self.register_enum_meta(*name, *ty_id, variants);
      }
      Insn::StructDef { .. }
      | Insn::ConstDef { .. }
      | Insn::ArrayTyDef { .. } => {}

      // Enum construction: for unit variants (no fields),
      // the value is just the discriminant. For tuple
      // variants, allocate [tag, f0, f1, ...] on stack.
      // Enum construction: every variant (unit or tuple) now
      // lowers to a pointer into the stack struct area holding
      // `[tag, f0, f1, ...]`. Unit variants allocate a single
      // `[tag]` slot; tuple variants allocate `[tag, f0, ...]`.
      // Uniform representation means every enum value in a
      // register is a pointer, so `is_enum_value` can safely
      // include `Load` / `ArrayIndex` and `BinOp::Eq`/`Neq` on
      // enum operands can deref both sides to compare tags.
      // Cost: one extra stack slot + one store per unit variant
      // instance — dwarfed by the syscall cost of `show`.
      Insn::EnumConstruct {
        variant, fields, ..
      } => {
        let slot_count = 1 + fields.len() as u32;
        let base = self.struct_base + self.next_struct_slot;

        // Store discriminant at base.
        self.emitter.emit_mov_imm(X16, *variant as u16);
        self.emitter.emit_str(X16, SP, base as i16);

        // Store fields (if any) at base + (i+1)*8.
        for (i, field) in fields.iter().enumerate() {
          let off = base + (i as u32 + 1) * STACK_SLOT_SIZE;

          if let Some(fp) = self.alloc_fp_reg(*field) {
            self.emitter.emit_str_fp(fp, SP, off as u16);
          } else if let Some(reg) = self.alloc_reg(*field) {
            self.emitter.emit_str(reg, SP, off as i16);
          }
        }

        if let Some(dst) = self.reg_for_insn(idx) {
          self.emitter.emit_add_imm(dst, SP, base as u16);
        }

        self.next_struct_slot += slot_count * STACK_SLOT_SIZE;
      }

      // Struct construction: store fields into
      // pre-allocated frame slots. struct_base +
      // next_struct_slot is this struct's start offset
      // from SP.
      Insn::StructConstruct { fields, .. } => {
        let base = self.struct_base + self.next_struct_slot;

        for (i, field) in fields.iter().enumerate() {
          let off = base + i as u32 * STACK_SLOT_SIZE;

          if let Some(reg) = self.alloc_reg(*field) {
            self.emitter.emit_str(reg, SP, off as i16);
          }
        }

        // Set dst register to point at this struct's
        // base. Use ADD (not MOV) because ARM64 MOV
        // via ORR encodes register 31 as XZR, not SP.
        if let Some(dst) = self.reg_for_insn(idx) {
          self.emitter.emit_add_imm(dst, SP, base as u16);
        }

        self.next_struct_slot += fields.len() as u32 * STACK_SLOT_SIZE;
      }

      // Struct/tuple field access: load from
      // base + index * 8.
      // Tuple construction: same layout as structs.
      // Store each element at pre-allocated frame slots.
      Insn::TupleLiteral { elements, .. } => {
        let base = self.struct_base + self.next_struct_slot;

        for (i, elem) in elements.iter().enumerate() {
          let off = base + i as u32 * STACK_SLOT_SIZE;

          if let Some(reg) = self.alloc_reg(*elem) {
            self.emitter.emit_str(reg, SP, off as i16);
          }
        }

        if let Some(dst) = self.reg_for_insn(idx) {
          self.emitter.emit_add_imm(dst, SP, base as u16);
        }

        self.next_struct_slot += elements.len() as u32 * STACK_SLOT_SIZE;
      }

      Insn::TupleIndex {
        dst,
        tuple,
        index,
        ty_id,
      } => {
        let base_reg = self.alloc_reg(*tuple).unwrap_or(X0);
        let offset = (*index as i16) * (STACK_SLOT_SIZE as i16);
        let is_flt =
          ty_id.0 >= FLOAT_TYPE_ID_MIN && ty_id.0 <= FLOAT_TYPE_ID_MAX;

        if is_flt {
          let fp_dst = self.fp_reg_for_insn(idx).unwrap_or(D0);

          self.emitter.emit_ldr_fp(fp_dst, base_reg, offset as u16);
        } else if let Some(dst_reg) = self.alloc_reg(*dst) {
          self.emitter.emit_ldr(dst_reg, base_reg, offset);
        }
      }

      // Struct field write: STR value to base + index * 8.
      Insn::FieldStore {
        base, index, value, ..
      } => {
        let base_reg = self.alloc_reg(*base).unwrap_or(X0);
        let val_reg = self.alloc_reg(*value).unwrap_or(X1);
        let offset = (*index as i16) * (STACK_SLOT_SIZE as i16);

        self.emitter.emit_str(val_reg, base_reg, offset);
      }

      Insn::Cast {
        dst,
        src,
        from_ty,
        to_ty,
      } => {
        let from = from_ty.0;
        let to = to_ty.0;

        let float_range = FLOAT_TYPE_ID_MIN..=FLOAT_TYPE_ID_MAX;
        let is_from_float = float_range.contains(&from);
        let is_to_float = float_range.contains(&to);

        if is_from_float && !is_to_float {
          // float -> int: FCVTZS Xd, Ds.
          let fp_src = self.alloc_fp_reg(*src).unwrap_or(D0);
          let gp_dst = self.alloc_reg(*dst).unwrap_or(X0);

          self.emitter.emit_fcvtzs(gp_dst, fp_src);
        } else if !is_from_float && is_to_float {
          // int -> float: SCVTF Dd, Xs.
          let gp_src = self.alloc_reg(*src).unwrap_or(X0);
          let fp_dst = self.alloc_fp_reg(*dst).unwrap_or(D0);

          self.emitter.emit_scvtf(fp_dst, gp_src);
        } else {
          // GP -> GP: int/char/bytes/bool are all in GP regs.
          // MOV if different registers, no-op if same.
          let src_reg = self.alloc_reg(*src).unwrap_or(X0);
          let dst_reg = self.alloc_reg(*dst).unwrap_or(X0);

          if src_reg != dst_reg {
            self.emitter.emit_mov_reg(dst_reg, src_reg);
          }
        }
      }

      // === STRUCTURED CONCURRENCY ===
      //
      // Each insn lowers to a `BL` placeholder plus an
      // extern-fixup record naming the runtime symbol.
      // The linker resolves these against
      // `libzo_runtime.dylib`. Arg-register marshaling
      // is minimal here — args already land in X0..X7
      // via the executor's value lowering.
      Insn::ChannelCreate {
        dst,
        elem_ty,
        capacity,
      } => {
        // ABI: `_zo_chan_new(elem_sz, capacity) -> *ZoChan`.
        // X0 = element size in bytes (known at compile
        // time from `elem_ty`), X1 = capacity (literal
        // from the SIR insn). Result pointer lands in
        // X0; `dst` captures the single chan handle.
        // The Tx/Rx distinction is ty-level only — the
        // subsequent TupleLiteral aliases the pointer
        // into two slots so `ch.0` and `ch.1` both
        // read the same runtime handle.
        let elem_sz = self.size_of_ty(*elem_ty);

        self.emit_mov_imm_64(X0, elem_sz as u64);
        self.emit_mov_imm_64(X1, *capacity as u64);

        self.emit_extern_call("_zo_chan_new");

        if let Some(dst_reg) = self.alloc_reg(*dst)
          && dst_reg != X0
        {
          self.emitter.emit_mov_reg(dst_reg, X0);
        }
      }
      Insn::ChannelSend { channel, value, .. } => {
        // ABI: `_zo_chan_send(chan, src: *const u8)`.
        // Values live in registers but the runtime
        // wants a pointer — spill to the scratch slot
        // reserved by the function prologue, pass its
        // address in X1.
        let slot = self.chan_scratch_base as i16;

        if let Some(src_reg) = self.alloc_reg(*value) {
          self.emitter.emit_str(src_reg, SP, slot);
        }

        if let Some(ch_reg) = self.alloc_reg(*channel)
          && ch_reg != X0
        {
          self.emitter.emit_mov_reg(X0, ch_reg);
        }

        self.emitter.emit_add_imm(X1, SP, slot as u16);
        self.emit_extern_call("_zo_chan_send");
      }
      Insn::ChannelRecv { dst, channel, .. } => {
        // ABI: `_zo_chan_recv(chan, dst: *mut u8)`.
        // The runtime writes into the scratch slot;
        // we then load the written value into the
        // destination register the allocator reserved
        // for `dst`.
        let slot = self.chan_scratch_base as i16;

        if let Some(ch_reg) = self.alloc_reg(*channel)
          && ch_reg != X0
        {
          self.emitter.emit_mov_reg(X0, ch_reg);
        }

        self.emitter.emit_add_imm(X1, SP, slot as u16);
        self.emit_extern_call("_zo_chan_recv");

        if let Some(dst_reg) = self.alloc_reg(*dst) {
          self.emitter.emit_ldr(dst_reg, SP, slot);
        }
      }
      Insn::ChannelClose { channel } => {
        // ABI: `_zo_chan_close(chan)`. X0 carries the
        // channel handle. Wakes every parked waiter
        // runtime-side so they observe the closed
        // state on their next loop.
        if let Some(ch_reg) = self.alloc_reg(*channel)
          && ch_reg != X0
        {
          self.emitter.emit_mov_reg(X0, ch_reg);
        }

        self.emit_extern_call("_zo_chan_close");
      }
      Insn::TaskSpawn {
        dst,
        kind,
        callee,
        args,
        ..
      } => {
        // Runtime exposes `_zo_task_spawn_N(callee,
        // arg0, ..., arg(N-1))` for N in 0..=3. Order
        // at the call site is exact C ABI: X0 =
        // callee address, X1 = arg0, X2 = arg1,
        // X3 = arg2. `function_addr_fixups` patches
        // the ADR emitted for the callee once that
        // function's final code offset is known.
        let n_args = args.len().min(3);

        // Args land in X1..X(n). Emit in reverse so a
        // later arg doesn't clobber an earlier arg's
        // source register before it's moved.
        for (i, arg) in args.iter().enumerate().take(3).rev() {
          let dst_reg = Register::new((i + 1) as u8);

          if let Some(src_reg) = self.alloc_reg(*arg)
            && src_reg != dst_reg
          {
            self.emitter.emit_mov_reg(dst_reg, src_reg);
          }
        }

        // Callee address in X0 — the runtime ABI's
        // first parameter.
        let adr_pos = self.emitter.current_offset();

        self.emitter.emit_adr(X0, 0);
        self.function_addr_fixups.push((adr_pos, *callee));

        let runtime_sym = match (kind, n_args) {
          (SpawnKind::Thread, _) => "_zo_task_spawn_thread",
          (SpawnKind::Green, 0) => "_zo_task_spawn",
          (SpawnKind::Green, 1) => "_zo_task_spawn_1",
          (SpawnKind::Green, 2) => "_zo_task_spawn_2",
          _ => "_zo_task_spawn_3",
        };

        self.emit_extern_call(runtime_sym);

        if let Some(dst_reg) = self.alloc_reg(*dst)
          && dst_reg != X0
        {
          self.emitter.emit_mov_reg(dst_reg, X0);
        }
      }
      Insn::TaskAwait { dst, task, .. } => {
        // ABI: `_zo_task_await(task: *ZoTask)` — X0
        // carries the task handle produced by a prior
        // `TaskSpawn`.
        if let Some(src) = self.alloc_reg(*task)
          && src != X0
        {
          self.emitter.emit_mov_reg(X0, src);
        }

        self.emit_extern_call("_zo_task_await");

        if let Some(dst_reg) = self.alloc_reg(*dst)
          && dst_reg != X0
        {
          self.emitter.emit_mov_reg(dst_reg, X0);
        }
      }
      // `nursery { body }` brackets a set of spawned
      // siblings. `NurseryBegin` is a no-op — the
      // scheduler queue is already in place. `NurseryEnd`
      // drains every ready task to completion so the
      // parent doesn't fall through the `}` leaving
      // orphaned green tasks in the queue. `supervise`
      // shares this drain path; the cascade semantics
      // are a runtime-side policy extension.
      Insn::NurseryBegin { .. } => {}
      Insn::NurseryEnd { .. } => {
        self.emit_extern_call("_zo_nursery_drain");
      }

      // Selective receive. Materializes the `chans`
      // array and output buffer in the function's
      // select-scratch area, loads the runtime ABI
      // registers, and calls `_zo_select_wait`. The arm
      // index (X0) lands in `out_which` for the arm
      // dispatch; the received value is read from the
      // scratch buffer by the companion `SelectRecv`
      // insn the executor emits immediately after.
      //
      // ABI: `_zo_select_wait(chans, nchans, out, sz)`.
      // Runtime loops polling each chan; first non-empty
      // wins and writes its value into `out`.
      Insn::SelectWait {
        out_which,
        chans,
        elem_ty,
      } => {
        let nchans = chans.len() as u32;
        let elem_sz = self.size_of_ty(*elem_ty);
        let chans_base = self.select_scratch_base;
        let out_base = chans_base + nchans * 8;

        // Spill each chan operand into the on-stack
        // array slot the runtime will index through.
        for (i, chan_vid) in chans.iter().enumerate() {
          if let Some(src) = self.alloc_reg(*chan_vid) {
            let off = chans_base + i as u32 * 8;

            self.emitter.emit_str(src, SP, off as i16);
          }
        }

        // Zero the out buffer so the post-call LDR
        // reads any bytes the runtime didn't touch as
        // zero instead of stale stack contents. Wider
        // elem types (`elem_sz > 8`) are a later scope;
        // today the ABI tops out at 8-byte scalars and
        // pointer-backed 8-byte handles.
        self.emitter.emit_str(XZR, SP, out_base as i16);

        // X0 = chans_array_ptr.
        self.emitter.emit_add_imm(X0, SP, chans_base as u16);
        // X1 = nchans.
        self.emit_mov_imm_64(X1, nchans as u64);
        // X2 = out_value_ptr.
        self.emitter.emit_add_imm(X2, SP, out_base as u16);
        // X3 = elem_sz.
        self.emit_mov_imm_64(X3, elem_sz as u64);

        self.emit_extern_call("_zo_select_wait");

        // X0 (arm index) → out_which reg.
        if let Some(dst_reg) = self.alloc_reg(*out_which)
          && dst_reg != X0
        {
          self.emitter.emit_mov_reg(dst_reg, X0);
        }
      }

      // Paired with the preceding `SelectWait`: loads
      // the runtime-written value from the scratch
      // buffer into the allocator-assigned `dst`
      // register. Split out so the register allocator
      // sees a single-dst insn per SIR entry.
      Insn::SelectRecv { dst, chans_len, .. } => {
        let off = self.select_scratch_base + chans_len * 8;

        if let Some(dst_reg) = self.alloc_reg(*dst) {
          self.emitter.emit_ldr(dst_reg, SP, off as i16);
        }
      }

      // `t.cancelled()` method on `Task<T>` — reads the
      // shared cancel flag. ABI:
      // `_zo_task_is_cancelled(task) -> bool`, X0 = task
      // handle, X0 out = result.
      Insn::TaskCancelled { dst, task, .. } => {
        if let Some(ch_reg) = self.alloc_reg(*task)
          && ch_reg != X0
        {
          self.emitter.emit_mov_reg(X0, ch_reg);
        }

        self.emit_extern_call("_zo_task_is_cancelled");

        if let Some(dst_reg) = self.alloc_reg(*dst)
          && dst_reg != X0
        {
          self.emitter.emit_mov_reg(dst_reg, X0);
        }
      }

      // `t.cancel()` method on `Task<T>` — latches the
      // shared cancel flag. ABI: `_zo_task_cancel(task)`,
      // X0 = task handle. No result.
      Insn::TaskCancel { task } => {
        if let Some(ch_reg) = self.alloc_reg(*task)
          && ch_reg != X0
        {
          self.emitter.emit_mov_reg(X0, ch_reg);
        }

        self.emit_extern_call("_zo_task_cancel");
      }

      // Runtime `s[lo..hi]` when the bounds aren't
      // compile-time constants. ABI:
      // `_zo_str_slice(src, lo, hi) -> *str`; X0..X2
      // carry the args, result in X0.
      Insn::StrSlice {
        dst, src, lo, hi, ..
      } => {
        if let Some(src_reg) = self.alloc_reg(*src)
          && src_reg != X0
        {
          self.emitter.emit_mov_reg(X0, src_reg);
        }

        if let Some(lo_reg) = self.alloc_reg(*lo)
          && lo_reg != X1
        {
          self.emitter.emit_mov_reg(X1, lo_reg);
        }

        if let Some(hi_reg) = self.alloc_reg(*hi)
          && hi_reg != X2
        {
          self.emitter.emit_mov_reg(X2, hi_reg);
        }

        self.emit_extern_call("_zo_str_slice");

        if let Some(dst_reg) = self.alloc_reg(*dst)
          && dst_reg != X0
        {
          self.emitter.emit_mov_reg(dst_reg, X0);
        }
      }

      _ => {}
    }
  }

  /// Convert X0 (unsigned int) to decimal string on the
  /// stack and write it to file descriptor `fd`.
  ///
  /// Algorithm: repeatedly divide by 10, push ASCII digits
  /// onto a stack buffer in reverse, then write.
  /// Compile-time type dispatch for a single argument
  /// (Graydon style). Emits the appropriate write for
  /// str, int, or float to the given fd.
  fn emit_typed_write(&mut self, arg_vid: Option<ValueId>, fd: u16) {
    let is_str = arg_vid.is_some_and(|v| self.is_string_value(v));
    let is_flt = arg_vid.is_some_and(|v| self.is_float_value(v));
    let is_bool = arg_vid.is_some_and(|v| self.is_bool_value(v));
    let is_char = arg_vid.is_some_and(|v| self.is_char_value(v));
    let enum_ty = arg_vid.and_then(|v| self.is_enum_value(v));

    // Check if the most recent emitted instruction was a
    // math intrinsic (FSQRT, FRINTM, etc.). The result
    // is in D0 and should use the float showln path.
    let is_flt = is_flt || self.last_was_math_intrinsic;

    self.last_was_math_intrinsic = false;

    if is_flt {
      if let Some(fp_src) = arg_vid.and_then(|v| self.alloc_fp_reg(v))
        && fp_src != D0
      {
        self.emitter.emit_fmov_fp(D0, fp_src);
      }

      self.emit_ftoa_and_write(fd);
    } else if is_bool && arg_vid.is_some() {
      if let Some(src) = arg_vid.and_then(|v| self.alloc_reg(v))
        && src != X0
      {
        self.emitter.emit_mov_reg(X0, src);
      }

      self.emit_bool_and_write(fd);
    } else if is_char && arg_vid.is_some() {
      if let Some(src) = arg_vid.and_then(|v| self.alloc_reg(v))
        && src != X0
      {
        self.emitter.emit_mov_reg(X0, src);
      }

      self.emit_char_and_write(fd);
    } else if let Some(ty_id) = enum_ty
      && let Some(vid) = arg_vid
    {
      // Enum scrutinee — dispatch into the pretty-printer
      // rather than leaking the pointer to `itoa`. Every
      // enum value in a register is now a pointer to
      // `[tag, f0, f1, ...]` on the stack (uniform repr);
      // `emit_enum_write` loads the tag and cmp-chains on
      // it.
      self.emit_enum_write(vid, ty_id, fd);
    } else if let Some(elem_ty) = arg_vid.and_then(|v| self.is_array_value(v))
      && let Some(vid) = arg_vid
    {
      // Dynamic array — walk `[len, cap, e0, e1, ...]` and
      // format each element via `emit_field_write`. Without
      // this branch the pointer falls through to `itoa` and
      // prints as a raw address.
      self.emit_array_write(vid, elem_ty, fd);
    } else if !is_str && arg_vid.is_some() {
      if let Some(src) = arg_vid.and_then(|v| self.alloc_reg(v))
        && src != X0
      {
        self.emitter.emit_mov_reg(X0, src);
      }

      self.emit_itoa_and_write(fd);
    } else if is_str {
      // String: single pointer to [len: u64][bytes][null].
      // Read length and data pointer from the struct.
      // Move to X16 first to avoid clobbering if ptr is
      // X1 or X2 (used by syscall args).
      let ptr = arg_vid.and_then(|v| self.alloc_reg(v)).unwrap_or(X1);

      if ptr != X16 {
        self.emitter.emit_mov_reg(X16, ptr);
      }

      // LDR X2, [X16, #0] — load length from struct.
      self.emitter.emit_ldr(X2, X16, 0);
      // ADD X1, X16, #8 — data starts at offset 8.
      self.emitter.emit_add_imm(X1, X16, 8);
      self.emitter.emit_mov_imm(X16, SYS_WRITE);
      self.emitter.emit_mov_imm(X0, fd);
      self.emitter.emit_svc(0);
    } else {
      // No argument — emit empty write.
      self.emitter.emit_mov_imm(X16, SYS_WRITE);
      self.emitter.emit_mov_imm(X0, fd);
      self.emitter.emit_svc(0);
    }
  }

  /// Record `Insn::EnumDef` metadata and pre-bake each
  /// variant's display string (`"EnumName::VariantName"`) into
  /// `string_data` under a synthetic symbol. Same technique as
  /// `emit_bool_and_write`'s "true" / "false" interning, scaled
  /// to one synthetic symbol per enum variant. Called once per
  /// enum at codegen time.
  fn register_enum_meta(
    &mut self,
    name: Symbol,
    ty_id: TyId,
    variants: &[(Symbol, u32, Vec<TyId>)],
  ) {
    if self.enum_metas.contains_key(&ty_id.0) {
      return;
    }

    let enum_str = self.interner.get(name).to_owned();
    let mut variant_metas = Vec::with_capacity(variants.len());

    for (vname, disc, fields) in variants {
      let var_str = self.interner.get(*vname);
      let display = format!("{enum_str}::{var_str}");

      let display_sym = Symbol(self.next_enum_sym);

      self.next_enum_sym += 1;

      let mut buf = Buffer::new();
      let bytes = display.as_bytes();

      buf.bytes(&(bytes.len() as u64).to_le_bytes());
      buf.bytes(bytes);
      buf.bytes(b"\0");

      self.string_data.push((display_sym, buf.finish()));

      variant_metas.push(VariantMeta {
        discriminant: *disc,
        field_tys: fields.clone(),
        display_sym,
      });
    }

    let any_tuple = variant_metas.iter().any(|v| !v.field_tys.is_empty());

    if any_tuple {
      self.register_punctuation_sym(ENUM_OPEN_PAREN_SYM, b"(");
      self.register_punctuation_sym(ENUM_COMMA_SPACE_SYM, b", ");
      self.register_punctuation_sym(ENUM_CLOSE_PAREN_SYM, b")");
    }

    self.enum_metas.insert(
      ty_id.0,
      EnumMeta {
        variants: variant_metas,
      },
    );
  }

  fn register_punctuation_sym(&mut self, sym: Symbol, text: &[u8]) {
    if self.string_data.iter().any(|(s, _)| *s == sym) {
      return;
    }

    let mut buf = Buffer::new();

    buf.bytes(&(text.len() as u64).to_le_bytes());
    buf.bytes(text);
    buf.bytes(b"\0");

    self.string_data.push((sym, buf.finish()));
  }

  /// Pretty-print an enum value. Unit variants leave the
  /// discriminant directly in the allocated register; tuple
  /// variants leave a pointer to `[tag, f0, f1, ...]` in the
  /// register. We can't statically tell the two shapes apart
  /// per call site, so we lower both through a shared cmp-chain
  /// on the discriminant. For tuple variants, the discriminant
  /// is fetched via `LDR [ptr, #0]`; for unit variants, the
  /// register already holds the discriminant. Since we
  /// dispatch by tag value and every enum's discriminants are
  /// densely packed, the cmp-chain is correct for both shapes
  /// as long as unit variants never share their discriminant
  /// with a tuple variant — which is guaranteed because unit
  /// and tuple variants inside one enum share one discriminant
  /// namespace.
  ///
  /// Payload printing is deferred to a follow-up: v1 just
  /// emits the variant name (`"Loot::Gold"`), not the fields.
  /// Users who need the fields access them individually until
  /// the recursive field print lands.
  fn emit_enum_write(&mut self, vid: ValueId, ty_id: TyId, fd: u16) {
    let Some(meta) = self.enum_metas.get(&ty_id.0) else {
      if let Some(src) = self.alloc_reg(vid)
        && src != X0
      {
        self.emitter.emit_mov_reg(X0, src);
      }
      self.emit_itoa_and_write(fd);
      return;
    };

    let variants: Vec<(u32, Symbol, Vec<TyId>)> = meta
      .variants
      .iter()
      .map(|v| (v.discriminant, v.display_sym, v.field_tys.clone()))
      .collect();

    let src = self.alloc_reg(vid).unwrap_or(X0);

    // Save enum pointer in X19 (callee-saved, outside
    // allocator pool) so it survives write syscalls.
    self.emitter.emit_mov_reg(Register::new(19), src);
    self.emitter.emit_ldr(X17, src, 0);

    let mut done_fixups: Vec<usize> = Vec::with_capacity(variants.len());

    for (disc, display_sym, field_tys) in &variants {
      self.emitter.emit_cmp_imm(X17, *disc as u16);

      let bne_pos = self.emitter.current_offset();
      self.emitter.emit_bne(0);

      self.emit_synthetic_str_write(*display_sym, fd);

      if !field_tys.is_empty() {
        self.emit_synthetic_str_write(ENUM_OPEN_PAREN_SYM, fd);

        for (i, field_ty) in field_tys.iter().enumerate() {
          let offset = ((i as i16) + 1) * STACK_SLOT_SIZE as i16;

          self.emitter.emit_ldr(X0, Register::new(19), offset);

          self.emit_field_write(*field_ty, fd);

          if i + 1 < field_tys.len() {
            self.emit_synthetic_str_write(ENUM_COMMA_SPACE_SYM, fd);
          }
        }

        self.emit_synthetic_str_write(ENUM_CLOSE_PAREN_SYM, fd);
      }

      let done_pos = self.emitter.current_offset();
      self.emitter.emit_b(0);
      done_fixups.push(done_pos as usize);

      let after_body = self.emitter.current_offset() as i32;
      self
        .emitter
        .patch_bcond_at(bne_pos as usize, after_body - bne_pos as i32);
    }

    let done_label = self.emitter.current_offset() as i32;

    for pos in done_fixups {
      self.emitter.patch_b_at(pos, done_label - pos as i32);
    }
  }

  fn emit_synthetic_str_write(&mut self, sym: Symbol, fd: u16) {
    let adr_pos = self.emitter.current_offset();

    self.string_fixups.push((adr_pos, sym));
    self.emitter.emit_adr(X16, 0);
    self.emitter.emit_ldr(X2, X16, 0);
    self.emitter.emit_add_imm(X1, X16, 8);
    self.emitter.emit_mov_imm(X16, SYS_WRITE);
    self.emitter.emit_mov_imm(X0, fd);
    self.emitter.emit_svc(0);
  }

  fn emit_field_write(&mut self, ty_id: TyId, fd: u16) {
    let is_str = ty_id.0 == STR_TYPE_ID;
    let is_float = ty_id.0 >= FLOAT_TYPE_ID_MIN && ty_id.0 <= FLOAT_TYPE_ID_MAX;
    let is_bool = ty_id.0 == BOOL_TYPE_ID;
    let is_char = ty_id.0 == CHAR_TYPE_ID;

    if is_str {
      self.emitter.emit_ldr(X2, X0, 0);
      self.emitter.emit_add_imm(X1, X0, 8);
      self.emitter.emit_mov_imm(X16, SYS_WRITE);
      self.emitter.emit_mov_imm(X0, fd);
      self.emitter.emit_svc(0);
    } else if is_float {
      self.emitter.emit_fmov_gp_to_fp(D0, X0);
      self.emit_ftoa_and_write(fd);
    } else if is_bool {
      self.emit_bool_and_write(fd);
    } else if is_char {
      self.emit_char_and_write(fd);
    } else {
      self.emit_itoa_and_write(fd);
    }
  }

  /// Pretty-print a dynamic array value as `[e0, e1, ...]`.
  ///
  /// Layout (from `Insn::ArrayLiteral` codegen):
  /// `[len:u64][cap:u64][e0:u64][e1:u64]...`. Elements are
  /// uniform 8-byte slots regardless of element type — ints
  /// as-is, floats reinterpreted, strings/arrays as pointers.
  ///
  /// The loop keeps state in callee-saved regs so write
  /// syscalls (which trash X0-X17) don't lose it:
  /// - X19 = array base pointer
  /// - X20 = length
  /// - X21 = index `i`
  ///
  /// Element dispatch reuses `emit_field_write` — same
  /// per-type branching as the enum pretty-printer uses for
  /// payload fields.
  fn emit_array_write(&mut self, vid: ValueId, elem_ty: TyId, fd: u16) {
    let src = self.alloc_reg(vid).unwrap_or(X0);

    let r_base = Register::new(19);
    let r_len = Register::new(20);
    let r_idx = Register::new(21);
    let r_tmp = Register::new(22);

    self.register_punctuation_sym(ARRAY_OPEN_BRACKET_SYM, b"[");
    self.register_punctuation_sym(ARRAY_CLOSE_BRACKET_SYM, b"]");
    self.register_punctuation_sym(ENUM_COMMA_SPACE_SYM, b", ");

    // Save array base in X19, load length into X20, zero i.
    self.emitter.emit_mov_reg(r_base, src);
    self.emitter.emit_ldr(r_len, r_base, 0);
    self.emitter.emit_mov_imm(r_idx, 0);

    // Opening bracket.
    self.emit_synthetic_str_write(ARRAY_OPEN_BRACKET_SYM, fd);

    // loop_start:
    let loop_start = self.emitter.current_offset();

    // CMP X21, X20; B.GE loop_end (patch later).
    self.emitter.emit_cmp(r_idx, r_len);

    let bge_pos = self.emitter.current_offset();

    self.emitter.emit_bge(0);

    // CBZ X21, skip_sep (patch later) — if i == 0, don't
    // emit a leading ", ".
    let cbz_pos = self.emitter.current_offset();

    self.emitter.emit_cbz(r_idx, 0);

    self.emit_synthetic_str_write(ENUM_COMMA_SPACE_SYM, fd);

    // skip_sep:
    let skip_sep = self.emitter.current_offset() as i32;

    self
      .emitter
      .patch_cbz_at(cbz_pos as usize, (skip_sep - cbz_pos as i32) >> 2);

    // X22 = i * 8; X22 += 16; X22 += base.
    self.emitter.emit_lsl(r_tmp, r_idx, 3);
    self.emitter.emit_add_imm(r_tmp, r_tmp, ARRAY_HEADER_SIZE);
    self.emitter.emit_add(r_tmp, r_base, r_tmp);
    // LDR X0, [X22].
    self.emitter.emit_ldr(X0, r_tmp, 0);

    // Dispatch on element type.
    self.emit_field_write(elem_ty, fd);

    // i++, B loop_start.
    self.emitter.emit_add_imm(r_idx, r_idx, 1);

    let back_pos = self.emitter.current_offset() as i32;

    self.emitter.emit_b(loop_start as i32 - back_pos);

    // loop_end:
    let loop_end = self.emitter.current_offset() as i32;

    self
      .emitter
      .patch_bcond_at(bge_pos as usize, loop_end - bge_pos as i32);

    // Closing bracket.
    self.emit_synthetic_str_write(ARRAY_CLOSE_BRACKET_SYM, fd);
  }

  /// Emit a newline write to the given fd.
  fn emit_newline(&mut self, fd: u16) {
    self.emitter.emit_mov_imm(X1, ASCII_NEWLINE);
    self.emitter.emit_sub_imm(X2, SP, NEWLINE_BUFFER_OFFSET);
    self.emitter.emit_strb(X1, X2, 0);
    self.emitter.emit_mov_reg(X1, X2);
    self.emitter.emit_mov_imm(X2, 1);
    self.emitter.emit_mov_imm(X16, SYS_WRITE);
    self.emitter.emit_mov_imm(X0, fd);
    self.emitter.emit_svc(0);
  }

  /// Write a single char (in X0) to fd.
  /// Stores the byte to a stack scratch slot, then writes 1
  /// byte via SYS_WRITE. Same technique as emit_newline.
  fn emit_char_and_write(&mut self, fd: u16) {
    // Store low byte of X0 to scratch slot on stack.
    self.emitter.emit_sub_imm(X2, SP, NEWLINE_BUFFER_OFFSET);
    self.emitter.emit_strb(X0, X2, 0);
    // X1 = pointer to the byte, X2 = length 1.
    self.emitter.emit_mov_reg(X1, X2);
    self.emitter.emit_mov_imm(X2, 1);
    self.emitter.emit_mov_imm(X16, SYS_WRITE);
    self.emitter.emit_mov_imm(X0, fd);
    self.emitter.emit_svc(0);
  }

  /// Convert D0 (double) to decimal string and write to fd.
  ///
  /// Strategy: print integer part, ".", then 6 fractional
  /// digits. Handles negative by printing "-" prefix.
  fn emit_ftoa_and_write(&mut self, fd: u16) {
    // FCVTZS X0, D0 — integer part (truncated).
    self.emitter.emit_fcvtzs(X0, D0);

    // Print the integer part via itoa.
    self.emit_itoa_and_write(fd);

    // Print "."
    self.emitter.emit_sub_imm(SP, SP, NEWLINE_BUFFER_OFFSET);
    self.emitter.emit_mov_imm(X1, b'.' as u16);
    self.emitter.emit_strb(X1, SP, 0);
    // ADD X1, SP, #0 (can't use MOV for SP — XZR alias).
    self.emitter.emit_add_imm(X1, SP, 0);
    self.emitter.emit_mov_imm(X2, 1);
    self.emitter.emit_mov_imm(X16, SYS_WRITE);
    self.emitter.emit_mov_imm(X0, fd);
    self.emitter.emit_svc(0);
    self.emitter.emit_add_imm(SP, SP, NEWLINE_BUFFER_OFFSET);

    // Compute fractional part: frac = (D0 - int_part) * 1e6.
    // Reload D0's integer part into X0, convert back to D1.
    self.emitter.emit_fcvtzs(X0, D0);
    self.emitter.emit_scvtf(D1, X0);
    // D0 = D0 - D1 (fractional part, 0.0 to 0.999...)
    self.emitter.emit_fsub(D0, D0, D1);

    // Multiply by 1000000 to get 6 decimal digits.
    // Load 1000000.0 into D1 via GP.
    let million_bits = 1_000_000.0f64.to_bits();

    self
      .emitter
      .emit_mov_imm(X0, (million_bits & 0xFFFF) as u16);
    self
      .emitter
      .emit_movk(X0, ((million_bits >> 16) & 0xFFFF) as u16, 16);
    self
      .emitter
      .emit_movk(X0, ((million_bits >> 32) & 0xFFFF) as u16, 32);
    self
      .emitter
      .emit_movk(X0, ((million_bits >> 48) & 0xFFFF) as u16, 48);

    self.emitter.emit_fmov_gp_to_fp(D1, X0);

    // D0 = frac * 1000000.0
    self.emitter.emit_fmul(D0, D0, D1);
    // X0 = int(D0)
    self.emitter.emit_fcvtzs(X0, D0);

    // Print 6 digits via itoa.
    self.emit_itoa_and_write(fd);
  }

  /// Emit bool-to-string write: prints "true" or "false".
  /// X0 holds the bool value (0 or 1).
  fn emit_bool_and_write(&mut self, fd: u16) {
    let sym_true = Symbol(0xFFFD);
    let sym_false = Symbol(0xFFFC);

    // Register "true" and "false" string data once.
    if !self.string_data.iter().any(|(s, _)| *s == sym_true) {
      let mut buf = Buffer::new();
      let len = 4u64; // "true".len()

      buf.bytes(&len.to_le_bytes());
      buf.bytes(b"true");
      buf.bytes(b"\0");

      self.string_data.push((sym_true, buf.finish()));
    }

    if !self.string_data.iter().any(|(s, _)| *s == sym_false) {
      let mut buf = Buffer::new();
      let len = 5u64; // "false".len()

      buf.bytes(&len.to_le_bytes());
      buf.bytes(b"false");
      buf.bytes(b"\0");

      self.string_data.push((sym_false, buf.finish()));
    }

    // CBZ X0, false_path — if 0, print "false".
    let cbz_pos = self.emitter.current_offset();

    self.emitter.emit_cbz(X0, 0);

    // True path: ADR X16 -> "true" string.
    let true_fixup = self.emitter.current_offset();

    self.string_fixups.push((true_fixup, sym_true));
    self.emitter.emit_adr(X16, 0);
    // B past the false path ADR (skip 1 instruction = 4).
    self.emitter.emit_b(8);

    // False path.
    let false_start = self.emitter.current_offset();
    let cbz_offset = (false_start as i32 - cbz_pos as i32) >> 2;

    self.emitter.patch_cbz_at(cbz_pos as usize, cbz_offset);

    let false_fixup = self.emitter.current_offset();

    self.string_fixups.push((false_fixup, sym_false));
    self.emitter.emit_adr(X16, 0);

    // Merge: unpack string struct and write.
    self.emitter.emit_ldr(X2, X16, 0);
    self.emitter.emit_add_imm(X1, X16, 8);
    self.emitter.emit_mov_imm(X16, SYS_WRITE);
    self.emitter.emit_mov_imm(X0, fd);
    self.emitter.emit_svc(0);
  }

  fn emit_itoa_and_write(&mut self, fd: u16) {
    self.emitter.emit_sub_imm(SP, SP, ITOA_BUFFER_SIZE);

    // X1 = end of buffer (write pointer, works backward).
    self.emitter.emit_add_imm(X1, SP, ITOA_BUFFER_END);
    // X2 = 0 (length counter).
    self.emitter.emit_mov_imm(X2, 0);

    // Handle negative signed integers: if MSB set,
    // negate X0 and set X17 = 1 (neg flag) for later.
    self.emitter.emit_mov_imm(X17, 0);
    // CMP X0, #0 — sets flags.
    self.emitter.emit_cmp_imm(X0, 0);
    // B.GE +12 — skip the 3-insn neg block (3*4=12 bytes).
    self.emitter.emit_bge(12);
    // Negate: X0 = 0 - X0.
    self.emitter.emit_sub(X0, XZR, X0);
    // Mark as negative.
    self.emitter.emit_mov_imm(X17, 1);

    // X3 = 10 (divisor).
    let x3 = Register::new(3);

    self.emitter.emit_mov_imm(x3, ASCII_NEWLINE);

    let loop_start = self.emitter.current_offset();

    // X4 = X0 / 10.
    let x4 = Register::new(4);
    let x5 = Register::new(5);

    self.emitter.emit_udiv(x4, X0, x3);
    // X5 = X0 - X4 * 10 (remainder = digit).
    self.emitter.emit_msub(x5, x4, x3, X0);
    // X5 += '0'.
    self.emitter.emit_add_imm(x5, x5, ASCII_ZERO);
    // Store byte at [X1], X1 -= 1.
    self.emitter.emit_strb_post_dec(x5, X1);
    // X2 += 1 (length).
    self.emitter.emit_add_imm(X2, X2, 1);
    // X0 = quotient.
    self.emitter.emit_mov_reg(X0, x4);
    // If X0 != 0, loop.
    let cbnz_offset = loop_start as i32 - self.emitter.current_offset() as i32;

    self.emitter.emit_cbnz(X0, cbnz_offset);

    // X1 points one past the first digit — adjust.
    self.emitter.emit_add_imm(X1, X1, 1);

    // If negative (X17 == 1), prepend '-' before first digit.
    self.emitter.emit_cmp_imm(X17, 0);
    // B.EQ +20 — skip the 4-insn sign block (4*4+4=20).
    self.emitter.emit_beq(20);
    let x5 = Register::new(5);
    self.emitter.emit_mov_imm(x5, b'-' as u16);
    self.emitter.emit_sub_imm(X1, X1, 1);
    // Store '-' at X1 (the position before the first digit).
    self.emitter.emit_strb(x5, X1, 0);
    self.emitter.emit_add_imm(X2, X2, 1);

    // Write syscall: write(fd, X1, X2).
    self.emitter.emit_mov_imm(X16, SYS_WRITE);
    self.emitter.emit_mov_imm(X0, fd);
    self.emitter.emit_svc(0);

    // Restore stack.
    self.emitter.emit_add_imm(SP, SP, ITOA_BUFFER_SIZE);
  }

  /// Emit check(bool) — if X0 == 0, write
  /// "check failed\n" to stderr and exit(1).
  /// Runtime string concatenation: `a ++ b`.
  ///
  /// Both operands are pointers to `[len:u64][bytes][null]`.
  /// Result is a new string on the stack with the combined
  /// content. SP is permanently lowered (cleaned up by the
  /// function epilogue).
  fn emit_str_concat(&mut self, dst: Register, lhs: Register, rhs: Register) {
    let x3 = Register::new(3);
    let x4 = Register::new(4);
    let x5 = Register::new(5);

    // Load lengths: X16 = len_a, X17 = len_b.
    self.emitter.emit_ldr(X16, lhs, 0);
    self.emitter.emit_ldr(X17, rhs, 0);

    // X3 = total = len_a + len_b.
    self.emitter.emit_add(x3, X16, X17);

    // Allocate: 8 (header) + total + 1 (null), aligned
    // to 16. Use fixed over-allocation: round up by adding
    // 24 (8+1+15) then masking. We use ADD+AND via two
    // X4 = (total + 9 + 15) & ~15 — 16-byte aligned.
    self.emitter.emit_add_imm(x4, x3, 24);
    self.emitter.emit_and_align16(x4, x4);
    self.emitter.emit_sub_ext(SP, SP, x4);

    // Store combined length at [SP + 0].
    self.emitter.emit_str(x3, SP, 0);

    // Copy bytes from lhs: src = lhs + 8, dst = SP + 8.
    self.emitter.emit_add_imm(x4, SP, 8);
    self.emitter.emit_add_imm(x5, lhs, 8);
    // X16 = len_a (counter).

    let copy1_loop = self.emitter.current_offset();

    self.emitter.emit_cbz(X16, 0);
    let cbz1_pos = self.emitter.current_offset() - 4;

    self.emitter.emit_ldrb(X17, x5, 0);
    self.emitter.emit_strb(X17, x4, 0);
    self.emitter.emit_add_imm(x4, x4, 1);
    self.emitter.emit_add_imm(x5, x5, 1);
    self.emitter.emit_sub_imm(X16, X16, 1);

    let back1 = copy1_loop as i32 - self.emitter.current_offset() as i32;

    self.emitter.emit_b(back1);

    // Patch CBZ to skip past the loop.
    let after1 = self.emitter.current_offset();
    let skip1 = (after1 as i32 - cbz1_pos as i32) >> 2;

    self.emitter.patch_cbz_at(cbz1_pos as usize, skip1);

    // Copy bytes from rhs.
    self.emitter.emit_add_imm(x5, rhs, 8);
    self.emitter.emit_ldr(X16, rhs, 0);

    let copy2_loop = self.emitter.current_offset();

    self.emitter.emit_cbz(X16, 0);
    let cbz2_pos = self.emitter.current_offset() - 4;

    self.emitter.emit_ldrb(X17, x5, 0);
    self.emitter.emit_strb(X17, x4, 0);
    self.emitter.emit_add_imm(x4, x4, 1);
    self.emitter.emit_add_imm(x5, x5, 1);
    self.emitter.emit_sub_imm(X16, X16, 1);

    let back2 = copy2_loop as i32 - self.emitter.current_offset() as i32;

    self.emitter.emit_b(back2);

    // Patch CBZ to skip past the loop.
    let after2 = self.emitter.current_offset();
    let skip2 = (after2 as i32 - cbz2_pos as i32) >> 2;

    self.emitter.patch_cbz_at(cbz2_pos as usize, skip2);

    // Null terminator.
    self.emitter.emit_mov_imm(X16, 0);
    self.emitter.emit_strb(X16, x4, 0);

    // Result pointer.
    self.emitter.emit_add_imm(dst, SP, 0);
  }

  // ================================================================
  // IO builtins — ARM64 syscall implementations.
  // macOS convention: carry flag set = error, X0 = errno.
  // ================================================================

  /// `exists(path: str) -> bool` — access(path, F_OK).
  fn emit_io_exists(&mut self, args: &[ValueId], idx: usize) {
    let path = args.first().and_then(|v| self.alloc_reg(*v)).unwrap_or(X0);

    self.emitter.emit_add_imm(X0, path, 8);
    self.emitter.emit_mov_imm(X1, 0);
    self.emitter.emit_mov_imm(X16, SYS_ACCESS);
    self.emitter.emit_svc(0);

    if let Some(dst) = self.reg_for_insn(idx) {
      self.emitter.emit_cmp_imm(X0, 0);
      self.emitter.emit_cset(dst, COND_EQ);
    }
  }

  /// `read_file(path: str) -> Result<str, int>`
  /// open → read → close → construct Result on stack.
  fn emit_io_read_file(&mut self, args: &[ValueId], idx: usize) {
    let path = args.first().and_then(|v| self.alloc_reg(*v)).unwrap_or(X0);
    let result_base = self.struct_base + self.next_struct_slot;

    // Stack layout (relative to result_base):
    //   [0]  Result tag
    //   [1]  Result field (str ptr or errno)
    //   [2]  scratch: saved bytes_read
    //   [3]  string length prefix
    //   [4+] string bytes + null
    let scratch_off = result_base + 2 * STACK_SLOT_SIZE;
    let str_base = result_base + 3 * STACK_SLOT_SIZE;
    let buf_off = str_base + STACK_SLOT_SIZE;

    // --- open ---
    self.emitter.emit_add_imm(X0, path, 8);
    self.emitter.emit_mov_imm(X1, O_READ_ONLY);
    self.emitter.emit_mov_imm(X2, 0);
    self.emitter.emit_mov_imm(X16, SYS_OPEN);
    self.emitter.emit_svc(0);

    let open_err_pos = self.emitter.current_offset();
    self.emitter.emit_bcs(0);

    // --- read ---
    self.emitter.emit_mov_reg(X17, X0);
    self.emitter.emit_mov_reg(X0, X17);
    self.emit_add_sp_offset(X1, buf_off);
    self.emitter.emit_mov_imm(X2, READ_FILE_BUF_SIZE);
    self.emitter.emit_mov_imm(X16, SYS_READ);
    self.emitter.emit_svc(0);
    self.emit_str_sp(X0, scratch_off);

    // --- close ---
    self.emitter.emit_mov_reg(X0, X17);
    self.emitter.emit_mov_imm(X16, SYS_CLOSE);
    self.emitter.emit_svc(0);

    self.emit_ldr_sp(X2, scratch_off);

    // --- construct Result::Ok(str) ---
    self.emit_str_sp(X2, str_base);
    self.emit_add_sp_offset(X0, buf_off);
    self.emitter.emit_add(X0, X0, X2);
    self.emitter.emit_strb(XZR, X0, 0);
    self.emit_str_sp(XZR, result_base);
    self.emit_add_sp_offset(X0, str_base);
    self.emit_str_sp(X0, result_base + STACK_SLOT_SIZE);

    let ok_done_pos = self.emitter.current_offset();
    self.emitter.emit_b(0);

    // --- error path ---
    let err_label = self.emitter.current_offset();
    self.emitter.patch_bcond_at(
      open_err_pos as usize,
      err_label as i32 - open_err_pos as i32,
    );
    self.emitter.emit_mov_imm(X16, 1);
    self.emit_str_sp(X16, result_base);
    self.emit_str_sp(X0, result_base + STACK_SLOT_SIZE);

    // --- merge ---
    let done_label = self.emitter.current_offset();
    self
      .emitter
      .patch_b_at(ok_done_pos as usize, done_label as i32 - ok_done_pos as i32);

    if let Some(dst) = self.reg_for_insn(idx) {
      self.emit_add_sp_offset(dst, result_base);
    }

    let total_slots =
      2 + 1 + 1 + (READ_FILE_BUF_SIZE as u32 + 8) / STACK_SLOT_SIZE;
    self.next_struct_slot += total_slots * STACK_SLOT_SIZE;
  }

  /// `write_file(path, content) -> Result<int, int>`
  fn emit_io_write_file(&mut self, args: &[ValueId], idx: usize) {
    self.emit_io_write_impl(args, idx, O_WRITE_ONLY_CREATE_TRUNCATE);
  }

  /// `append_file(path, content) -> Result<int, int>`
  fn emit_io_append_file(&mut self, args: &[ValueId], idx: usize) {
    self.emit_io_write_impl(args, idx, O_WRITE_ONLY_CREATE_APPEND);
  }

  /// Shared write implementation.
  fn emit_io_write_impl(
    &mut self,
    args: &[ValueId],
    idx: usize,
    open_flags: u16,
  ) {
    let path = args.first().and_then(|v| self.alloc_reg(*v)).unwrap_or(X0);
    let content = args.get(1).and_then(|v| self.alloc_reg(*v)).unwrap_or(X1);
    let result_base = self.struct_base + self.next_struct_slot;

    // Save content pointer before open clobbers regs.
    self.emit_str_sp(content, result_base + 4 * STACK_SLOT_SIZE);

    // --- open ---
    self.emitter.emit_add_imm(X0, path, 8);
    self.emitter.emit_mov_imm(X1, open_flags);
    self.emitter.emit_mov_imm(X2, FILE_MODE_644);
    self.emitter.emit_mov_imm(X16, SYS_OPEN);
    self.emitter.emit_svc(0);

    let open_err_pos = self.emitter.current_offset();
    self.emitter.emit_bcs(0);

    // --- write ---
    self.emitter.emit_mov_reg(X17, X0);
    // Reload saved content pointer.
    self.emit_ldr_sp(X1, result_base + 4 * STACK_SLOT_SIZE);
    self.emitter.emit_ldr(X2, X1, 0);
    self.emitter.emit_add_imm(X1, X1, 8);
    self.emitter.emit_mov_reg(X0, X17);
    self.emitter.emit_mov_imm(X16, SYS_WRITE);
    self.emitter.emit_svc(0);
    self.emitter.emit_mov_reg(X2, X0);

    // --- close ---
    self.emitter.emit_mov_reg(X0, X17);
    self.emitter.emit_mov_imm(X16, SYS_CLOSE);
    self.emitter.emit_svc(0);

    // --- Ok path ---
    self.emit_str_sp(XZR, result_base);
    self.emit_str_sp(X2, result_base + STACK_SLOT_SIZE);
    let ok_done_pos = self.emitter.current_offset();
    self.emitter.emit_b(0);

    // --- Err path ---
    let err_label = self.emitter.current_offset();
    self.emitter.patch_bcond_at(
      open_err_pos as usize,
      err_label as i32 - open_err_pos as i32,
    );
    self.emitter.emit_mov_imm(X16, 1);
    self.emit_str_sp(X16, result_base);
    self.emit_str_sp(X0, result_base + STACK_SLOT_SIZE);

    // --- merge ---
    let done_label = self.emitter.current_offset();
    self
      .emitter
      .patch_b_at(ok_done_pos as usize, done_label as i32 - ok_done_pos as i32);

    if let Some(dst) = self.reg_for_insn(idx) {
      self.emit_add_sp_offset(dst, result_base);
    }

    self.next_struct_slot += 5 * STACK_SLOT_SIZE;
  }

  fn emit_check_fail(&mut self) {
    // CBNZ X0, +ok_label → if true, skip fail.
    // Use code offset as unique label ID (won't
    // collide with SIR labels which are sequential
    // from 0).
    let ok_label = 0x80000000 | self.emitter.current_offset();

    self
      .branch_fixups
      .push((self.emitter.current_offset(), ok_label));

    self.emitter.emit_cbnz(X0, 0);

    // Fail path: write "check failed\n" to stderr.
    let msg = b"check failed\n";
    let msg_sym = Symbol(0xFFFE);

    // Only push string data once.
    if !self.string_data.iter().any(|(s, _)| *s == msg_sym) {
      let mut buf = Buffer::new();
      let len = msg.len() as u64;

      buf.bytes(&len.to_le_bytes());
      buf.bytes(msg);
      buf.bytes(b"\0");

      self.string_data.push((msg_sym, buf.finish()));
    }

    let fixup_pos = self.emitter.current_offset();

    self.string_fixups.push((fixup_pos, msg_sym));
    // ADR X16 -> string struct, then unpack.
    self.emitter.emit_adr(X16, 0);
    self.emitter.emit_ldr(X2, X16, 0);
    self.emitter.emit_add_imm(X1, X16, 8);
    self.emitter.emit_mov_imm(X16, SYS_WRITE);
    self.emitter.emit_mov_imm(X0, FD_STDERR);
    self.emitter.emit_svc(0);

    // exit(1).
    self.emitter.emit_mov_imm(X16, SYS_EXIT);
    self.emitter.emit_mov_imm(X0, 1);
    self.emitter.emit_svc(0);

    // Ok label: continue execution.
    self.labels.insert(ok_label, self.emitter.current_offset());
  }

  /// `HashMap<K, V>::new()` — emit `BL _zo_map_new`
  /// and stash the returned `ZoMap*` in a freshly
  /// allocated struct slot. The dst register holds the
  /// struct address (`{ ptr }` shape: a single 8-byte
  /// field at offset 0).
  ///
  /// K / V types are inferred from the call's binding
  /// site: `imu m: HashMap<int, int> = HashMap::new();`
  /// — the executor's mono pass propagates the
  /// annotated args through `value_types[dst]` for
  /// later method calls. This handler hardcodes
  /// 4-byte / 4-byte for MVP; mixed-size tables (str
  /// keys, larger values) ride on per-call K/V type
  /// derivation that lives at insert/get time. Future
  /// work threads K/V into the new() handler too.
  fn emit_map_new(&mut self, _args: &[ValueId], idx: usize) {
    // Allocate the struct (one ptr field).
    let struct_base = self.struct_base + self.next_struct_slot;

    self.next_struct_slot += STACK_SLOT_SIZE;

    // _zo_map_new(key_kind=0, key_sz=4, val_sz=4, cap=16).
    self.emitter.emit_mov_imm(X0, 0);
    self.emitter.emit_mov_imm(X1, 4);
    self.emitter.emit_mov_imm(X2, 4);
    self.emitter.emit_mov_imm(X3, 16);
    self.emit_extern_call("_zo_map_new");

    // Store the returned pointer into the struct's ptr
    // slot, hand the struct's address back as dst.
    self.emit_str_sp(X0, struct_base);

    if let Some(dst) = self.reg_for_insn(idx) {
      self.emit_add_sp_offset(dst, struct_base);
    }
  }

  /// `m.insert(k, v)` — spill k and v to scratch, load
  /// `m.ptr` (offset 0 of the struct), call
  /// `_zo_map_insert`. K / V sizes derive from the
  /// arg types at this call site.
  fn emit_map_insert(&mut self, args: &[ValueId], _idx: usize) {
    let recv = args.first().and_then(|v| self.alloc_reg(*v)).unwrap_or(X0);
    let k = args.get(1).and_then(|v| self.alloc_reg(*v)).unwrap_or(X1);
    let v = args.get(2).and_then(|v| self.alloc_reg(*v)).unwrap_or(X2);

    let scratch_base = self.struct_base + self.next_struct_slot;
    let k_off = scratch_base;
    let v_off = scratch_base + STACK_SLOT_SIZE;

    self.next_struct_slot += 2 * STACK_SLOT_SIZE;

    // Spill k and v to their scratch slots.
    self.emit_str_sp(k, k_off);
    self.emit_str_sp(v, v_off);

    // X0 = m.ptr, X1 = &k, X2 = &v.
    self.emitter.emit_ldr(X0, recv, 0);
    self.emit_add_sp_offset(X1, k_off);
    self.emit_add_sp_offset(X2, v_off);
    self.emit_extern_call("_zo_map_insert");
  }

  /// `m.get(k)` — spill k, allocate a value-output
  /// scratch, call `_zo_map_get`, then construct the
  /// `Option<V>` Result-style aggregate the executor
  /// expects on the stack. For MVP the aggregate is a
  /// 2-slot `{ tag, val }` block — `tag = 0` (Some) on
  /// hit, `tag = 1` (None) on miss.
  fn emit_map_get(&mut self, args: &[ValueId], idx: usize) {
    let recv = args.first().and_then(|v| self.alloc_reg(*v)).unwrap_or(X0);
    let k = args.get(1).and_then(|v| self.alloc_reg(*v)).unwrap_or(X1);

    let scratch_base = self.struct_base + self.next_struct_slot;
    let k_off = scratch_base;
    let v_out_off = scratch_base + STACK_SLOT_SIZE;
    let opt_base = scratch_base + 2 * STACK_SLOT_SIZE;

    self.next_struct_slot += 4 * STACK_SLOT_SIZE;

    self.emit_str_sp(k, k_off);
    // Pre-zero v_out (the runtime only writes on hit).
    self.emit_str_sp(XZR, v_out_off);

    self.emitter.emit_ldr(X0, recv, 0);
    self.emit_add_sp_offset(X1, k_off);
    self.emit_add_sp_offset(X2, v_out_off);
    self.emit_extern_call("_zo_map_get");

    // X0 = bool found. Construct Option<V> at opt_base:
    //   tag = found ? 0 : 1
    //   val = *v_out
    self.emitter.emit_mov_imm(X16, 1);
    self.emitter.emit_eor(X16, X16, X0); // X16 = !found
    self.emit_str_sp(X16, opt_base);

    self.emit_ldr_sp(X16, v_out_off);
    self.emit_str_sp(X16, opt_base + STACK_SLOT_SIZE);

    if let Some(dst) = self.reg_for_insn(idx) {
      self.emit_add_sp_offset(dst, opt_base);
    }
  }

  /// `m.contains_key(k)` — spill k, call `_zo_map_
  /// contains`. Returns bool in dst.
  fn emit_map_contains(&mut self, args: &[ValueId], idx: usize) {
    let recv = args.first().and_then(|v| self.alloc_reg(*v)).unwrap_or(X0);
    let k = args.get(1).and_then(|v| self.alloc_reg(*v)).unwrap_or(X1);

    let scratch_base = self.struct_base + self.next_struct_slot;
    let k_off = scratch_base;

    self.next_struct_slot += STACK_SLOT_SIZE;

    self.emit_str_sp(k, k_off);

    self.emitter.emit_ldr(X0, recv, 0);
    self.emit_add_sp_offset(X1, k_off);
    self.emit_extern_call("_zo_map_contains");

    if let Some(dst) = self.reg_for_insn(idx)
      && dst != X0
    {
      self.emitter.emit_mov_reg(dst, X0);
    }
  }

  /// `m.remove(k)` — same shape as `m.get(k)` plus a
  /// runtime-side tombstone. Returns `Option<V>`.
  fn emit_map_remove(&mut self, args: &[ValueId], idx: usize) {
    let recv = args.first().and_then(|v| self.alloc_reg(*v)).unwrap_or(X0);
    let k = args.get(1).and_then(|v| self.alloc_reg(*v)).unwrap_or(X1);

    let scratch_base = self.struct_base + self.next_struct_slot;
    let k_off = scratch_base;
    let v_out_off = scratch_base + STACK_SLOT_SIZE;
    let opt_base = scratch_base + 2 * STACK_SLOT_SIZE;

    self.next_struct_slot += 4 * STACK_SLOT_SIZE;

    self.emit_str_sp(k, k_off);
    self.emit_str_sp(XZR, v_out_off);

    self.emitter.emit_ldr(X0, recv, 0);
    self.emit_add_sp_offset(X1, k_off);
    self.emit_add_sp_offset(X2, v_out_off);
    self.emit_extern_call("_zo_map_remove");

    self.emitter.emit_mov_imm(X16, 1);
    self.emitter.emit_eor(X16, X16, X0);
    self.emit_str_sp(X16, opt_base);

    self.emit_ldr_sp(X16, v_out_off);
    self.emit_str_sp(X16, opt_base + STACK_SLOT_SIZE);

    if let Some(dst) = self.reg_for_insn(idx) {
      self.emit_add_sp_offset(dst, opt_base);
    }
  }

  /// `__zo_map_len_raw(ptr)` — pass-through to
  /// `_zo_map_len`. Caller already loaded the
  /// `*mut ZoMap` into the arg register; we just
  /// route it to X0 and emit the BL.
  fn emit_map_len_raw(&mut self, args: &[ValueId], idx: usize) {
    if let Some(src) = args.first().and_then(|v| self.alloc_reg(*v))
      && src != X0
    {
      self.emitter.emit_mov_reg(X0, src);
    }

    self.emit_extern_call("_zo_map_len");

    if let Some(dst) = self.reg_for_insn(idx)
      && dst != X0
    {
      self.emitter.emit_mov_reg(dst, X0);
    }
  }

  /// `__zo_map_free_raw(ptr)` — pass-through to
  /// `_zo_map_free`. No return value.
  fn emit_map_free_raw(&mut self, args: &[ValueId], _idx: usize) {
    if let Some(src) = args.first().and_then(|v| self.alloc_reg(*v))
      && src != X0
    {
      self.emitter.emit_mov_reg(X0, src);
    }

    self.emit_extern_call("_zo_map_free");
  }

  /// `Vec::new()` — allocate the runtime ZoVec, store its
  /// pointer into the surface struct's only field.
  ///
  /// Element kind is reserved for future use (the
  /// runtime treats slots as opaque bytes today); element
  /// size hardcoded to 8 covers `int`, `str` (pointer),
  /// `char` (zero-extended), `bool` (zero-extended). A
  /// follow-up phase derives both per-call from `$T`.
  fn emit_vec_new(&mut self, _args: &[ValueId], idx: usize) {
    let struct_base = self.struct_base + self.next_struct_slot;

    self.next_struct_slot += STACK_SLOT_SIZE;

    self.emitter.emit_mov_imm(X0, 0);
    self.emitter.emit_mov_imm(X1, 8);
    self.emitter.emit_mov_imm(X2, 8);
    self.emit_extern_call("_zo_vec_new");

    self.emit_str_sp(X0, struct_base);

    if let Some(dst) = self.reg_for_insn(idx) {
      self.emit_add_sp_offset(dst, struct_base);
    }
  }

  /// `v.push(value)` — spill the value to a scratch slot,
  /// load `v.ptr`, call `_zo_vec_push`. The runtime
  /// copies `elem_sz` bytes from the scratch slot.
  fn emit_vec_push(&mut self, args: &[ValueId], _idx: usize) {
    let recv = args.first().and_then(|v| self.alloc_reg(*v)).unwrap_or(X0);
    let v = args.get(1).and_then(|v| self.alloc_reg(*v)).unwrap_or(X1);

    let scratch_base = self.struct_base + self.next_struct_slot;
    let v_off = scratch_base;

    self.next_struct_slot += STACK_SLOT_SIZE;

    self.emit_str_sp(v, v_off);

    self.emitter.emit_ldr(X0, recv, 0);
    self.emit_add_sp_offset(X1, v_off);
    self.emit_extern_call("_zo_vec_push");
  }

  /// `v.pop()` — allocate a value-out scratch slot, call
  /// `_zo_vec_pop`, then build the `Option<T>` aggregate
  /// the executor expects on the stack.
  fn emit_vec_pop(&mut self, args: &[ValueId], idx: usize) {
    let recv = args.first().and_then(|v| self.alloc_reg(*v)).unwrap_or(X0);

    let scratch_base = self.struct_base + self.next_struct_slot;
    let v_out_off = scratch_base;
    let opt_base = scratch_base + STACK_SLOT_SIZE;

    self.next_struct_slot += 3 * STACK_SLOT_SIZE;

    self.emit_str_sp(XZR, v_out_off);

    self.emitter.emit_ldr(X0, recv, 0);
    self.emit_add_sp_offset(X1, v_out_off);
    self.emit_extern_call("_zo_vec_pop");

    self.emitter.emit_mov_imm(X16, 1);
    self.emitter.emit_eor(X16, X16, X0);
    self.emit_str_sp(X16, opt_base);

    self.emit_ldr_sp(X16, v_out_off);
    self.emit_str_sp(X16, opt_base + STACK_SLOT_SIZE);

    if let Some(dst) = self.reg_for_insn(idx) {
      self.emit_add_sp_offset(dst, opt_base);
    }
  }

  /// `v.get(idx)` — call `_zo_vec_get` with `(ptr, idx,
  /// &v_out)`, build the `Option<T>` aggregate.
  fn emit_vec_get(&mut self, args: &[ValueId], idx: usize) {
    let recv = args.first().and_then(|v| self.alloc_reg(*v)).unwrap_or(X0);
    let i = args.get(1).and_then(|v| self.alloc_reg(*v)).unwrap_or(X1);

    let scratch_base = self.struct_base + self.next_struct_slot;
    let v_out_off = scratch_base;
    let opt_base = scratch_base + STACK_SLOT_SIZE;

    self.next_struct_slot += 3 * STACK_SLOT_SIZE;

    self.emit_str_sp(XZR, v_out_off);

    self.emitter.emit_ldr(X0, recv, 0);

    if i != X1 {
      self.emitter.emit_mov_reg(X1, i);
    }

    self.emit_add_sp_offset(X2, v_out_off);
    self.emit_extern_call("_zo_vec_get");

    self.emitter.emit_mov_imm(X16, 1);
    self.emitter.emit_eor(X16, X16, X0);
    self.emit_str_sp(X16, opt_base);

    self.emit_ldr_sp(X16, v_out_off);
    self.emit_str_sp(X16, opt_base + STACK_SLOT_SIZE);

    if let Some(dst) = self.reg_for_insn(idx) {
      self.emit_add_sp_offset(dst, opt_base);
    }
  }

  /// `v.set(idx, value)` — spill `value`, call
  /// `_zo_vec_set` with `(ptr, idx, &v_in)`. Returns the
  /// runtime's `bool` (true on hit, false on OOB).
  fn emit_vec_set(&mut self, args: &[ValueId], idx: usize) {
    let recv = args.first().and_then(|v| self.alloc_reg(*v)).unwrap_or(X0);
    let i = args.get(1).and_then(|v| self.alloc_reg(*v)).unwrap_or(X1);
    let v = args.get(2).and_then(|v| self.alloc_reg(*v)).unwrap_or(X2);

    let scratch_base = self.struct_base + self.next_struct_slot;
    let v_off = scratch_base;

    self.next_struct_slot += STACK_SLOT_SIZE;

    self.emit_str_sp(v, v_off);

    self.emitter.emit_ldr(X0, recv, 0);

    if i != X1 {
      self.emitter.emit_mov_reg(X1, i);
    }

    self.emit_add_sp_offset(X2, v_off);
    self.emit_extern_call("_zo_vec_set");

    if let Some(dst) = self.reg_for_insn(idx)
      && dst != X0
    {
      self.emitter.emit_mov_reg(dst, X0);
    }
  }

  /// `__zo_vec_len_raw(ptr)` — pass-through to
  /// `_zo_vec_len`.
  fn emit_vec_len_raw(&mut self, args: &[ValueId], idx: usize) {
    if let Some(src) = args.first().and_then(|v| self.alloc_reg(*v))
      && src != X0
    {
      self.emitter.emit_mov_reg(X0, src);
    }

    self.emit_extern_call("_zo_vec_len");

    if let Some(dst) = self.reg_for_insn(idx)
      && dst != X0
    {
      self.emitter.emit_mov_reg(dst, X0);
    }
  }

  /// `__zo_vec_free_raw(ptr)` — pass-through to
  /// `_zo_vec_free`.
  fn emit_vec_free_raw(&mut self, args: &[ValueId], _idx: usize) {
    if let Some(src) = args.first().and_then(|v| self.alloc_reg(*v))
      && src != X0
    {
      self.emitter.emit_mov_reg(X0, src);
    }

    self.emit_extern_call("_zo_vec_free");
  }

  /// `HashSet::new()` — allocate a `ZoMap` with
  /// `val_sz = 0`. The runtime stores a zero-length
  /// `Vec<u8>` per slot value; presence is fully encoded
  /// by the slot's occupancy state.
  fn emit_set_new(&mut self, _args: &[ValueId], idx: usize) {
    let struct_base = self.struct_base + self.next_struct_slot;

    self.next_struct_slot += STACK_SLOT_SIZE;

    self.emitter.emit_mov_imm(X0, 0);
    self.emitter.emit_mov_imm(X1, 4);
    self.emitter.emit_mov_imm(X2, 0);
    self.emitter.emit_mov_imm(X3, 16);
    self.emit_extern_call("_zo_map_new");

    self.emit_str_sp(X0, struct_base);

    if let Some(dst) = self.reg_for_insn(idx) {
      self.emit_add_sp_offset(dst, struct_base);
    }
  }

  /// `s.insert(k)` — spill `k`, call `_zo_map_insert`
  /// with a null val pointer (val_sz is 0, so the
  /// runtime never dereferences). Returns
  /// `true` if the key was new — derived by checking
  /// whether `_zo_map_contains` was false BEFORE the
  /// insert. The simplest path is to call contains
  /// first, then unconditional insert.
  fn emit_set_insert(&mut self, args: &[ValueId], idx: usize) {
    let recv = args.first().and_then(|v| self.alloc_reg(*v)).unwrap_or(X0);
    let k = args.get(1).and_then(|v| self.alloc_reg(*v)).unwrap_or(X1);

    let scratch_base = self.struct_base + self.next_struct_slot;
    let k_off = scratch_base;
    let was_new_off = scratch_base + STACK_SLOT_SIZE;

    self.next_struct_slot += 2 * STACK_SLOT_SIZE;

    self.emit_str_sp(k, k_off);

    // Probe first: was the key absent?
    self.emitter.emit_ldr(X0, recv, 0);
    self.emit_add_sp_offset(X1, k_off);
    self.emit_extern_call("_zo_map_contains");

    // !contains == was_new.
    self.emitter.emit_mov_imm(X16, 1);
    self.emitter.emit_eor(X16, X16, X0);
    self.emit_str_sp(X16, was_new_off);

    // Insert (val_ptr = SP, runtime never reads since
    // val_sz=0).
    self.emitter.emit_ldr(X0, recv, 0);
    self.emit_add_sp_offset(X1, k_off);
    self.emitter.emit_mov_reg(X2, SP);
    self.emit_extern_call("_zo_map_insert");

    self.emit_ldr_sp(X16, was_new_off);

    if let Some(dst) = self.reg_for_insn(idx)
      && dst != X16
    {
      self.emitter.emit_mov_reg(dst, X16);
    }
  }

  /// `s.contains(k)` — spill k, call `_zo_map_contains`.
  fn emit_set_contains(&mut self, args: &[ValueId], idx: usize) {
    let recv = args.first().and_then(|v| self.alloc_reg(*v)).unwrap_or(X0);
    let k = args.get(1).and_then(|v| self.alloc_reg(*v)).unwrap_or(X1);

    let scratch_base = self.struct_base + self.next_struct_slot;
    let k_off = scratch_base;

    self.next_struct_slot += STACK_SLOT_SIZE;

    self.emit_str_sp(k, k_off);

    self.emitter.emit_ldr(X0, recv, 0);
    self.emit_add_sp_offset(X1, k_off);
    self.emit_extern_call("_zo_map_contains");

    if let Some(dst) = self.reg_for_insn(idx)
      && dst != X0
    {
      self.emitter.emit_mov_reg(dst, X0);
    }
  }

  /// `s.remove(k)` — spill k, call `_zo_map_remove`
  /// with `val_out` pointing at scratch (runtime
  /// writes 0 bytes when `val_sz=0`).
  fn emit_set_remove(&mut self, args: &[ValueId], idx: usize) {
    let recv = args.first().and_then(|v| self.alloc_reg(*v)).unwrap_or(X0);
    let k = args.get(1).and_then(|v| self.alloc_reg(*v)).unwrap_or(X1);

    let scratch_base = self.struct_base + self.next_struct_slot;
    let k_off = scratch_base;

    self.next_struct_slot += STACK_SLOT_SIZE;

    self.emit_str_sp(k, k_off);

    self.emitter.emit_ldr(X0, recv, 0);
    self.emit_add_sp_offset(X1, k_off);
    self.emitter.emit_mov_reg(X2, SP);
    self.emit_extern_call("_zo_map_remove");

    if let Some(dst) = self.reg_for_insn(idx)
      && dst != X0
    {
      self.emitter.emit_mov_reg(dst, X0);
    }
  }

  /// `__zo_set_len_raw` and `__zo_set_free_raw` route
  /// directly to the shared map exports — sets reuse
  /// the map allocator wholesale.
  fn emit_set_len_raw(&mut self, args: &[ValueId], idx: usize) {
    self.emit_map_len_raw(args, idx);
  }

  fn emit_set_free_raw(&mut self, args: &[ValueId], idx: usize) {
    self.emit_map_free_raw(args, idx);
  }

  /// Emit CMP + MOV 1 + MOV 0 + CSEL pattern for
  /// comparisons. Uses X16 as zero scratch to avoid
  /// clobbering dst.
  fn emit_cmp_csel(
    &mut self,
    dst: Register,
    lhs: Register,
    rhs: Register,
    cond: u8,
  ) {
    self.emitter.emit_cmp(lhs, rhs);
    self.emitter.emit_mov_imm(dst, 1);
    self.emitter.emit_mov_imm(X16, 0);
    self.emitter.emit_csel(dst, dst, X16, cond);
  }

  /// Emit a call to the runtime render function.
  fn emit_render_call(&mut self, value: ValueId) {
    let template_symbol = Symbol(value.0 + TEMPLATE_SYMBOL_OFFSET);
    let fixup_pos = self.emitter.current_offset();

    self.string_fixups.push((fixup_pos, template_symbol));
    self.emitter.emit_adr(X0, 0);

    self.emitter.emit_mov_imm(X16, SYS_WRITE);
    self.emitter.emit_mov_imm(X0, FD_STDOUT);
    self.emitter.emit_svc(0);
  }

  /// Write binary to file and make it executable.
  pub fn write_executable(
    binary: Vec<u8>,
    path: impl AsRef<Path>,
  ) -> std::io::Result<()> {
    fs::write(&path, binary)?;

    #[cfg(unix)]
    {
      let metadata = fs::metadata(&path)?;
      let mut permissions = metadata.permissions();

      permissions.set_mode(0o755);
      fs::set_permissions(&path, permissions)?;
    }

    Ok(())
  }

  /// Generate a complete "Hello, World" executable.
  pub fn generate_hello_world() -> Vec<u8> {
    let mut emitter = ARM64Emitter::new();
    let hello_str = b"Hello, World!\n";

    emitter.emit_mov_imm(X16, SYS_WRITE);
    emitter.emit_mov_imm(X0, FD_STDOUT);

    emitter.emit_adr(X1, HELLO_STR_OFFSET);
    emitter.emit_mov_imm(X2, HELLO_STR_LEN);
    emitter.emit_svc(0);

    emitter.emit_mov_imm(X16, SYS_EXIT);
    emitter.emit_mov_imm(X0, 0);
    emitter.emit_svc(0);

    let mut code = emitter.code();
    code.extend_from_slice(hello_str);

    let mut macho = MachO::new();
    macho.add_code(code);
    macho.add_data(Vec::new());
    macho.add_pagezero_segment();
    macho.add_text_segment();
    macho.add_data_segment();
    macho.add_function_symbol("_main", 1, TEXT_SECTION_BASE, false);
    macho.add_dylinker();
    macho.add_dylib("/usr/lib/libSystem.B.dylib");
    macho.add_uuid();
    macho.add_build_version();
    macho.add_source_version();
    macho.add_main(CODE_OFFSET);
    macho.add_dyld_info();
    macho.finish()
  }

  /// Generate a complete "Hello, World" executable with
  /// code signature.
  pub fn generate_hello_world_signed() -> Vec<u8> {
    let mut emitter = ARM64Emitter::new();
    let hello_str = b"Hello, World!\n";

    emitter.emit_mov_imm(X16, SYS_WRITE);
    emitter.emit_mov_imm(X0, FD_STDOUT);

    emitter.emit_adr(X1, HELLO_STR_OFFSET);
    emitter.emit_mov_imm(X2, HELLO_STR_LEN);
    emitter.emit_svc(0);

    emitter.emit_mov_imm(X16, SYS_EXIT);
    emitter.emit_mov_imm(X0, 0);
    emitter.emit_svc(0);

    let mut code = emitter.code();
    code.extend_from_slice(hello_str);

    let code_len = code.len();

    let mut macho = MachO::new();
    macho.add_code(code);
    macho.add_data(Vec::new());
    macho.add_pagezero_segment();
    macho.add_text_segment();
    macho.add_data_segment();
    macho.add_function_symbol("_main", 1, TEXT_SECTION_BASE, false);

    macho.add_source_file_info("hello_world.zo", "/tmp/zo");
    macho.add_compiler_info("zo v0.1.0", 2);
    macho.add_function_brackets("_main", 1, TEXT_SECTION_BASE, code_len as u64);
    macho.add_source_line(1, TEXT_SECTION_BASE);

    let mut frame_entry =
      DebugFrameEntry::new(TEXT_SECTION_BASE, code_len as u64);

    frame_entry.add_def_cfa(CFA_FP_REG, 0);
    frame_entry.add_nop();
    macho.add_debug_frame_entry(frame_entry);

    macho.add_dylinker();
    macho.add_dylib("/usr/lib/libSystem.B.dylib");
    macho.add_uuid();
    macho.add_build_version();
    macho.add_source_version();
    macho.add_main(CODE_OFFSET);
    macho.add_dyld_info();
    macho.finish_with_signature()
  }
}

impl<'a> zo_codegen_backend::Backend for ARM64Gen<'a> {
  fn generate(&mut self, sir: &Sir) -> Artifact {
    self.generate(sir)
  }
}
