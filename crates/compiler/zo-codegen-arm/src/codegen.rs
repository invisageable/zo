pub(crate) mod template;

use zo_buffer::Buffer;
use zo_codegen_backend::Artifact;
use zo_emitter_arm::{
  ARM64Emitter, COND_EQ, COND_GE, COND_GT, COND_LE, COND_LT, COND_NE, COND_VC,
  COND_VS, D0, D1, FpRegister, Register, SP, X0, X1, X2, X16, X17, X29, X30,
  XZR,
};
use zo_interner::{Interner, Symbol};
use zo_register_allocation::{EmitTiming, RegAlloc, RegisterClass, SpillKind};
use zo_sir::{BinOp, Insn, LoadSource, Sir, UnOp};
use zo_ty::TyId;
use zo_value::ValueId;
use zo_writer_macho::{DATA_VM_ADDR, DebugFrameEntry, MachO};

use rustc_hash::FxHashMap as HashMap;

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

// --- macOS ARM64 System Calls ---
const SYS_WRITE: u16 = 4;
const SYS_EXIT: u16 = 1;
const FD_STDOUT: u16 = 1;
const FD_STDERR: u16 = 2;

// --- ASCII Constants ---
const ASCII_NEWLINE: u16 = 10;
const ASCII_ZERO: u16 = 48;

// --- Stack Frame Layout ---
const STACK_SLOT_SIZE: u32 = 8;
const FP_LR_SAVE_OFFSET: i16 = -16;
const FP_LR_LOAD_OFFSET: i16 = 16;
// TODO(codegen): MUTABLE_VAR_RESERVE is a fixed 64-byte budget
// (8 mutable slots). Functions with many locals + showln calls
// (~13+ interpolated showlns) overflow this and segfault. The
// reserve should be computed from the actual number of Store
// instructions in the function body, not hardcoded.
const MUTABLE_VAR_RESERVE: u32 = 64;
// 7 caller-saved temp regs (X9-X17) * 8 bytes each.
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
const ARRAY_HEADER_SIZE: u16 = 8;

// --- Type Detection ---
const BOOL_TYPE_ID: u32 = 2;
const FLOAT_TYPE_ID_MIN: u32 = 15;
const FLOAT_TYPE_ID_MAX: u32 = 17;

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
  /// Base offset for caller-save spill area.
  caller_save_base: u32,
  /// Next mutable variable slot.
  next_mut_slot: u32,
  /// Base offset for struct allocations in the frame.
  struct_base: u32,
  /// Next struct slot offset (relative to struct_base).
  next_struct_slot: u32,
  /// Functions that return structs: name -> field count.
  struct_return_fns: HashMap<Symbol, u32>,
  /// Set when the last emitted instruction was a math
  /// intrinsic (FSQRT, FRINT*). Result is in D0.
  last_was_math_intrinsic: bool,
  /// Libm functions used (ordered, no duplicates).
  /// Each entry is the C symbol name (e.g. "_pow").
  libm_used: Vec<String>,
  /// Code offsets of stubs for each libm function.
  /// Populated after all user code is emitted.
  libm_stub_offsets: HashMap<String, u32>,
  /// BL fixups: (code_offset, c_symbol_name).
  /// Patched in assemble() to point at stubs.
  libm_fixups: Vec<(u32, String)>,
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
/// pre-baked `"EnumName::Variant"` string in `string_data`;
/// when `field_count > 0`, `emit_enum_write` appends a shared
/// `"(...)"` marker so users can distinguish unit variants from
/// tuple variants without having to inspect the payload. Real
/// payload printing lands in a follow-up.
struct VariantMeta {
  discriminant: u32,
  field_count: u32,
  display_sym: Symbol,
}

/// Synthetic symbol reserved for the shared `"(...)"` suffix
/// appended to tuple-variant enum prints. Lives at the top of
/// the synthetic-symbol range so it never collides with
/// per-variant display strings.
const ENUM_TUPLE_ELLIPSIS_SYM: Symbol = Symbol(0xE000_FFFF);

impl<'a> ARM64Gen<'a> {
  /// Creates a new [`ARM64Gen`] instance.
  pub fn new(interner: &'a Interner) -> Self {
    Self {
      emitter: ARM64Emitter::new(),
      interner,
      functions: HashMap::default(),
      string_data: Vec::new(),
      current_function: None,
      string_fixups: Vec::new(),
      template_data: Vec::new(),
      has_templates: false,
      labels: HashMap::default(),
      branch_fixups: Vec::new(),
      reg_alloc: None,
      current_fn_start: None,
      mutable_slots: HashMap::default(),
      param_slots: HashMap::default(),
      caller_save_base: 0,
      next_mut_slot: 0,
      struct_base: 0,
      next_struct_slot: 0,
      struct_return_fns: HashMap::default(),
      last_was_math_intrinsic: false,
      libm_used: Vec::new(),
      libm_stub_offsets: HashMap::default(),
      libm_fixups: Vec::new(),
      call_fixups: Vec::new(),
      enum_metas: HashMap::default(),
      next_enum_sym: ENUM_SYNTHETIC_SYM_BASE,
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

  /// Check if a ValueId was produced by a ConstString or
  /// is a Load of a str-typed variable.
  fn is_string_value(&self, vid: ValueId, all_insns: &[Insn]) -> bool {
    self
      .find_producing_insn(vid, all_insns)
      .is_some_and(|insn| match insn {
        Insn::ConstString { .. } => true,
        Insn::Load { ty_id, .. } | Insn::ArrayIndex { ty_id, .. } => {
          ty_id.0 == 4
        }
        _ => false,
      })
  }

  /// Check if a ValueId was produced by a float instruction.
  fn is_float_value(&self, vid: ValueId, all_insns: &[Insn]) -> bool {
    self
      .find_producing_insn(vid, all_insns)
      .is_some_and(|insn| match insn {
        Insn::ConstFloat { .. } => true,
        Insn::BinOp { ty_id, .. }
        | Insn::Load { ty_id, .. }
        | Insn::Call { ty_id, .. }
        | Insn::ArrayIndex { ty_id, .. } => {
          ty_id.0 >= FLOAT_TYPE_ID_MIN && ty_id.0 <= FLOAT_TYPE_ID_MAX
        }
        _ => false,
      })
  }

  /// Check if a ValueId was produced by a bool instruction.
  fn is_bool_value(&self, vid: ValueId, all_insns: &[Insn]) -> bool {
    self
      .find_producing_insn(vid, all_insns)
      .is_some_and(|insn| match insn {
        Insn::ConstBool { .. } => true,
        Insn::BinOp { ty_id, .. }
        | Insn::Load { ty_id, .. }
        | Insn::ArrayIndex { ty_id, .. } => ty_id.0 == BOOL_TYPE_ID,
        _ => false,
      })
  }

  /// Return the enum `TyId` when the given `ValueId` was
  /// produced by an enum constructor, or loaded / indexed out
  /// of an enum-typed slot. Thanks to the uniform pointer
  /// representation every enum value in a register is a
  /// pointer to `[tag, f0, ...]`, so `Load` / `ArrayIndex` of
  /// an enum-typed value is safe to dispatch through
  /// `emit_enum_write`. The codegen-owned `enum_metas` table
  /// is the "is this ty_id an enum?" oracle — only enums the
  /// codegen has actually seen via `Insn::EnumDef` are
  /// eligible; any stale `ty_id` falls through to `None`.
  fn is_enum_value(&self, vid: ValueId, all_insns: &[Insn]) -> Option<TyId> {
    self
      .find_producing_insn(vid, all_insns)
      .and_then(|insn| match insn {
        Insn::EnumConstruct { ty_id, .. } => Some(*ty_id),
        Insn::Load {
          src: LoadSource::Local(sym),
          ..
        } => {
          // Trace through the variable's Store to find if
          // it was initialized by an EnumConstruct. Use
          // the EnumConstruct's ty_id (which is always
          // correct) instead of the Load's ty_id (which
          // can be a generic type variable that collides
          // numerically with an enum ty_id).
          let fn_start = self.current_fn_start.unwrap_or(0);

          all_insns[fn_start..].iter().rev().find_map(|i| {
            if let Insn::Store { name, value, .. } = i
              && *name == *sym
            {
              // Found the Store — check if its value
              // came from an EnumConstruct.
              all_insns[fn_start..].iter().find_map(|j| {
                if let Insn::EnumConstruct { dst, ty_id, .. } = j
                  && *dst == *value
                  && self.enum_metas.contains_key(&ty_id.0)
                {
                  Some(*ty_id)
                } else {
                  None
                }
              })
            } else {
              None
            }
          })
        }
        _ => None,
      })
  }

  /// Find the SIR instruction that produced a ValueId.
  fn find_producing_insn<'b>(
    &self,
    vid: ValueId,
    all_insns: &'b [Insn],
  ) -> Option<&'b Insn> {
    let fn_start = self.current_fn_start.unwrap_or(0);

    self.reg_alloc.as_ref().and_then(|a| {
      a.value_ids
        .iter()
        .enumerate()
        .skip(fn_start)
        .find(|(_, v)| **v == Some(vid))
        .and_then(|(i, _)| all_insns.get(i))
    })
  }

  /// Emit a single spill operation (GP or FP).
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

  // --- Code generation ---

  /// Generates `ARM64` code from SIR.
  pub fn generate(&mut self, sir: &Sir) -> Artifact {
    // Run register allocation before codegen.
    self.reg_alloc =
      Some(RegAlloc::allocate(&sir.instructions, sir.next_value_id));

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
    for i in 0..self.libm_used.len() {
      let sym = self.libm_used[i].clone();
      let offset = self.emitter.current_offset();

      self.libm_stub_offsets.insert(sym, offset);

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
    let libm_fixups = std::mem::take(&mut self.libm_fixups);

    let mut code = self.emitter.code();

    for (fixup_pos, c_sym) in &libm_fixups {
      if let Some(&stub_off) = self.libm_stub_offsets.get(c_sym) {
        let relative = (stub_off as i32 - *fixup_pos as i32) >> 2;
        let pos = *fixup_pos as usize;
        let insn = BL_OPCODE | ((relative as u32) & FIXUP_IMM26_MASK);

        code[pos..pos + 4].copy_from_slice(&insn.to_le_bytes());
      }
    }

    self.libm_fixups = libm_fixups;
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
    let n_got = self.libm_used.len();
    let mut got_data = Vec::with_capacity(n_got * 8);
    let mut bind_entries: Vec<(&str, u8, u64)> = Vec::new();

    for (i, c_sym) in self.libm_used.iter().enumerate() {
      let got_offset_in_data = (i * 8) as u64;
      let got_vm_addr = DATA_VM_ADDR + got_offset_in_data;

      // Populate GOT slot with zero (dyld overwrites).
      got_data.extend_from_slice(&[0u8; 8]);

      // Patch the stub: ADRP X16, page_diff; LDR X16,
      // [X16, #page_off]; BR X16.
      if let Some(&stub_off) = self.libm_stub_offsets.get(c_sym) {
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

      // segment 2 = __DATA (pagezero=0, __TEXT=1, __DATA=2)
      bind_entries.push((c_sym, DATA_SEGMENT_INDEX, got_offset_in_data));
    }

    // Build bind opcodes for dyld.
    // dylib ordinal 1 = first LC_LOAD_DYLIB (libSystem).
    if !bind_entries.is_empty() {
      let bind_data =
        MachO::build_bind_opcodes(&bind_entries, LIBSYSTEM_DYLIB_ORDINAL);

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

    // Add undefined symbols for each libm function.
    // dylib ordinal 1 = libSystem.
    for c_sym in &self.libm_used {
      macho.add_undefined_symbol(c_sym, LIBSYSTEM_DYLIB_ORDINAL as u16);
    }

    macho.add_dylinker();
    macho.add_dylib("/usr/lib/libSystem.B.dylib");
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

        // Function prologue: save FP/LR if non-leaf.
        if let Some(info) = self
          .reg_alloc
          .as_ref()
          .and_then(|a| a.function_info.get(&idx))
        {
          if info.has_calls {
            self.emitter.emit_stp(X29, X30, SP, FP_LR_SAVE_OFFSET);
          }

          // Reserve space for spills + mutable slots +
          // parameter spill slots + struct allocations.
          let param_reserve = params.len() as u32 * STACK_SLOT_SIZE;
          let caller_save = if info.has_calls {
            CALLER_SAVE_RESERVE
          } else {
            0
          };
          let frame = (info.spill_size
            + MUTABLE_VAR_RESERVE
            + param_reserve
            + caller_save
            + info.struct_size
            + FRAME_ALIGN_MASK)
            & !FRAME_ALIGN_MASK;

          if frame > 0 {
            self.emitter.emit_sub_imm(SP, SP, frame as u16);
          }

          // Set caller-save spill base for BL save/restore.
          self.caller_save_base =
            info.spill_size + MUTABLE_VAR_RESERVE + param_reserve;

          // Struct base: after caller-save area.
          self.struct_base =
            info.spill_size + MUTABLE_VAR_RESERVE + param_reserve + caller_save;

          // Spill parameters to stack so they survive
          // register reuse. Params arrive in X0-X7 (GP)
          // or D0-D7 (FP) depending on type.
          let param_base = info.spill_size + MUTABLE_VAR_RESERVE;

          for (i, (_, ty_id)) in params.iter().enumerate() {
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
          }
        }
        LoadSource::Param(idx) => {
          // Load from parameter spill slot (saved in prologue). This is safe
          // even after registers have been reused for other values.
          if let Some(&off) = self.param_slots.get(idx) {
            if let Some(fp_dst) = self.alloc_fp_reg(*dst) {
              // Float param: load from FP spill slot.
              self.emitter.emit_ldr_fp(fp_dst, SP, off as u16);
            } else if let Some(dst_reg) = self.alloc_reg(*dst) {
              // GP param: load from GP spill slot.
              self.emitter.emit_ldr(dst_reg, SP, off as i16);
            }
          } else if let Some(fp_dst) = self.alloc_fp_reg(*dst) {
            // Fallback: no spill slot — read from original register.
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
          let fd = self.alloc_fp_reg(*dst).unwrap_or(D0);
          match op {
            BinOp::Add => self.emitter.emit_fadd(fd, fl, fr),
            BinOp::Sub => self.emitter.emit_fsub(fd, fl, fr),
            BinOp::Mul => self.emitter.emit_fmul(fd, fl, fr),
            BinOp::Div => self.emitter.emit_fdiv(fd, fl, fr),
            BinOp::Lt
            | BinOp::Lte
            | BinOp::Gt
            | BinOp::Gte
            | BinOp::Eq
            | BinOp::Neq => self.emitter.emit_fcmp(fl, fr),
            _ => {}
          }
        } else if self.enum_metas.contains_key(&ty_id.0)
          && matches!(op, BinOp::Eq | BinOp::Neq)
        {
          // Enum equality: both operands are pointers to `[tag, ...]` thanks
          // to the uniform representation. Pointer-level cmp would return
          // false for two distinct allocations holding the same variant, so
          // load both tags first and then compare. Other comparison operators
          // (`<`, `<=`, …) are undefined on enum types and fall through to the
          // integer path below as a noop.
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

          match op {
            BinOp::Add => self.emitter.emit_add(d, l, r),
            BinOp::Sub => self.emitter.emit_sub(d, l, r),
            BinOp::Mul => self.emitter.emit_mul(d, l, r),
            BinOp::Div => self.emitter.emit_sdiv(d, l, r),
            BinOp::Rem => {
              // dst = lhs - (lhs / rhs) * rhs
              // Use X16 as scratch.
              self.emitter.emit_sdiv(X16, l, r);
              self.emitter.emit_mul(X16, X16, r);
              self.emitter.emit_sub(d, l, X16);
            }
            BinOp::And | BinOp::BitAnd => self.emitter.emit_and(d, l, r),
            BinOp::Or | BinOp::BitOr => self.emitter.emit_orr(d, l, r),
            BinOp::BitXor => self.emitter.emit_eor(d, l, r),
            BinOp::Shl => self.emitter.emit_lsl(d, l, 1),
            BinOp::Shr => self.emitter.emit_lsr(d, l, 1),
            BinOp::Lt => self.emit_cmp_csel(d, l, r, COND_LT),
            BinOp::Lte => self.emit_cmp_csel(d, l, r, COND_LE),
            BinOp::Gt => self.emit_cmp_csel(d, l, r, COND_GT),
            BinOp::Gte => self.emit_cmp_csel(d, l, r, COND_GE),
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

            self.emit_typed_write(arg_vid, all_insns, fd);

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

          // Math intrinsics — ARM64 hardware instructions. The arg is a float
          // in a FP register. Move it to D0, execute the instruction, leave
          // result in D0 for showln/binding to consume.
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
            self.libm_fixups.push((fixup_pos, c_sym.clone()));

            // Track used libm functions (no duplicates).
            if !self.libm_used.contains(&c_sym) {
              self.libm_used.push(c_sym);
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
        if let Some(start) = self.current_fn_start
          && let Some(info) = self
            .reg_alloc
            .as_ref()
            .and_then(|a| a.function_info.get(&start))
        {
          let param_reserve = self.param_slots.len() as u32 * STACK_SLOT_SIZE;
          let caller_save = if info.has_calls {
            CALLER_SAVE_RESERVE
          } else {
            0
          };
          let frame = (info.spill_size
            + MUTABLE_VAR_RESERVE
            + param_reserve
            + caller_save
            + info.struct_size
            + FRAME_ALIGN_MASK)
            & !FRAME_ALIGN_MASK;

          if frame > 0 {
            self.emitter.emit_add_imm(SP, SP, frame as u16);
          }

          if info.has_calls {
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
        // Layout in pre-allocated frame: [len, e0, e1, ..., eN]
        // Uses struct_base + next_struct_slot (frame-relative).
        let base = self.struct_base + self.next_struct_slot;
        let n = elements.len() as u16;

        // Store length at [SP + base].
        self.emitter.emit_mov_imm(X16, n);
        self.emitter.emit_str(X16, SP, base as i16);

        // Store each element at [SP + base + (i+1)*8].
        // Floats use FP registers → emit_str_fp.
        for (i, elem) in elements.iter().enumerate() {
          let off = base + (i as u32 + 1) * STACK_SLOT_SIZE;

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

        // Advance slot for next allocation.
        self.next_struct_slot += (1 + elements.len() as u32) * STACK_SLOT_SIZE;
      }

      Insn::ArrayIndex {
        dst,
        array,
        index,
        ty_id,
      } => {
        let arr_reg = self.alloc_reg(*array).unwrap_or(X0);
        let idx_reg = self.alloc_reg(*index).unwrap_or(X1);

        // Compute element address: base + header + index * 8.
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

      Insn::ArrayStore {
        array,
        index,
        value,
        ty_id,
      } => {
        // Store value at base + 8 + index * 8.
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
      Insn::StructDef { .. } | Insn::ConstDef { .. } => {}

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

          if let Some(reg) = self.alloc_reg(*field) {
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
        dst, tuple, index, ..
      } => {
        if let Some(dst_reg) = self.alloc_reg(*dst) {
          let base_reg = self.alloc_reg(*tuple).unwrap_or(X0);
          let offset = (*index as i16) * (STACK_SLOT_SIZE as i16);

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
  fn emit_typed_write(
    &mut self,
    arg_vid: Option<ValueId>,
    all_insns: &[Insn],
    fd: u16,
  ) {
    let is_str = arg_vid.is_some_and(|v| self.is_string_value(v, all_insns));
    let is_flt = arg_vid.is_some_and(|v| self.is_float_value(v, all_insns));
    let is_bool = arg_vid.is_some_and(|v| self.is_bool_value(v, all_insns));
    let enum_ty = arg_vid.and_then(|v| self.is_enum_value(v, all_insns));

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
        field_count: fields.len() as u32,
        display_sym,
      });
    }

    self.enum_metas.insert(
      ty_id.0,
      EnumMeta {
        variants: variant_metas,
      },
    );
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
      // Fallback — shouldn't happen, but keep the compiler
      // from panicking. Drop to itoa on the source register.
      if let Some(src) = self.alloc_reg(vid)
        && src != X0
      {
        self.emitter.emit_mov_reg(X0, src);
      }
      self.emit_itoa_and_write(fd);
      return;
    };

    // Snapshot the variant list so we can drop the borrow on
    // `self` before emitting code (emit_* takes `&mut self`).
    let variants: Vec<(u32, Symbol, bool)> = meta
      .variants
      .iter()
      .map(|v| (v.discriminant, v.display_sym, v.field_count > 0))
      .collect();

    // Only register the shared "(...)" suffix when this enum
    // actually has at least one tuple variant. Pure-unit enums
    // (e.g. `enum Color { Red, Green, Blue }`) must not leak
    // the ellipsis string into the artifact — that would be
    // user-visible dead data and makes output regressions
    // harder to catch. Mirrors `emit_bool_and_write`'s
    // guard-on-Symbol pattern, restricted by the per-call
    // need.
    let ellipsis_sym = ENUM_TUPLE_ELLIPSIS_SYM;
    let any_tuple = variants.iter().any(|(_, _, has_fields)| *has_fields);

    if any_tuple && !self.string_data.iter().any(|(s, _)| *s == ellipsis_sym) {
      let mut buf = Buffer::new();
      let bytes: &[u8] = b"(...)";

      buf.bytes(&(bytes.len() as u64).to_le_bytes());
      buf.bytes(bytes);
      buf.bytes(b"\0");

      self.string_data.push((ellipsis_sym, buf.finish()));
    }

    // Load the discriminant into X17. Every enum value is now
    // a pointer to `[tag, f0, f1, ...]` — even unit variants —
    // so we always read slot 0. The earlier unit-vs-tuple
    // branch is gone with the uniform representation.
    let src = self.alloc_reg(vid).unwrap_or(X0);

    self.emitter.emit_ldr(X17, src, 0);

    // Emit one arm per variant: cmp + b.ne + write-name + b.
    // Track fixups to patch once we know the final positions.
    let mut bne_fixups: Vec<usize> = Vec::with_capacity(variants.len());
    let mut done_fixups: Vec<usize> = Vec::with_capacity(variants.len());

    for (disc, display_sym, has_fields) in &variants {
      // cmp x15, #disc.
      self.emitter.emit_cmp_imm(X17, *disc as u16);

      // b.ne past_this_variant (patched later).
      let bne_pos = self.emitter.current_offset();
      self.emitter.emit_bne(0);
      bne_fixups.push(bne_pos as usize);

      // Load the display-string pointer via ADR + fixup.
      let adr_pos = self.emitter.current_offset();
      self.string_fixups.push((adr_pos, *display_sym));
      self.emitter.emit_adr(X16, 0);

      // Unpack the zo-string struct and syscall write.
      self.emitter.emit_ldr(X2, X16, 0);
      self.emitter.emit_add_imm(X1, X16, 8);
      self.emitter.emit_mov_imm(X16, SYS_WRITE);
      self.emitter.emit_mov_imm(X0, fd);
      self.emitter.emit_svc(0);

      // Tuple variants get a `(...)` suffix so the user sees
      // they carry a payload without blowing up the output.
      if *has_fields {
        let ellipsis_pos = self.emitter.current_offset();
        self.string_fixups.push((ellipsis_pos, ellipsis_sym));
        self.emitter.emit_adr(X16, 0);

        self.emitter.emit_ldr(X2, X16, 0);
        self.emitter.emit_add_imm(X1, X16, 8);
        self.emitter.emit_mov_imm(X16, SYS_WRITE);
        self.emitter.emit_mov_imm(X0, fd);
        self.emitter.emit_svc(0);
      }

      // b done (patched later).
      let done_pos = self.emitter.current_offset();
      self.emitter.emit_b(0);
      done_fixups.push(done_pos as usize);

      // Patch the b.ne to land right here — past the body.
      let after_body = self.emitter.current_offset() as i32;
      let bne_rel = after_body - bne_pos as i32;
      self.emitter.patch_bcond_at(bne_pos as usize, bne_rel);
    }

    // Patch every done-jump to point at the current offset.
    let done_label = self.emitter.current_offset() as i32;

    for pos in done_fixups {
      let rel = done_label - pos as i32;
      self.emitter.patch_b_at(pos, rel);
    }

    // `bne_fixups` is no longer needed — each was patched
    // immediately after its body to skip to the next arm.
    let _ = bne_fixups;
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
    self.emitter.emit_sub(SP, SP, x4);

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
