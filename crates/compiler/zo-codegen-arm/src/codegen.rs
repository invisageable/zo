pub(crate) mod template;

use crate::promotion::Promotion;

use zo_buffer::Buffer;
use zo_codegen_backend::{Artifact, MachoLinkObject, Webviewing};
use zo_emitter_arm::{
  ARM64Emitter, COND_CC, COND_CS, COND_EQ, COND_GE, COND_GT, COND_HI, COND_LE,
  COND_LS, COND_LT, COND_NE, COND_VC, COND_VS, D0, D1, D16, FpRegister,
  PatchSite, Register, SP, X0, X1, X2, X3, X4, X5, X6, X7, X9, X10, X11, X16,
  X17, X29, X30, XZR,
};
use zo_interner::{DenseMap, Interner, Sentinel, Symbol};
use zo_module_resolver::{AbstractDef, AbstractImpl};
use zo_register_allocation::{
  AllocInput, EmitTiming, EnumPayloadFields, IO_RESULT_FRAME_SLOTS,
  IO_SHARED_BUF_SLOTS, RegAlloc, RegisterClass, SpillKind,
  flat_struct_slots_of, resolve_ty,
};
use zo_sir::{BinOp, Insn, ListItemCmd, LoadSource, Sir, SpawnKind, UnOp};
use zo_ty::{Ty, TyId, TyTable, struct_leaf_words};
use zo_ui_protocol::codec;
use zo_ui_protocol::{Attr, LIST_ITEM_SENTINEL, UiCommand};
use zo_value::{FunctionKind, ValueId};
use zo_writer_macho::{CODE_OFFSET, DebugFrameEntry, MachO, TEXT_SECTION_BASE};

use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

// --- macOS ARM64 System Calls ---
const SYS_EXIT: u16 = 1;
const SYS_READ: u16 = 3;
const SYS_WRITE: u16 = 4;
const SYS_OPEN: u16 = 5;
const SYS_CLOSE: u16 = 6;
const SYS_UNLINK: u16 = 10;
const SYS_ACCESS: u16 = 33;
const FD_STDOUT: u16 = 1;
const FD_STDERR: u16 = 2;
const O_READ_ONLY: u16 = 0;
const O_WRITE_ONLY_CREATE_TRUNCATE: u16 = 0x601;
const O_WRITE_ONLY_CREATE_APPEND: u16 = 0x209;
const FILE_MODE_644: u16 = 0o644;
const READ_FILE_BUF_SIZE: u16 = 4096;

// `IO_RESULT_FRAME_SLOTS` (3) and `IO_SHARED_BUF_SLOTS`
// (513) are imported from `zo-register-allocation` so the
// regalloc's `struct_slots` budget and the codegen's
// `next_struct_slot` bumps stay aligned. Per-IO-call
// frame layout: tag (8), heap str ptr or errno (8),
// saved bytes_read scratch (8). The shared buffer is
// `READ_FILE_BUF_SIZE` data bytes + one slot for null
// padding / alignment.

// --- ASCII Constants ---
const ASCII_NEWLINE: u16 = 10;
const ASCII_ZERO: u16 = 48;

// --- Stack Frame Layout ---
const STACK_SLOT_SIZE: u32 = 8;
const FP_LR_SAVE_OFFSET: i16 = -16;
const FP_LR_LOAD_OFFSET: i16 = 16;
/// Bytes the pre-indexed `stp x29, x30, [sp, #-16]!` frame
/// record adds to the prologue's SP drop. Only non-leaf
/// functions push it (`has_calls`), so a callee's incoming
/// stack args sit `frame + FRAME_RECORD_SIZE` above its
/// working SP for non-leaf callees, and `frame` above it for
/// leaf callees with no record.
const FRAME_RECORD_SIZE: u32 = 16;
// 15 caller-saved temp regs (X1-X15) * 8 bytes each.
// Includes X1..X8 because the register allocator may
// place live values there — element ConstInt slots in
// `ArrayLiteral` with > 7 elements, multi-arg call
// receivers, etc. Pre-bump from X9..X15 didn't cover
// this and `bl _malloc` silently clobbered the
// extras. X0 is excluded because it's the BL's first
// argument slot, set by the caller right before
// `emit_extern_call`. X16/X17 are intra-procedure
// scratch (the AAPCS allows any callee to use them
// without saving), so the caller never expects them
// preserved — saving is unnecessary.
const CALLER_SAVE_RESERVE: u32 = 144;
/// 2 GP slots reserved per function past `select_scratch` for
/// `Insn::ArrayPush`'s realloc + struct-element heap-clone
/// saves. Sized for the larger of the two paths (both save 2
/// regs across one BL); the paths run sequentially within a
/// single push and reuse the same slots. Lives ABOVE struct
/// memory so a saved `val_reg` pointing into a struct can't
/// stomp the struct it points at.
const ARRAY_PUSH_SCRATCH_SIZE: u32 = 16;
const CALLER_SAVE_COUNT: usize = 15;
const CALLER_SAVE_START: u8 = 1;
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
const BYTES_TYPE_ID: u32 = 5; // TyChecker: Bytes @ index 5
const CHAR_TYPE_ID: u32 = 3; // TyChecker: Char @ index 3
const STR_TYPE_ID: u32 = 4; // TyChecker: Str @ index 4
const FLOAT_TYPE_ID_MIN: u32 = 15; // TyChecker: F32  @ index 15
const F64_TYPE_ID: u32 = 16; //         TyChecker: F64  @ index 16
const FLOAT_TYPE_ID_MAX: u32 = 17; // TyChecker: Arch @ index 17 (range upper bound)

// --- Mach-O Layout ---
// `TEXT_SECTION_BASE` (vmaddr where `__text` begins) and
// `CODE_OFFSET` (file offset of the entry point) are owned
// by `zo-writer-macho` and re-exported here via the `use`
// at the top of the file. Defining local copies once led to
// a layout drift — the writer placed `__text` at file 0x800
// but a stale local `CODE_OFFSET = 0x400` was still fed to
// `add_main`, so `LC_MAIN.entryoff` pointed inside the
// load-command region and the kernel SIGILL'd on launch.
// Single source of truth fixes this for good.
pub(super) const UI_ENTRY_SYMBOL: u32 = 0xFFFF;
pub const TEMPLATE_SYMBOL_OFFSET: u32 = 0x1000;

/// AAPCS general-purpose argument registers in order.
/// Args 0..=7 land in X0..X7; beyond that they spill to
/// the stack.
const AAPCS_ARG_REGS: [Register; 8] = [X0, X1, X2, X3, X4, X5, X6, X7];

/// The variable-size areas of a function's stack frame, in
/// the fixed order the prologue lays them out. The prologue
/// and epilogue both feed this to `aligned_frame_size` so the
/// two `sub sp` / `add sp` amounts can never drift apart.
struct FrameAreas {
  /// Register-spill slots (`FunctionInfo::spill_size`).
  spill_size: u32,
  /// `mut` variable slots (`FunctionInfo::mutable_size`).
  mut_size: u32,
  /// One word per parameter for the home-slot stores.
  param_reserve: u32,
  /// Blanket caller-save + overflow-arg staging — dynamic
  /// per `compute_caller_save_reserve`.
  caller_save: u32,
  /// Struct / enum / array literal scratch.
  struct_size: u32,
  /// Channel-op scratch (`FunctionInfo::chan_scratch_size`).
  chan_scratch_size: u32,
  /// `SelectWait` scratch (`FunctionInfo::select_scratch_size`).
  select_scratch_size: u32,
  /// `StringFormat` pointer-array scratch.
  string_format_scratch_size: u32,
  /// Save area for promoted callee-saved registers (x19..x28
  /// claimed by register promotion). One 8-byte slot per
  /// register, padded to keep the frame 16-aligned. Lives at
  /// the top of the frame so no other area's base offset
  /// shifts when promotion is active.
  promo_save_size: u32,
}

/// One enum variant's layout snapshot for the
/// post-call deep-copy. `discriminant` selects the
/// variant at runtime; `field_tys` mirrors the
/// in-payload order so per-field deep-copy decisions
/// (struct vs primitive) match the callee's
/// `EnumConstruct` emission.
struct EnumVariantInfo {
  discriminant: u32,
  field_tys: Vec<TyId>,
}

/// Snapshot of an enum return type's layout passed to
/// `emit_enum_deep_copy_after_call`. `variants` is the
/// full variant list so the dispatch can compare
/// against every discriminant.
struct EnumDeepCopyLayout {
  variants: Vec<EnumVariantInfo>,
}

impl EnumDeepCopyLayout {
  /// `1 + max(variant.field_count)` — discriminant slot
  /// plus the widest variant's payload. Derived from
  /// `variants` so the layout never drifts.
  fn outer_slots(&self) -> u32 {
    1 + self
      .variants
      .iter()
      .map(|v| v.field_tys.len() as u32)
      .max()
      .unwrap_or(0)
  }
}

/// Per-slot vtable patch — `slot_offset` is the byte
/// offset INSIDE the vtable blob identified by
/// `vtable_sym`; `method_key` is the (name, owning_pack)
/// pair `self.functions` keys by.
#[derive(Clone, Copy)]
struct VtableSlotFixup {
  vtable_sym: Symbol,
  slot_offset: u32,
  method_key: (Symbol, Option<Symbol>),
}

// Runtime dylib symbols: every `#render` program emits calls
// into the UI surface of `libzo_runtime.dylib`. Names match
// the `#[no_mangle]` exports of `zo-runtime-native::ffi`
// (the crate the runtime's `ui` feature re-exports;
// Mach-O leading-underscore convention).
//
// These symbols route to the same staged runtime dylib as
// every other `_zo_*` import (`@executable_path/` rewrites
// to `@loader_path/deps/libzo_runtime.dylib` in the
// linker). Folding them into the one runtime entry — rather
// than a parallel absolute-path `libzo_runtime_native`
// reference — lets the compiler stage a single relocatable
// dylib and makes the lean-vs-full choice authoritative:
// importing any of these is exactly what flips the linker
// to `RuntimeKind::Full`.
const RUNTIME_DYLIB_FILE: &str = "@executable_path/libzo_runtime.dylib";
const SYM_RUN: &str = "_zo_run_native";
// The webview entry: same ABI as `_zo_run_native`, but drives the wry
// webview instead of eframe. Selected over `SYM_RUN` when the program
// is built for the webview target.
const SYM_RUN_WEB: &str = "_zo_run_web";
const SYM_STATE_INIT: &str = "_zo_state_init";
const SYM_STATE_GET: &str = "_zo_state_get";
const SYM_STATE_SET: &str = "_zo_state_set";
// Str-typed reactive slots route through a separate
// `Vec<Vec<u8>>` (length-prefixed copies) — the i64 STATE
// can't hold a string value.
const SYM_STATE_GET_STR: &str = "_zo_state_get_str";
const SYM_STATE_SET_STR: &str = "_zo_state_set_str";
// Reactive `[]str` arrays (a list binding's `items_var`) live in
// the runtime's `ARR_STATE`; `arr.push(x)` lowers to this.
const SYM_STATE_ARR_PUSH: &str = "_zo_state_arr_push";

/// Synthetic-symbol base for per-list item-recipe blobs embedded
/// in `template_data`. Sits below the enum-synthetic base
/// (`0xE000_0000`) and above any interned / template symbol, so a
/// recipe symbol never collides. Each list binding mints the next
/// id via `next_recipe_blob`.
const RECIPE_BLOB_SYM_BASE: u32 = 0xD000_0000;

/// One reactive list binding, ready to emit into the
/// `ZoRuntimeContext`'s `ListBindingAbi` array. `recipe_sym` keys
/// the postcard `Vec<UiCommand>` item recipe in `template_data`
/// (resolved to an address by the `string_fixups` machinery).
struct ListBindingEntry {
  /// Index of the placeholder `Text` command the rendered list
  /// items replace.
  cmd_idx: u32,
  /// Reactive array slot whose elements drive the list.
  items_slot: u32,
  /// Symbol of the embedded item-recipe blob.
  recipe_sym: Symbol,
  /// Byte length of the recipe blob.
  recipe_len: u32,
}

/// Lower a list binding's per-item recipe to a `UiCommand`
/// sub-stream the runtime walks once per array element. The item
/// value placeholder (`TextFromItem`) becomes a sentinel `Text`
/// the runtime substitutes; everything else maps one-to-one.
fn convert_list_recipe(recipe: &[ListItemCmd]) -> Vec<UiCommand> {
  recipe
    .iter()
    .map(|step| match step {
      ListItemCmd::Element { tag, attrs } => UiCommand::Element {
        tag: tag.clone(),
        attrs: attrs.clone(),
        self_closing: false,
      },
      ListItemCmd::EndElement => UiCommand::EndElement,
      ListItemCmd::Text(text) => UiCommand::Text(text.clone()),
      ListItemCmd::TextFromItem => {
        UiCommand::Text(LIST_ITEM_SENTINEL.to_string())
      }
    })
    .collect()
}

/// Synthetic-symbol base for the per-template event
/// dispatcher (`_zo_dispatch_N`). Paired 1:1 with
/// `TEMPLATE_SYMBOL_OFFSET` — given template
/// `ValueId(id)`, its dispatcher's `Symbol` is
/// `Symbol(id + TEMPLATE_DISPATCHER_SYMBOL_OFFSET)`.
pub(super) const TEMPLATE_DISPATCHER_SYMBOL_OFFSET: u32 = 0x2000;

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

// --- Hello World ---
const HELLO_STR_OFFSET: i32 = 0x18;
const HELLO_STR_LEN: u16 = 14;
const CFA_FP_REG: u8 = 31;

// Mach-O constants (`PAGE_MASK`, dylib ordinals,
// `ZO_RUNTIME_SYMBOL_PREFIX`, `DATA_SEGMENT_INDEX`,
// `TEXT_SECTION_BASE`, `CODE_OFFSET`) live in
// `zo-writer-macho` — both this crate and `zo-linker`
// share them.

// --- Libm Functions ---

/// Maps a zo function name to its C library symbol.
/// Resolve a `pub ffi` zo name to its C symbol for the
/// linker. `link_name` from a `%% link_name = "X".`
/// attribute wins; otherwise the zo name takes the
/// platform leading underscore. Emitted verbatim into
/// `BL <c_sym>`.
fn c_sym_for(
  interner: &Interner,
  zo_name: Symbol,
  link_name: Option<Symbol>,
) -> String {
  match link_name {
    Some(ln) => format!("_{}", interner.get(ln)),
    None => format!("_{}", interner.get(zo_name)),
  }
}

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

/// Index newtype: position in the SIR instruction stream.
/// Reserves `u32::MAX` as the absent sentinel so a
/// `DenseMap<ValueId, InsnIdx>` can store it directly with
/// no `Option` overhead.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct InsnIdx(u32);

impl Sentinel for InsnIdx {
  const ABSENT: InsnIdx = InsnIdx(u32::MAX);
}

/// Represents the [`ARM64Gen`] code generation instance.
pub struct ARM64Gen<'a> {
  /// The [`ARM64Emitter`].
  pub(super) emitter: ARM64Emitter,
  /// String interner for resolving symbols.
  interner: &'a Interner,
  /// Function labels (name -> code offset).
  pub(super) functions: HashMap<(Symbol, Option<Symbol>), u32>,
  /// String data to emit at end. Iteration is in
  /// registration order — the linker depends on this for
  /// stable string-table layout.
  string_data: Vec<(Symbol, Vec<u8>)>,
  /// Membership shadow of `string_data` for O(1) "is this
  /// symbol already registered?" checks. Replaces the
  /// `string_data.iter().any(|(s, _)| *s == sym)` linear
  /// scan that recurred at every enum/bool/runtime-msg
  /// registration site.
  ///
  /// `HashSet`, NOT `DenseSet` — the codegen mints
  /// synthetic symbols from `ENUM_SYNTHETIC_SYM_BASE`
  /// (`0xE000_0000`) so the symbol space is sparse, not
  /// dense. A `DenseSet` would grow its bitset to ~58M
  /// words (~462 MB) on the first synthetic insert.
  string_data_seen: HashSet<Symbol>,
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
  function_addr_fixups: Vec<(u32, (Symbol, Option<Symbol>))>,
  /// Per-vtable raw bytes; populated by `emit_vtables`
  /// after every FunDef has been laid out.
  vtable_data: Vec<(Symbol, Vec<u8>)>,
  /// Per-slot patches — finalize writes each slot with
  /// `method_addr − vtable_addr`.
  vtable_fixups: Vec<VtableSlotFixup>,
  /// Code-side ADR loaders for vtable addresses,
  /// resolved at link time.
  vtable_addr_fixups: Vec<(u32, Symbol)>,
  /// Abstract definitions threaded from the executor.
  abstract_defs: HashMap<Symbol, AbstractDef>,
  /// `(Abstract, ConcreteType) → AbstractImpl` threaded
  /// from the executor.
  abstract_impls: HashMap<(Symbol, Symbol), AbstractImpl>,
  /// Template data sections (symbol -> data).
  pub(super) template_data: Vec<(Symbol, Vec<u8>)>,
  /// Per-template list of event-handler function names
  /// (e.g. `"__closure_0"`), in declaration order. Stored
  /// as strings — the interner is held immutably so we
  /// can't intern fresh symbols at template-handling time.
  /// `generate_template_dispatchers` resolves each string
  /// to a `Symbol` by scanning `self.functions`, whose
  /// keys were interned by the executor. The position of
  /// a handler in this list IS the `u32` widget-handler
  /// ID that the runtime passes to
  /// `ctx.handle_event(idx, kind)`.
  pub(super) template_handlers: HashMap<ValueId, Vec<String>>,
  /// Reactive state assignment: each `mut` symbol that
  /// appears in any `Template.bindings.text` gets a unique
  /// `u32` slot id. Closures load/store reactive variables
  /// through `zo_state_get(slot)` / `zo_state_set(slot,
  /// value)` calls into the runtime dylib instead of
  /// reading/writing stack frames — that way the runtime
  /// owns the state buffer and the renderer can refresh
  /// `Text` bindings after every event dispatch without
  /// crossing the ABI.
  reactive_slots: HashMap<Symbol, u32>,
  /// Per-template list of `(cmd_idx, slot_id, is_str)`
  /// triples to pass into the `text_bindings` array of
  /// the `ZoRuntimeContext` at `#render` time. Built by the
  /// same pre-pass that populates `reactive_slots`.
  template_text_bindings: HashMap<ValueId, Vec<(u32, u32, bool)>>,
  /// Reactive `[]str` array slots — a list binding's `items_var`.
  /// `arr.push` on these lowers to `_zo_state_arr_push` (into the
  /// runtime's `ARR_STATE`) instead of the local realloc path,
  /// and scalar-state Load/Store skip them.
  reactive_arr_slots: HashSet<Symbol>,
  /// Per-template list bindings to emit into the
  /// `ZoRuntimeContext`'s `ListBindingAbi` array at `#render`.
  /// Built by the same pre-pass that populates `reactive_slots`.
  template_list_bindings: HashMap<ValueId, Vec<ListBindingEntry>>,
  /// Per-template attribute bindings — `(cmd_idx, attr_idx,
  /// slot, is_str)` — to emit into the `ZoRuntimeContext`'s
  /// `AttrBindingAbi` array. The runtime re-applies each on
  /// every event so e.g. `input_val = ""` clears the input.
  template_attr_bindings: HashMap<ValueId, Vec<(u32, u32, u32, bool)>>,
  /// Monotonic counter minting each item-recipe blob's symbol
  /// (`RECIPE_BLOB_SYM_BASE + n`).
  next_recipe_blob: u32,
  /// Set of `FunDef` indices whose body touches a
  /// reactive `mut`. Codegen inserts `bl _zo_state_get` /
  /// `bl _zo_state_set` (and `bl _zo_state_init` for the
  /// program's `main`) for those operations — but the
  /// allocator's `info.has_calls` is computed from
  /// explicit `Insn::Call`s only, so a function whose
  /// only calls are reactive helpers would otherwise get
  /// a leaf-fn prologue (no FP/LR save, no caller-save
  /// reserve), the inserted `bl` then clobbering `X30`
  /// and spilling into someone else's frame. Pre-pass
  /// promotes those functions to non-leaf so the
  /// prologue/epilogue reserve the right area.
  fns_needing_calls: HashSet<InsnIdx>,
  /// Whether we have templates that need the entry point.
  pub has_templates: bool,
  /// The label offsets: label_id → byte offset in code.
  labels: HashMap<u32, u32>,
  /// The branch fixups: (code_offset, target_label_id).
  branch_fixups: Vec<(u32, u32)>,
  /// Register allocation result.
  reg_alloc: Option<RegAlloc>,
  /// Per-insn offset into `reg_alloc.spill_ops` (sorted
  /// by `insn_idx`). `spill_offsets[i]` is the index of
  /// the first spill_op whose `insn_idx == i`;
  /// `spill_offsets[i+1] - spill_offsets[i]` is the
  /// per-insn bucket size. Length = `num_insns + 1`.
  /// Built once in `generate()` after the regalloc
  /// runs. Replaces a per-insn linear filter over the
  /// whole spill-ops Vec — O(insns × spills) became
  /// O(insns + spills).
  spill_offsets: Vec<u32>,
  /// Flat-map fallback for GP register lookups.
  reload_overrides: HashMap<(u32, u32), u8>,
  /// Flat-map fallback for FP register lookups.
  fp_reload_overrides: HashMap<(u32, u32), u8>,
  /// Current function's start index into SIR instructions.
  current_fn_start: Option<usize>,
  /// Current instruction index during emission — drives
  /// per-instruction register lookups.
  current_emit_idx: usize,
  /// Mutable variable stack slots: name → offset from SP.
  mutable_slots: HashMap<u32, u32>,
  /// Register-promotion plan for the current function:
  /// scalar locals lifted from stack slots into dedicated
  /// callee-saved registers (x19..x28). Rebuilt per FunDef
  /// in `enter_function`. Consulted at every `Insn::Store`
  /// and `Insn::Load { LoadSource::Local }` so a promoted
  /// local's read/write is a register move instead of stack
  /// memory traffic. See `promotion.rs`.
  promotion: Promotion,
  /// SSA values whose register IS a promotion register: a
  /// `Load { LoadSource::Local(promoted) }` binds its `dst`
  /// here instead of emitting a memory load, so downstream
  /// `BinOp` / `Call` operands read the callee-saved register
  /// directly. Cleared per FunDef. Keyed by `ValueId.0`.
  promo_value_reg: HashMap<u32, Register>,
  /// Parameter index → promotion register, for `mut`
  /// parameters whose symbol was promoted. The executor
  /// lowers a parameter read as either `LoadSource::Local`
  /// (handled via `promotion`) or `LoadSource::Param(idx)`,
  /// and BOTH forms appear for a `mut` param that is also
  /// reassigned (`while n > 1 { n = n / 2 }`). Without this,
  /// the `Param(idx)` reads would still come from the stale
  /// home slot while the `Store` writes the register — the
  /// loop would never see the update and spin forever.
  /// Cleared per FunDef.
  param_promo_reg: HashMap<u32, Register>,
  /// Inline-storage `[N]T` variables: name → SP-relative
  /// offset of the array block's first byte. The block
  /// holds `[len:8][cap:8][e0:8]...[eN:8]`. `Insn::Store`
  /// memcopies into the block; `Insn::Load` returns the
  /// block's address. Without this, fixed-array
  /// reassignment (`row = next` for `[N]T`) was a pointer
  /// alias — both names ended up referring to the same
  /// underlying literal block, so writes through one
  /// were visible through the other.
  array_var_blocks: HashMap<u32, u32>,
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
  /// Bytes reserved in the current frame for blanket
  /// caller-save spills and overflow-arg staging. Computed
  /// once per function in `enter_function`. Zero for leaf-ish
  /// functions whose only calls are pure zo→zo user calls
  /// (those rely on the register allocator's precise per-call
  /// spill, so the flat X1..X15 blanket is pure waste). See
  /// `compute_caller_save_reserve`.
  caller_save_reserve: u32,
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
  /// Scratch slots `Insn::ArrayPush` saves into around its
  /// realloc / heap-clone BLs. Lives past every struct slot
  /// so a saved `val_reg` pointing into a struct can't alias
  /// the struct's bytes.
  array_push_scratch_base: u32,
  string_format_scratch_base: u32,
  /// Offset from SP of the promoted-register save area —
  /// the top-of-frame region holding the caller's x19..x28
  /// while this function uses them for promoted locals. The
  /// prologue stores each claimed register here; every
  /// return path restores from here before the `add sp`.
  promo_save_base: u32,
  /// Offset from SP of the select-wait scratch area.
  /// Layout: `nchans * 8` bytes of `*mut ZoChan`
  /// pointers immediately followed by an `elem_sz`
  /// output buffer that `_zo_select_wait` writes the
  /// received value into. Sized at allocation time
  /// via `FunctionInfo::select_scratch_size`.
  select_scratch_base: u32,
  /// Next struct slot offset (relative to struct_base).
  next_struct_slot: u32,
  /// Offset (relative to SP) of the shared IO read
  /// buffer for the current function. `None` until the
  /// first `read_file` / `readln` / `read` allocates it;
  /// reused by every subsequent IO read in the function.
  /// Reset at each `FunDef`.
  io_shared_buf_offset: Option<u32>,
  /// Functions that return structs: name -> field count.
  struct_return_fns: HashMap<Symbol, u32>,
  /// Per-variant substituted struct payload fields, for
  /// enum-returning functions. Indexed by discriminant.
  /// Read in the enum-deep-copy block to override the
  /// (still-generic) enum-type variant fields with the
  /// actual struct payload types at this call site.
  enum_payload_struct_fields: EnumPayloadFields,
  /// Set when the last emitted instruction was a math
  /// intrinsic (FSQRT, FRINT*). Result is in D0.
  last_was_math_intrinsic: bool,
  /// External C functions used (ordered, no duplicates).
  /// Each entry is the C symbol name (e.g. "_pow", "_malloc").
  /// Iteration order is preserved — the GOT layout depends
  /// on it.
  extern_used: Vec<String>,
  /// Membership shadow of `extern_used` for O(1) "did we
  /// already register this stub?" checks. C symbol names
  /// are interned `&'static str` (or owned heap strings),
  /// not dense u32 ids — so we use a hash set, not a
  /// `DenseSet`. This is the one place where the small
  /// HashSet really is the right call.
  extern_used_set: HashSet<String>,
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
  call_fixups: Vec<(u32, (Symbol, Option<Symbol>))>,
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
  /// Struct element type of a `Vec` access, keyed by the
  /// call's `ValueId`. Cloned from `Sir::vec_elem_tys`; see
  /// it for why the side channel exists.
  vec_elem_tys: HashMap<u32, TyId>,
  /// Per-array metadata keyed by the array's `TyId.0`.
  /// Populated by the pre-pass in `generate` from
  /// `Insn::ArrayTyDef`. Drives `emit_array_write` (uses
  /// `elem_ty`) and `Insn::ArrayLiteral`'s stack-vs-heap
  /// branch (uses `size`). Type-checker rewrites a
  /// literal's `ty_id` from `[N]T` to `[]T` when the
  /// binding annotation is dynamic, so a `size = Some(_)`
  /// hit here really does mean the literal won't be
  /// `push`ed.
  array_metas: HashMap<u32, ArrayMeta>,
  /// HashMap type → `(key_fmt, val_fmt)` `MapFmt`
  /// discriminants, keyed by the map's `TyId.0`.
  /// Populated by the same pre-pass that fills
  /// `array_metas`, but reading `Insn::MapTyDef`. Drives
  /// `emit_map_write` so `showln(m)` calls
  /// `_zo_map_show` with the right per-side scalar
  /// kinds — without it the receiver pointer falls
  /// through to `itoa` and prints as a raw address.
  map_metas: HashMap<u32, (u32, u32)>,
  /// `Vec<$T>` type → element `MapFmt` discriminant,
  /// keyed by the vec's `TyId.0`. Populated by the same
  /// pre-pass that fills `array_metas`, reading
  /// `Insn::VecTyDef`. Drives `emit_vec_write` so
  /// `showln(v)` calls `_zo_vec_show` with the right
  /// element kind — without it the receiver struct
  /// pointer routes through the generic struct printer
  /// and prints `Vec { ptr: <int> }`.
  vec_metas: HashMap<u32, u32>,
  /// `HashSet<$K>` type → key `MapFmt` discriminant,
  /// keyed by the set's `TyId.0`. Same role as
  /// `vec_metas`, reading `Insn::SetTyDef`.
  set_metas: HashMap<u32, u32>,
  /// Per-construction concrete payload types, keyed by the
  /// `EnumConstruct.dst` ValueId. Each entry pins down the
  /// variant index and the concrete `TyId` of every payload
  /// slot at that construction site. The generic `enum_metas`
  /// only sees the enum template's `Ty::Infer($T)` field
  /// types, so without this override `showln(Maybe<str>::Some)`
  /// would dispatch through `emit_itoa_and_write` and leak the
  /// str header pointer. Propagated through `Store`/`Load` via
  /// `local_enum_field_tys` so a `showln(m2)` later in the
  /// function still sees the construction-site types.
  value_enum_field_tys: HashMap<u32, (u32, Vec<TyId>)>,
  /// SP-relative offset of every `EnumConstruct` /
  /// `StructConstruct` payload, keyed by its `dst` value-id.
  /// `Insn::Return` falls back to this when the register
  /// allocator didn't assign a GP register to the composite
  /// value; without the fallback X0 holds stale data and
  /// the caller's deep-copy reads garbage.
  composite_value_slots: HashMap<ValueId, u32>,
  /// Mirror of `value_enum_field_tys` keyed by local-variable
  /// `Symbol.as_u32()`. Populated whenever an `Insn::Store`
  /// writes a value that has a `value_enum_field_tys` entry,
  /// consumed by `Insn::Load { src: LoadSource::Local(_) }`
  /// to copy the override onto the loaded SSA destination.
  local_enum_field_tys: HashMap<u32, (u32, Vec<TyId>)>,
  /// Per-construction tuple element types keyed by the
  /// `Insn::TupleLiteral.dst` SSA value-id. Tuples carry
  /// their element types only on `Ty::Tuple(TupleTyId)` in
  /// `zo-ty`, which the codegen has no handle to — so the
  /// pretty-printer pulls them out of the construction site
  /// instead. Propagated through `Store`/`Load` via
  /// `local_tuple_elem_tys`.
  value_tuple_elem_tys: HashMap<u32, Vec<TyId>>,
  /// Mirror of `value_tuple_elem_tys` keyed by local-variable
  /// `Symbol.as_u32()`. Same Store-then-Load forwarding shape
  /// as `local_enum_field_tys`.
  local_tuple_elem_tys: HashMap<u32, Vec<TyId>>,
  /// Struct metadata keyed by `TyId.0`, populated on each
  /// `Insn::StructDef`. Drives the pretty-printer in
  /// `emit_struct_write` so `showln(p)` emits
  /// `Point { x: 10, y: 20 }` instead of leaking the pointer
  /// through `itoa`.
  struct_metas: HashMap<u32, StructMeta>,
  /// Scratch buffer reused by `emit_enum_walk_from_x19`
  /// for per-variant `B` (jump-to-end) fixups. Owned by
  /// the codegen so capacity is retained across enum
  /// walks instead of reallocating a fresh `Vec` per
  /// `showln(enum)` site.
  enum_walk_done_fixups: Vec<usize>,
  /// `ValueId` → defining `InsnIdx` lookup, populated once
  /// per generate() call before instruction emission.
  /// Replaces the `for i in (0..idx).rev()` linear scan
  /// in `materialize_value_into_x16` (called per element
  /// of every aggregate literal — `[N]T`, `(t1, t2, ...)`,
  /// `Point { x, y, ... }`, `Loot::Gold(50)`). Without
  /// this, large literals were O(N²) on the SIR stream.
  /// Direct `Vec<InsnIdx>` load — no hashing.
  value_def_idx: DenseMap<ValueId, InsnIdx>,
  /// Keys inserted into `value_def_idx` since the last
  /// reset. `enter_function` resets the map per function;
  /// resetting only these keys keeps that O(entries) instead
  /// of O(max global ValueId) — `DenseMap::clear` walks its
  /// whole backing `Vec`, which the global pre-pass grows to
  /// the program's max ValueId (~2.4M slots on a 500K-line
  /// file), making a per-function full clear O(functions ×
  /// values) = O(n²).
  value_def_dirty: Vec<ValueId>,
  /// FFI signatures by symbol — populated in the pre-pass
  /// by scanning every `Insn::FunDef` whose `kind` is
  /// `FunctionKind::Intrinsic`. The `_` arm of
  /// `Insn::Call`'s dispatch looks here before falling
  /// through to the user-function path: a hit triggers
  /// the generic AAPCS path (`abi::classify` +
  /// `emit_ffi_call`).
  ffi_sigs: HashMap<Symbol, FfiSig>,
  /// `link_name` attribute payload per `pub ffi` —
  /// populated in the same pre-pass that builds
  /// `ffi_sigs`. The call-site dispatch reads it to pick
  /// the right C symbol for `BL <c_sym>`.
  ffi_link_names: HashMap<Symbol, Symbol>,
  /// Per-FFI host-resolved dylib path (`#link { macos: ...,
  /// linux: ..., windows: ... }` from the declaring pack).
  /// Built in the pre-pass; consumed by `into_link_object`
  /// so the linker can route `LC_LOAD_DYLIB` + bind
  /// ordinals from per-pack metadata instead of hardcoded
  /// symbol-name predicates.
  extern_dylib_paths: HashMap<String, String>,
  /// Read-only view of the type system needed by the
  /// classifier (struct field walks for HFA, etc.).
  /// `None` when the orchestrator didn't supply a
  /// `TyChecker` — the generic FFI fallback stays off in
  /// that case and per-symbol arms remain authoritative.
  type_view: Option<TypeViewStored<'a>>,
  /// Whether `#render` emits a call to `_zo_run_web` (wry) instead of
  /// `_zo_run_native` (eframe). The SIR is identical either way; only
  /// the runtime entry symbol differs.
  webviewing: Webviewing,
}

/// Slice-only mirror of `abi::TypeQuery`, owned by the
/// ARM64Gen so the lifetime is tied to the codegen
/// instance instead of a transient call.
#[derive(Clone, Copy)]
struct TypeViewStored<'a> {
  tys: &'a [Ty],
  ty_table: &'a TyTable,
}

/// Per-FFI signature derived from `Insn::FunDef`'s
/// `params` + `return_ty`. Kept as bare `TyId` lists so
/// the classifier can be called per call-site without
/// re-walking the SIR.
struct FfiSig {
  params: Vec<TyId>,
  return_ty: TyId,
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

impl EnumMeta {
  /// Snapshot variants into the owned shape consumed by the
  /// pretty-printer walker. Owned because the walker mutates
  /// `self` while iterating, which would conflict with a live
  /// `&self.enum_metas` borrow. Callers that need to override
  /// per-construction-site payload types (generic enums) mutate
  /// the returned `Vec` before passing it to the walker.
  fn variants_view(&self) -> Vec<(u32, Symbol, Vec<TyId>)> {
    self
      .variants
      .iter()
      .map(|v| (v.discriminant, v.display_sym, v.field_tys.clone()))
      .collect()
  }
}

/// Enum payload layout: `[disc:u64][f0:u64][f1:u64]...`. Field
/// `i` lives at slot `i + ENUM_PAYLOAD_BASE_SLOT` from the
/// pointer in X19, so byte offset is
/// `(i + 1) * STACK_SLOT_SIZE`.
const ENUM_PAYLOAD_BASE_SLOT: i16 = 1;

const ENUM_OPEN_PAREN_SYM: Symbol = Symbol(0xE000_FFFC);
const ENUM_COMMA_SPACE_SYM: Symbol = Symbol(0xE000_FFFD);
const ENUM_CLOSE_PAREN_SYM: Symbol = Symbol(0xE000_FFFE);

const ARRAY_OPEN_BRACKET_SYM: Symbol = Symbol(0xE000_FFF8);
const ARRAY_CLOSE_BRACKET_SYM: Symbol = Symbol(0xE000_FFF9);

const STR_DQUOTE_SYM: Symbol = Symbol(0xE000_FFF7);

const TUPLE_OPEN_PAREN_SYM: Symbol = Symbol(0xE000_FFF6);
const TUPLE_CLOSE_PAREN_SYM: Symbol = Symbol(0xE000_FFF5);

/// `" }"` — close marker for the struct pretty-printer. Shared
/// across every struct because the trailing space + brace is
/// identical regardless of struct name.
const STRUCT_CLOSE_BRACE_SYM: Symbol = Symbol(0xE000_FFF4);

/// Per-struct pretty-printer metadata. One entry per `StructDef`
/// seen by the codegen. `header_sym` owns the pre-baked
/// `"StructName { "` string in `string_data`; each entry in
/// `fields` owns a `"field_name = "` label and the field's type
/// so `emit_struct_write` can dispatch through the field-type's
/// writer.
struct StructMeta {
  header_sym: Symbol,
  fields: Vec<StructFieldMeta>,
}

struct StructFieldMeta {
  label_sym: Symbol,
  ty_id: TyId,
}

/// Per-array metadata stored in `ARM64Gen::array_metas`.
#[derive(Clone, Copy)]
struct ArrayMeta {
  elem_ty: TyId,
  /// `Some(N)` for `[N]T` (stack-allocatable in
  /// `Insn::ArrayLiteral`), `None` for `[]T` (heap).
  size: Option<u32>,
}

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
      string_data_seen: HashSet::default(),
      current_function: None,
      string_fixups: Vec::new(),
      function_addr_fixups: Vec::new(),
      vtable_data: Vec::new(),
      vtable_fixups: Vec::new(),
      vtable_addr_fixups: Vec::new(),
      abstract_defs: HashMap::default(),
      abstract_impls: HashMap::default(),
      template_data: Vec::new(),
      template_handlers: HashMap::default(),
      reactive_slots: HashMap::default(),
      template_text_bindings: HashMap::default(),
      reactive_arr_slots: HashSet::default(),
      template_list_bindings: HashMap::default(),
      template_attr_bindings: HashMap::default(),
      next_recipe_blob: 0,
      fns_needing_calls: HashSet::default(),
      has_templates: false,
      labels: HashMap::default(),
      branch_fixups: Vec::new(),
      reg_alloc: None,
      spill_offsets: Vec::new(),
      reload_overrides: HashMap::default(),
      fp_reload_overrides: HashMap::default(),
      current_fn_start: None,
      current_emit_idx: 0,
      mutable_slots: HashMap::default(),
      promotion: Promotion::default(),
      promo_value_reg: HashMap::default(),
      param_promo_reg: HashMap::default(),
      array_var_blocks: HashMap::default(),
      param_slots: HashMap::default(),
      param_sym_slots: HashMap::default(),
      caller_save_base: 0,
      caller_save_reserve: 0,
      next_mut_slot: 0,
      struct_base: 0,
      chan_scratch_base: 0,
      select_scratch_base: 0,
      array_push_scratch_base: 0,
      string_format_scratch_base: 0,
      promo_save_base: 0,
      next_struct_slot: 0,
      io_shared_buf_offset: None,
      struct_return_fns: HashMap::default(),
      enum_payload_struct_fields: HashMap::default(),
      last_was_math_intrinsic: false,
      extern_used: Vec::new(),
      extern_used_set: HashSet::default(),
      extern_stub_offsets: HashMap::default(),
      extern_fixups: Vec::new(),
      call_fixups: Vec::new(),
      enum_metas: HashMap::default(),
      next_enum_sym: ENUM_SYNTHETIC_SYM_BASE,
      value_types: HashMap::default(),
      vec_elem_tys: HashMap::default(),
      array_metas: HashMap::default(),
      map_metas: HashMap::default(),
      vec_metas: HashMap::default(),
      set_metas: HashMap::default(),
      value_enum_field_tys: HashMap::default(),
      composite_value_slots: HashMap::default(),
      local_enum_field_tys: HashMap::default(),
      value_tuple_elem_tys: HashMap::default(),
      local_tuple_elem_tys: HashMap::default(),
      struct_metas: HashMap::default(),
      enum_walk_done_fixups: Vec::new(),
      value_def_idx: DenseMap::new(),
      value_def_dirty: Vec::new(),
      ffi_sigs: HashMap::default(),
      ffi_link_names: HashMap::default(),
      extern_dylib_paths: HashMap::default(),
      type_view: None,
      webviewing: Webviewing::No,
    }
  }

  /// Select the webview runtime entry (`_zo_run_web`) for `#render`
  /// instead of the native one. Set by the orchestrator for a
  /// `--target webview` build.
  pub fn with_webviewing(mut self, webviewing: Webviewing) -> Self {
    self.webviewing = webviewing;
    self
  }

  /// Attach the type-system view needed by the generic
  /// AAPCS FFI path (struct field walks for HFA, etc.).
  /// When set, the `_` arm of `Insn::Call`'s dispatch
  /// falls back to `abi::classify` + `emit_ffi_call` for
  /// any symbol that resolves to a `FunctionKind::Intrinsic`
  /// `Insn::FunDef`. Without this, the FFI fallback stays
  /// off and per-symbol arms remain authoritative.
  pub fn with_type_view(
    mut self,
    tys: &'a [Ty],
    ty_table: &'a TyTable,
  ) -> Self {
    self.type_view = Some(TypeViewStored { tys, ty_table });
    self
  }

  /// Threads the executor's abstract registry into
  /// codegen so `emit_vtables` and `Insn::CoerceToDyn`
  /// lowering can resolve `(Abstract, ConcreteType)`
  /// pairs.
  pub fn with_abstract_state(
    mut self,
    abstract_defs: HashMap<Symbol, AbstractDef>,
    abstract_impls: HashMap<(Symbol, Symbol), AbstractImpl>,
  ) -> Self {
    self.abstract_defs = abstract_defs;
    self.abstract_impls = abstract_impls;
    self
  }

  // --- Register allocation helpers ---

  /// Look up the allocated register for a ValueId.
  fn alloc_reg(&self, vid: ValueId) -> Option<Register> {
    // A value loaded from a promoted local lives in its
    // callee-saved register — return it directly so consumers
    // read the register with no intervening memory load.
    if let Some(&reg) = self.promo_value_reg.get(&vid.0) {
      return Some(reg);
    }

    self
      .reg_alloc
      .as_ref()
      .and_then(|a| a.get_at(self.current_emit_idx, vid))
      .map(Register::new)
      .or_else(|| {
        let fn_start = self.current_fn_start? as u32;
        let key = (fn_start, vid.0);

        if let Some(&reg) = self.reload_overrides.get(&key) {
          return Some(Register::new(reg));
        }

        self
          .reg_alloc
          .as_ref()
          .and_then(|a| a.get(fn_start, vid))
          .map(Register::new)
      })
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
      .and_then(|a| a.get_fp_at(self.current_emit_idx, vid))
      .map(FpRegister::new)
      .or_else(|| {
        let fn_start = self.current_fn_start? as u32;
        let key = (fn_start, vid.0);

        if let Some(&reg) = self.fp_reload_overrides.get(&key) {
          return Some(FpRegister::new(reg));
        }

        self
          .reg_alloc
          .as_ref()
          .and_then(|a| a.get_fp(fn_start, vid))
          .map(FpRegister::new)
      })
  }

  /// Bind per-function state at the start of every FunDef:
  /// records the current function context, then resets every
  /// frame-local map / counter so leftovers from the
  /// previous function can't alias. Pairs with the
  /// per-function rebuild of `value_def_idx` over the
  /// function's SIR range (ValueId counters reset per
  /// function, so the flat map would alias vid=N).
  /// Insert into `value_def_idx`, tracking the key so the
  /// next `reset_value_def_idx` resets only touched slots.
  fn value_def_insert(&mut self, key: ValueId, idx: InsnIdx) {
    self.value_def_idx.insert(key, idx);
    self.value_def_dirty.push(key);
  }

  /// Reset only the slots touched since the last reset —
  /// O(entries), not O(max global ValueId).
  fn reset_value_def_idx(&mut self) {
    for key in self.value_def_dirty.drain(..) {
      self.value_def_idx.remove(key);
    }
  }

  /// Whether an `Insn::Call` to `name` lowers to a pure
  /// zo→zo user-function `BL` — the only call path that
  /// relies entirely on the register allocator's precise
  /// per-call spill and therefore needs NO blanket caller-
  /// save reserve.
  ///
  /// @note — mirrors the `Insn::Call` dispatch: every builtin
  /// / runtime / libm / FFI arm wraps its `BL` in
  /// `emit_extern_call` (or the libm inline blanket), so those
  /// keep the reserve. `flush` / `exit` and the hardware-math
  /// intrinsics emit no `BL` at all, but classifying them as
  /// "needs reserve = false" is safe (and they never co-exist
  /// with a reason to grow the frame). The conservative
  /// default for an unrecognized name is `false` only when it
  /// is also absent from `ffi_sigs`; an FFI signature forces
  /// the reserve.
  fn call_is_pure_zo(&self, name: Symbol) -> bool {
    if self.ffi_sigs.contains_key(&name) {
      return false;
    }

    match self.interner.get(name) {
      // No-`BL` intrinsics — no caller-save needed.
      "flush" | "exit" | "sqrt" | "floor" | "ceil" | "trunc" | "round"
      | "is_nan" | "is_finite" => true,
      // Everything that wraps a runtime `BL` in
      // `emit_extern_call` keeps the blanket reserve.
      "show" | "showln" | "eshow" | "eshowln" | "check" | "exists"
      | "read_file" | "write_file" | "append_file" | "readln" | "read"
      | "args" | "remove_file" | "read_dir" | "pow" | "sin" | "cos" | "tan"
      | "log" | "log2" | "log10" | "exp" | "zo_str_replace"
      | "arr_int::sort" | "zo_map_len_raw" | "zo_map_free_raw"
      | "zo_vec_len_raw" | "zo_vec_free_raw" | "zo_set_len_raw"
      | "zo_set_free_raw" => false,
      // `Type::method` runtime dispatch (HashMap / Vec /
      // HashSet) and any other `::`-mangled builtin lowers
      // through a marshaling `emit_extern_call`.
      other => !other.contains("::"),
    }
  }

  /// Whether a non-`Call` SIR instruction lowers to a runtime
  /// `BL` that the register allocator does NOT model with
  /// precise per-call spills — so its blanket `emit_extern_call`
  /// caller-save is load-bearing and forces the reserve.
  fn insn_needs_blanket_caller_save(insn: &Insn) -> bool {
    matches!(
      insn,
      Insn::ArrayLiteral { .. }
        | Insn::ArrayPush { .. }
        | Insn::ChannelCreate { .. }
        | Insn::ChannelSend { .. }
        | Insn::ChannelRecv { .. }
        | Insn::ChannelClose { .. }
        | Insn::TaskSpawn { .. }
        | Insn::TaskAwait { .. }
        | Insn::TaskCancelled { .. }
        | Insn::TaskCancel { .. }
        | Insn::NurseryEnd { .. }
        | Insn::SelectWait { .. }
        | Insn::SelectRecv { .. }
        | Insn::StrSlice { .. }
        | Insn::ToStr { .. }
        | Insn::StringFormat { .. }
        | Insn::CoerceToDyn { .. }
        | Insn::DynDispatch { .. }
        | Insn::TestBegin { .. }
        | Insn::TestRun { .. }
        | Insn::TestSummary
    ) || matches!(
      insn,
      Insn::BinOp { ty_id, op, .. }
        if ty_id.0 == STR_TYPE_ID
          && matches!(
            op,
            zo_sir::BinOp::Eq | zo_sir::BinOp::Neq | zo_sir::BinOp::Concat
          )
    )
  }

  /// Whether an instruction accesses a reactive `mut` slot and
  /// therefore lowers to a state-helper `BL` (`emit_state_load`
  /// / `emit_state_store`, both via `emit_extern_call`). Like
  /// the explicit runtime instructions, that blanket spill is
  /// load-bearing — without the reserve its `str x1..x15`
  /// overruns the frame into the saved x29/x30 record.
  ///
  /// @note — mirrors the reactive dispatch in `Insn::Load` /
  /// `Insn::Store`: a `LoadSource::Local`/`Store` whose symbol
  /// is in `reactive_slots` routes through the runtime dylib
  /// instead of the stack frame. The plain (non-reactive) form
  /// of either is a register move or `ldr`/`str` with no `BL`.
  fn insn_is_reactive_access(&self, insn: &Insn) -> bool {
    match insn {
      Insn::Load {
        src: LoadSource::Local(sym),
        ..
      } => self.reactive_slots.contains_key(sym),
      Insn::Store { name, .. } => self.reactive_slots.contains_key(name),
      _ => false,
    }
  }

  /// Total stack frame size in bytes, 16-byte aligned. The
  /// single source of truth shared by the prologue's `sub sp`
  /// and the epilogue's `add sp`.
  fn aligned_frame_size(areas: FrameAreas) -> u32 {
    (areas.spill_size
      + areas.mut_size
      + areas.param_reserve
      + areas.caller_save
      + areas.struct_size
      + areas.chan_scratch_size
      + areas.select_scratch_size
      + ARRAY_PUSH_SCRATCH_SIZE
      + areas.string_format_scratch_size
      + areas.promo_save_size
      + FRAME_ALIGN_MASK)
      & !FRAME_ALIGN_MASK
  }

  /// Bytes the promoted-register save area occupies in the
  /// current frame: one 8-byte slot per claimed callee-saved
  /// register, rounded up to 16 so the frame stays aligned.
  /// Single source of truth for the prologue's save loop and
  /// the epilogue's restore loop.
  fn promo_save_size(&self) -> u32 {
    let n = self.promotion.used_count() as u32;

    (n * STACK_SLOT_SIZE + FRAME_ALIGN_MASK) & !FRAME_ALIGN_MASK
  }

  /// Bytes the current function must reserve for blanket
  /// caller-save spills and overflow-arg staging, replacing
  /// the flat `CALLER_SAVE_RESERVE` that every call-having
  /// function paid regardless of need.
  ///
  /// @note — A function needs the full `CALLER_SAVE_RESERVE`
  /// only when it still emits a blanket spill: any runtime-
  /// lowered non-`Call` instruction, or any builtin / libm /
  /// FFI `Call`. Pure zo→zo user calls are covered by the
  /// allocator's precise spills, so they need zero blanket —
  /// only enough staging for their own >8-arg overflow.
  fn compute_caller_save_reserve(&self, body: &[Insn]) -> u32 {
    let mut needs_blanket = false;
    let mut max_overflow_args: u32 = 0;

    for insn in body {
      if Self::insn_needs_blanket_caller_save(insn)
        || self.insn_is_reactive_access(insn)
      {
        needs_blanket = true;
      }

      if let Insn::Call { name, args, .. } = insn {
        if !self.call_is_pure_zo(*name) {
          needs_blanket = true;
        }

        let overflow = args.len().saturating_sub(MAX_REG_ARGS) as u32;

        if overflow > max_overflow_args {
          max_overflow_args = overflow;
        }
      }

      // Indirect calls marshal overflow args through the same
      // staging area, so they must reserve it too. Missing this
      // left the staging slots un-reserved when a frame's only
      // overflow call was indirect.
      if let Insn::CallIndirect { args, .. } = insn {
        let overflow = args.len().saturating_sub(MAX_REG_ARGS) as u32;

        if overflow > max_overflow_args {
          max_overflow_args = overflow;
        }
      }
    }

    // Overflow args stage past the X1..X15 blanket region at
    // `caller_save_base + (CALLER_SAVE_COUNT + 1) * 8`, so the
    // staging reserve always includes that prefix. The offset
    // formula at the call site is unchanged — only the SIZE
    // reserved here adapts.
    let staging = if max_overflow_args > 0 {
      (CALLER_SAVE_COUNT as u32 + 1 + max_overflow_args) * STACK_SLOT_SIZE
    } else {
      0
    };

    if needs_blanket {
      CALLER_SAVE_RESERVE.max(staging)
    } else {
      staging
    }
  }

  fn enter_function(&mut self, name: Symbol, idx: usize, all_insns: &[Insn]) {
    self.current_function = Some(name);
    self.current_fn_start = Some(idx);

    self.value_types.clear();
    self.reset_value_def_idx();
    self.reload_overrides.clear();
    self.fp_reload_overrides.clear();
    self.mutable_slots.clear();
    self.promo_value_reg.clear();
    self.param_promo_reg.clear();
    self.array_var_blocks.clear();
    self.next_mut_slot = 0;
    self.next_struct_slot = 0;
    self.io_shared_buf_offset = None;
    self.param_slots.clear();
    self.param_sym_slots.clear();
    self.local_enum_field_tys.clear();
    self.local_tuple_elem_tys.clear();
    self.composite_value_slots.clear();

    let fn_end = all_insns[idx + 1..]
      .iter()
      .position(|ins| matches!(ins, Insn::FunDef { .. }))
      .map(|p| idx + 1 + p)
      .unwrap_or(all_insns.len());

    self.caller_save_reserve =
      self.compute_caller_save_reserve(&all_insns[idx..fn_end]);

    self.promotion = self.build_promotion(&all_insns[idx..fn_end]);

    for (offset, ins) in all_insns[idx..fn_end].iter().enumerate() {
      let widx = InsnIdx((idx + offset) as u32);

      match ins {
        Insn::ConstInt { dst, .. }
        | Insn::ConstFloat { dst, .. }
        | Insn::ConstBool { dst, .. }
        | Insn::Load { dst, .. }
        | Insn::TupleIndex { dst, .. } => {
          self.value_def_insert(*dst, widx);
        }
        _ => {}
      }
    }
  }

  /// Build the register-promotion plan for the function body
  /// `body`. A scalar local store/load type is promotable
  /// when it resolves to a register-sized GP scalar (`int`,
  /// `s8..s64`, `u8..u64`, `bool`, `char`, or a pointer).
  /// Without a type view the classifier promotes nothing —
  /// correctness over reach. The native build path always
  /// supplies a type view, so promotion is active there.
  ///
  /// Promotion is disabled outright for any function whose
  /// body clobbers x19..x28 (see
  /// `body_clobbers_promotion_regs`).
  fn build_promotion(&self, body: &[Insn]) -> Promotion {
    let view = self.type_view;
    let regs_safe = !self.body_clobbers_promotion_regs(body);

    Promotion::analyze(
      body,
      |ty_id| match view {
        Some(view) => {
          matches!(
            resolve_ty(view.tys, TyId(ty_id)),
            Ty::Int { .. } | Ty::Bool | Ty::Char | Ty::Ref(_)
          )
        }
        None => false,
      },
      regs_safe,
    )
  }

  /// Whether `body` contains an instruction whose codegen
  /// uses x19..x28 as ad-hoc scratch. Those registers double
  /// as the promotion pool, so any such instruction would
  /// stomp a promoted local — the function must promote
  /// nothing.
  ///
  /// This is a WHITELIST: a function is safe only when every
  /// instruction is one we have verified never touches
  /// x19..x28 (scalar arithmetic, control flow, plain calls,
  /// scalar prints). Anything else — arrays, strings, enums,
  /// tuples, structs, channels, tasks, IO, dynamic dispatch —
  /// disqualifies the whole function. Correct-by-
  /// construction: an instruction we don't recognize as safe
  /// is assumed to clobber. The hot kernels this optimization
  /// targets (tight numeric loops) consist entirely of the
  /// safe set, so they still promote; aggregate-heavy code
  /// keeps its memory locals.
  fn body_clobbers_promotion_regs(&self, body: &[Insn]) -> bool {
    // Value id → producing type, scoped to this body. Lets a
    // `show*` call recover whether its argument is an
    // aggregate; the per-emit `value_types` map isn't
    // populated yet at promotion-analysis time.
    let mut value_ty: HashMap<u32, TyId> = HashMap::default();

    body.iter().any(|insn| {
      match insn {
        Insn::Load { dst, ty_id, .. }
        | Insn::Call { dst, ty_id, .. }
        | Insn::StructConstruct { dst, ty_id, .. }
        | Insn::EnumConstruct { dst, ty_id, .. }
        | Insn::TupleLiteral { dst, ty_id, .. }
        | Insn::ArrayLiteral { dst, ty_id, .. } => {
          value_ty.insert(dst.0, *ty_id);
        }
        _ => {}
      }

      !self.insn_is_promotion_safe(insn, &value_ty)
    })
  }

  /// Whether one instruction's codegen is known to leave
  /// x19..x28 untouched. See `body_clobbers_promotion_regs`.
  fn insn_is_promotion_safe(
    &self,
    insn: &Insn,
    value_ty: &HashMap<u32, TyId>,
  ) -> bool {
    match insn {
      // Scalar value production, arithmetic, control flow,
      // and frame markers — all verified to stay clear of
      // x19..x28.
      Insn::FunDef { .. }
      | Insn::ConstInt { .. }
      | Insn::ConstBool { .. }
      | Insn::ConstFloat { .. }
      | Insn::ConstString { .. }
      | Insn::Load { .. }
      | Insn::Store { .. }
      | Insn::BinOp { .. }
      | Insn::UnOp { .. }
      | Insn::Cast { .. }
      | Insn::ToStr { .. }
      | Insn::StringFormat { .. }
      | Insn::Label { .. }
      | Insn::Jump { .. }
      | Insn::BranchIfNot { .. }
      | Insn::Return { .. }
      | Insn::VarDef { .. }
      // An indirect call marshals args into x0..x7, loads the
      // callee into x16, and `BLR`s — none of which touch the
      // promotion bank x19..x28. It can never be a `show*`
      // builtin, so it has no aggregate-print hazard.
      | Insn::CallIndirect { .. }
      | Insn::Drop { .. } => true,
      // A plain call is safe; a `show*` of an aggregate routes
      // into the x19..x28 pretty-printer and is not.
      Insn::Call { name, args, .. } => {
        let is_print = matches!(
          self.interner.get(*name),
          "show" | "showln" | "eshow" | "eshowln"
        );

        !(is_print
          && args
            .first()
            .and_then(|arg| value_ty.get(&arg.0).copied())
            .is_some_and(|ty| self.is_aggregate_print_ty(ty)))
      }
      // Everything else (arrays, channels, tasks, dynamic
      // dispatch, templates, IO scratch, …) may use x19..x28.
      _ => false,
    }
  }

  /// Whether printing a value of type `ty_id` routes through
  /// the x19..x28 aggregate pretty-printer rather than the
  /// scalar / string / float itoa path.
  fn is_aggregate_print_ty(&self, ty_id: TyId) -> bool {
    let Some(view) = self.type_view else {
      // No type view: be conservative and treat it as an
      // aggregate so promotion stays off when in doubt.
      return true;
    };

    matches!(
      resolve_ty(view.tys, ty_id),
      Ty::Struct(_) | Ty::Enum(_) | Ty::Tuple(_) | Ty::Array(_)
    )
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
    // `bytes` shares the `[len:u64][bytes]` layout with
    // `str` so the same print path works — both produce a
    // pointer to a length-prefixed buffer.
    self
      .type_of(vid)
      .is_some_and(|ty| ty.0 == STR_TYPE_ID || ty.0 == BYTES_TYPE_ID)
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

    self.array_metas.get(&ty.0).map(|m| m.elem_ty)
  }

  /// If `vid`'s type is a registered HashMap, return the
  /// `(key_fmt, val_fmt)` pair as `MapFmt` discriminants.
  /// `None` for non-maps. Same fall-through semantics as
  /// `is_array_value`: a missing entry simply means the
  /// type isn't a map this codegen knows how to walk.
  fn is_map_value(&self, vid: ValueId) -> Option<(u32, u32)> {
    let ty = self.type_of(vid)?;

    self.map_metas.get(&ty.0).copied()
  }

  /// If `vid`'s type is a registered `Vec<$T>`, return the
  /// element `MapFmt` discriminant. Same fall-through
  /// semantics as `is_map_value`.
  fn is_vec_value(&self, vid: ValueId) -> Option<u32> {
    let ty = self.type_of(vid)?;

    self.vec_metas.get(&ty.0).copied()
  }

  /// If `vid`'s type is a registered `HashSet<$K>`, return
  /// the key `MapFmt` discriminant.
  fn is_set_value(&self, vid: ValueId) -> Option<u32> {
    let ty = self.type_of(vid)?;

    self.set_metas.get(&ty.0).copied()
  }

  /// Returns the tuple's element `TyId`s when `vid` came
  /// from a known `Insn::TupleLiteral` site (or was loaded
  /// from a local that did). The tuple type encoding lives
  /// in `zo-ty`, which the codegen has no handle to — so
  /// the per-site override populated at construction time
  /// is the only source.
  fn is_tuple_value(&self, vid: ValueId) -> Option<Vec<TyId>> {
    // Returns by value rather than `&[TyId]` because the
    // caller passes the result into `&mut self`-taking
    // emit fns; a slice borrow off `&self` would conflict
    // with that mutable borrow. The cloned `Vec` is small
    // and only built per `showln(tuple)` arg — not on a
    // hot loop.
    self.value_tuple_elem_tys.get(&vid.0).cloned()
  }

  /// If `vid`'s type is a registered struct type, return its
  /// `TyId`. `None` for non-structs and for struct types whose
  /// `StructDef` wasn't surfaced (defensive — every struct type
  /// reached by SIR should have one).
  fn is_struct_value(&self, vid: ValueId) -> Option<TyId> {
    let ty = self.type_of(vid)?;

    if self.struct_metas.contains_key(&ty.0) {
      Some(ty)
    } else {
      None
    }
  }

  /// Spill caller-save GP registers (X1..X15) to frame.
  ///
  /// @note — pair with `emit_caller_save_reload` around any
  /// `BL` whose callee may clobber them.
  fn emit_caller_save_spill(&mut self) {
    let base = self.caller_save_base;

    for i in 0..CALLER_SAVE_COUNT {
      let reg = Register::new(CALLER_SAVE_START + i as u8);
      let off = base + i as u32 * STACK_SLOT_SIZE;

      self.emit_str_sp(reg, off);
    }
  }

  /// Reload caller-save GP registers (X1..X15) from frame.
  fn emit_caller_save_reload(&mut self) {
    let base = self.caller_save_base;

    for i in 0..CALLER_SAVE_COUNT {
      let reg = Register::new(CALLER_SAVE_START + i as u8);
      let off = base + i as u32 * STACK_SLOT_SIZE;

      self.emit_ldr_sp(reg, off);
    }
  }

  /// Emit a `BL` to `c_sym` and register it for GOT binding.
  ///
  /// @note — caller is responsible for surrounding caller-
  /// save spill/reload (see `emit_extern_call` for the
  /// common case).
  fn emit_extern_bl(&mut self, c_sym: &str) {
    let fixup_pos = self.emitter.current_offset();
    let sym = c_sym.to_owned();

    self.emitter.emit_bl(0);
    self.extern_fixups.push((fixup_pos, sym.clone()));

    if self.extern_used_set.insert(sym.clone()) {
      self.extern_used.push(sym);
    }
  }

  /// Emit a `BL` with caller-save spill/reload around it.
  ///
  /// @note — fine for FFIs whose return lives in a register
  /// the reload doesn't touch (X0 GP, D0 FP). Composite
  /// returns whose upper-register payload (X1) overlaps the
  /// reload set must split via `emit_caller_save_spill` +
  /// `emit_extern_bl` + manual lift + `emit_caller_save_reload`.
  fn emit_extern_call(&mut self, c_sym: &str) {
    self.emit_caller_save_spill();
    self.emit_extern_bl(c_sym);
    self.emit_caller_save_reload();
  }

  fn emit_spill_op(&mut self, kind: &SpillKind) {
    match kind {
      SpillKind::Store {
        reg,
        slot,
        class: RegisterClass::GP,
        ..
      } => self.emit_str_sp(Register::new(*reg), *slot * STACK_SLOT_SIZE),
      SpillKind::Load {
        reg,
        slot,
        class: RegisterClass::GP,
        vid,
      } => {
        self.emit_ldr_sp(Register::new(*reg), *slot * STACK_SLOT_SIZE);

        if let Some(fn_start) = self.current_fn_start {
          self.reload_overrides.insert((fn_start as u32, *vid), *reg);
        }
      }
      SpillKind::Store {
        reg,
        slot,
        class: RegisterClass::FP,
        ..
      } => self.emit_str_fp_sp(FpRegister::new(*reg), *slot * STACK_SLOT_SIZE),
      SpillKind::Load {
        reg,
        slot,
        class: RegisterClass::FP,
        vid,
      } => {
        self.emit_ldr_fp_sp(FpRegister::new(*reg), *slot * STACK_SLOT_SIZE);

        if let Some(fn_start) = self.current_fn_start {
          self
            .fp_reload_overrides
            .insert((fn_start as u32, *vid), *reg);
        }
      }
    }
  }

  /// Emit spill ops for instruction `idx` with given
  /// timing (before or after).
  fn emit_spills(&mut self, idx: usize, timing: EmitTiming) {
    if self.reg_alloc.is_none() || idx + 1 >= self.spill_offsets.len() {
      return;
    }

    let start = self.spill_offsets[idx] as usize;
    let end = self.spill_offsets[idx + 1] as usize;

    // Per-insn bucket is tiny (typically 0–5 entries),
    // so the inline timing filter is cheap and avoids a
    // second offset table.
    for k in start..end {
      let op = &self.reg_alloc.as_ref().unwrap().spill_ops[k];

      if op.timing != timing {
        continue;
      }

      let kind = op.kind.clone();

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

  /// Emit `ADD dst, SP, #offset`.
  ///
  /// Slow path materializes the offset into `dst` itself —
  /// never X16. Earlier revisions used X16 as a hidden
  /// temp, which silently corrupted any caller-held value
  /// in X16 (e.g. a freshly-built tag word about to be
  /// stored). `dst` is always a caller-supplied scratch in
  /// every call site, so reusing it is safe.
  fn emit_add_sp_offset(&mut self, dst: Register, offset: u32) {
    if offset <= 4095 {
      self.emitter.emit_add_imm(dst, SP, offset as u16);
    } else {
      self.emit_mov_imm_64(dst, offset as u64);
      self.emitter.emit_add_ext(dst, SP, dst);
    }
  }

  /// Emit `STR src, [SP, #offset]`. Falls back to a
  /// computed address through a scratch register when the
  /// offset overflows the inline-encodable range.
  ///
  /// `emit_str` accepts an `i16` scaled by 8, so any
  /// 8-byte-aligned offset up to 32760 fits the fast
  /// path. Above that — or for unaligned offsets — fall
  /// through to a materialized address in a scratch
  /// register that must not alias `src` (the value being
  /// stored). The scratch is X17 when `src` is X16 and
  /// X16 otherwise.
  fn emit_str_sp(&mut self, src: Register, offset: u32) {
    if offset <= 32760 && offset.is_multiple_of(8) {
      self.emitter.emit_str(src, SP, offset as i16);
    } else {
      let scratch = if src == X16 { X17 } else { X16 };

      self.emit_add_sp_offset(scratch, offset);
      self.emitter.emit_str(src, scratch, 0);
    }
  }

  /// Emit `LDR dst, [SP, #offset]`. Mirror of
  /// `emit_str_sp`: scaled imm12 reaches 32760, anything
  /// past that or unaligned routes through a scratch
  /// address. Uses `dst` as the scratch since LDR reads
  /// the base before writing the destination.
  fn emit_ldr_sp(&mut self, dst: Register, offset: u32) {
    if offset <= 32760 && offset.is_multiple_of(8) {
      self.emitter.emit_ldr(dst, SP, offset as i16);
    } else {
      self.emit_add_sp_offset(dst, offset);
      self.emitter.emit_ldr(dst, dst, 0);
    }
  }

  /// Emit `STR src, [SP, #offset]` for an FP register.
  /// Same address-range and aliasing rules as
  /// `emit_str_sp` — large or unaligned offsets route
  /// through an X16/X17 GP scratch (FP stores can't take
  /// a GP source, so the scratch only holds the address
  /// and never aliases `src`).
  fn emit_str_fp_sp(&mut self, src: FpRegister, offset: u32) {
    if offset <= 32760 && offset.is_multiple_of(8) {
      self.emitter.emit_str_fp(src, SP, offset as u16);
    } else {
      self.emit_add_sp_offset(X16, offset);
      self.emitter.emit_str_fp(src, X16, 0);
    }
  }

  /// Emit `LDR dst, [SP, #offset]` for an FP register.
  fn emit_ldr_fp_sp(&mut self, dst: FpRegister, offset: u32) {
    if offset <= 32760 && offset.is_multiple_of(8) {
      self.emitter.emit_ldr_fp(dst, SP, offset as u16);
    } else {
      self.emit_add_sp_offset(X16, offset);
      self.emitter.emit_ldr_fp(dst, X16, 0);
    }
  }

  // --- Code generation ---

  /// Generates `ARM64` code from SIR.
  pub fn generate(&mut self, sir: &Sir) -> Artifact {
    // Run register allocation before codegen. The type
    // view is forwarded so the allocator can budget for
    // nested-struct returns (deep-copy at the call site
    // needs more slots than the parent's flat field
    // count).
    let type_view = self.type_view.map(|v| (v.tys, v.ty_table));
    self.reg_alloc = Some(RegAlloc::allocate(AllocInput {
      insns: &sir.instructions,
      next_value_id: sir.next_value_id,
      interner: self.interner,
      type_view,
      vec_elem_tys: &sir.vec_elem_tys,
    }));

    let insns = &sir.instructions;

    // Build per-insn offset table into the (sorted)
    // spill_ops. `spill_offsets[i]` = first spill_op
    // index whose `insn_idx == i`; `spill_offsets[n]` =
    // total spill count. One linear pass.
    {
      let n = insns.len();
      let spill_ops = self
        .reg_alloc
        .as_ref()
        .map(|a| a.spill_ops.as_slice())
        .unwrap_or(&[]);

      self.spill_offsets.clear();
      self.spill_offsets.reserve(n + 1);

      let mut spill_cursor = 0_usize;

      for insn_idx in 0..n {
        while spill_cursor < spill_ops.len()
          && spill_ops[spill_cursor].insn_idx < insn_idx
        {
          spill_cursor += 1;
        }

        self.spill_offsets.push(spill_cursor as u32);
      }

      self.spill_offsets.push(spill_ops.len() as u32);
    }

    // Reuse the deep-slot map regalloc already built. It
    // ran the same FunDef → StructConstruct → Return scan
    // (with the same `flat_struct_slots_of` recursion) to
    // budget stack space; copying the map shares one
    // source of truth. No SIR re-scan.
    if let Some(reg_alloc) = self.reg_alloc.as_ref() {
      self.struct_return_fns = reg_alloc.struct_return_fns.clone();
      self.enum_payload_struct_fields =
        reg_alloc.enum_payload_struct_fields.clone();
    }

    // `collect` rather than `clone`: the SIR map uses the std
    // hasher, codegen's alias is `FxHashMap`.
    self.vec_elem_tys =
      sir.vec_elem_tys.iter().map(|(k, v)| (*k, *v)).collect();

    // Pre-pass: harvest per-pack `#link` paths. Must run
    // BEFORE the fused walk below because that walk
    // resolves each `pub ffi`'s dylib path by looking up
    // its declaring pack — and the pack's `#link` may
    // appear after some of its `pub ffi` declarations in
    // the SIR stream (preload modules go first; ordering
    // within a pack isn't guaranteed).
    let mut pack_dylib: HashMap<Symbol, String> = HashMap::default();

    for insn in insns.iter() {
      // Resolution + diagnostic happened at executor
      // time — codegen just reads the pre-resolved path.
      if let Insn::PackLink {
        pack,
        resolution: zo_sir::LinkResolution::Resolved(sym),
        ..
      } = insn
      {
        let path = self.interner.get(*sym).to_owned();

        // Fan the dylib path out to every ancestor pack
        // of `pack`. A `#link` in `provider/raylib/rcore.zo`
        // emits `Insn::PackLink { pack: raylib::rcore }`,
        // but sibling files (`rshape.zo`, `rtext.zo`)
        // have `Insn::FunDef.owning_pack = raylib::rshape`
        // etc. Without this fan-out, sibling FFIs would
        // miss the link and dyld would fail at runtime
        // (`Symbol not found: _DrawCircle`). Mirrors the
        // old folder-as-pack semantics where every file
        // under `provider/raylib/` shared one identity —
        // now expressed as a parent-walk over the
        // compound pack symbol.
        let pack_str = self.interner.get(*pack);
        let mut current: &str = pack_str;

        loop {
          if let Some(ancestor_sym) = self.interner.symbol(current) {
            pack_dylib.insert(ancestor_sym, path.clone());
          }

          match current.rsplit_once("::") {
            Some((parent, _)) => current = parent,
            None => break,
          }
        }
      }
    }

    // Single-walk pre-pass: collect FFI signatures + bind
    // each `pub ffi` to its declaring pack's `#link` dylib
    // path. Reads `owning_pack` from the FunDef itself
    // (set by the executor at emit time) instead of
    // tracking the most recent `PackDecl` positionally —
    // the positional model misattributed top-level user
    // FFIs to whichever preload pack landed last in the
    // merged SIR (`misato` / `sqlite`), routing user
    // symbols to the wrong dylib.
    //
    // `ffi_sigs` feeds the generic AAPCS dispatch fallback
    // (`_` arm of `Insn::Call`). `extern_dylib_paths`
    // feeds the linker's `LC_LOAD_DYLIB` routing.
    for insn in insns.iter() {
      if let Insn::FunDef {
        name,
        kind: FunctionKind::Intrinsic,
        params,
        return_ty,
        link_name,
        owning_pack,
        ..
      } = insn
      {
        self.ffi_sigs.insert(
          *name,
          FfiSig {
            params: params.iter().map(|(_, ty)| *ty).collect(),
            return_ty: *return_ty,
          },
        );

        if let Some(ln) = link_name {
          self.ffi_link_names.insert(*name, *ln);
        }

        // Walk up the FFI's pack ancestors to find a
        // matching `#link` entry. With the link's
        // fan-out (above), a `#link` declared in
        // `raylib::rcore` makes pack_dylib hold entries
        // at `raylib::rcore` AND `raylib`. Sibling FFIs
        // in `raylib::rshape` direct-lookup misses, but
        // walking up to `raylib` (the common parent)
        // hits. The combined fan-out + walk-up meet at
        // the closest ancestor under which the link
        // was declared.
        if let Some(pk) = owning_pack {
          let pk_str = self.interner.get(*pk);
          let mut current: &str = pk_str;

          loop {
            let resolved = self
              .interner
              .symbol(current)
              .and_then(|s| pack_dylib.get(&s));

            if let Some(path) = resolved {
              let c_sym = c_sym_for(self.interner, *name, *link_name);

              self.extern_dylib_paths.insert(c_sym, path.clone());

              break;
            }

            match current.rsplit_once("::") {
              Some((parent, _)) => current = parent,
              None => break,
            }
          }
        }
      }
    }

    // Pre-pass: collect `ArrayTyDef` and `MapTyDef`
    // metadata so the typed-write dispatchers can look
    // up element / scalar info regardless of where the
    // definitions land in the stream.
    for insn in insns.iter() {
      match insn {
        Insn::ArrayTyDef {
          array_ty,
          elem_ty,
          size,
        } => {
          self.array_metas.insert(
            array_ty.0,
            ArrayMeta {
              elem_ty: *elem_ty,
              size: *size,
            },
          );
        }
        Insn::MapTyDef {
          map_ty,
          key_fmt,
          val_fmt,
        } => {
          self.map_metas.insert(map_ty.0, (*key_fmt, *val_fmt));
        }
        Insn::VecTyDef { vec_ty, elem_fmt } => {
          self.vec_metas.insert(vec_ty.0, *elem_fmt);
        }
        Insn::SetTyDef { set_ty, key_fmt } => {
          self.set_metas.insert(set_ty.0, *key_fmt);
        }
        _ => {}
      }
    }

    // Pre-pass: assign each `Template`-bound reactive
    // symbol a slot id, and capture its first-store ty_id
    // as the `is_str` tag — without it `refresh_bindings`
    // would route string slots through `STATE` (i64) and
    // render the decimal of the buffer pointer.
    self.reactive_slots.clear();
    self.template_text_bindings.clear();
    self.reactive_arr_slots.clear();
    self.template_list_bindings.clear();
    self.template_attr_bindings.clear();

    let mut sym_first_store_ty: HashMap<Symbol, TyId> = HashMap::default();

    for insn in insns.iter() {
      if let Insn::Store { name, ty_id, .. } = insn {
        sym_first_store_ty.entry(*name).or_insert(*ty_id);
      }
    }

    for insn in insns.iter() {
      let Insn::Template {
        id,
        commands,
        bindings,
        ..
      } = insn
      else {
        continue;
      };

      let mut entries: Vec<(u32, u32, bool)> =
        Vec::with_capacity(bindings.text.len());

      for &(cmd_idx, sym) in &bindings.text {
        let slot = self.reactive_slot_for(sym);
        let is_str = sym_first_store_ty
          .get(&sym)
          .is_some_and(|t| t.0 == STR_TYPE_ID);

        entries.push((cmd_idx as u32, slot, is_str));
      }

      self.template_text_bindings.insert(*id, entries);

      // Attribute-bound `mut`s (e.g. `value={input_val}`) and
      // computed-binding captures must survive across events, so
      // they get reactive slots too — their writes then route
      // through `zo_state_set*` instead of an event-local frame.
      // Each attr binding is also recorded so the runtime can
      // re-apply it (`AttrBindingAbi`); `attr_idx` is the
      // attribute's position within its element's `attrs`.
      let mut attr_entries: Vec<(u32, u32, u32, bool)> = Vec::new();

      for (cmd_idx, attr) in &bindings.attrs {
        let Attr::Dynamic { var, .. } = attr else {
          continue;
        };

        let slot = self.reactive_slot_for(Symbol(*var));
        let attr_idx = commands.get(*cmd_idx).and_then(|cmd| match cmd {
          UiCommand::Element { attrs, .. } => attrs.iter().position(
            |a| matches!(a, Attr::Dynamic { var: v, .. } if v == var),
          ),
          _ => None,
        });

        if let Some(attr_idx) = attr_idx {
          let is_str = sym_first_store_ty
            .get(&Symbol(*var))
            .is_some_and(|t| t.0 == STR_TYPE_ID);

          attr_entries.push((*cmd_idx as u32, attr_idx as u32, slot, is_str));
        }
      }

      if !attr_entries.is_empty() {
        self.template_attr_bindings.insert(*id, attr_entries);
      }

      for (_cmd_idx, computed) in &bindings.computed {
        for &sym in &computed.captures {
          self.reactive_slot_for(sym);
        }
      }

      // List bindings: the `items_var` is a reactive `[]str`
      // array (its slot lives in `ARR_STATE`). Embed the per-item
      // recipe as a postcard blob and record the binding so
      // `emit_render_call` can emit its `ListBindingAbi` entry.
      let mut list_entries = Vec::with_capacity(bindings.list.len());

      for (cmd_idx, list) in &bindings.list {
        let items_slot = self.reactive_slot_for(list.items_var);

        self.reactive_arr_slots.insert(list.items_var);

        let recipe = convert_list_recipe(&list.item_template);
        let recipe_bytes = codec::encode(&recipe).unwrap_or_default();
        let recipe_len = recipe_bytes.len() as u32;
        let recipe_sym = Symbol(RECIPE_BLOB_SYM_BASE + self.next_recipe_blob);

        self.next_recipe_blob += 1;
        self.template_data.push((recipe_sym, recipe_bytes));

        list_entries.push(ListBindingEntry {
          cmd_idx: *cmd_idx as u32,
          items_slot,
          recipe_sym,
          recipe_len,
        });
      }

      if !list_entries.is_empty() {
        self.template_list_bindings.insert(*id, list_entries);
      }
    }

    // Pre-pass companion: find every function whose body
    // touches a reactive symbol. The inserted state-helper
    // `bl`s clobber X30, so those functions need a
    // non-leaf prologue/epilogue regardless of what the
    // allocator's `has_calls` said.
    self.fns_needing_calls.clear();
    {
      let mut current_fn: Option<InsnIdx> = None;

      for (i, insn) in insns.iter().enumerate() {
        match insn {
          Insn::FunDef { .. } => {
            current_fn = Some(InsnIdx(i as u32));
          }
          Insn::Load {
            src: LoadSource::Local(sym),
            ..
          } => {
            if self.reactive_slots.contains_key(sym)
              && let Some(start) = current_fn
            {
              self.fns_needing_calls.insert(start);
            }
          }
          Insn::Store { name, .. } => {
            if self.reactive_slots.contains_key(name)
              && let Some(start) = current_fn
            {
              self.fns_needing_calls.insert(start);
            }
          }
          // A reactive `arr.push` lowers to `bl _zo_state_arr_push`,
          // which clobbers X30 just like the scalar state helpers.
          Insn::ArrayPush {
            owner: Some(sym), ..
          } => {
            if self.reactive_arr_slots.contains(sym)
              && let Some(start) = current_fn
            {
              self.fns_needing_calls.insert(start);
            }
          }
          _ => {}
        }
      }
    }

    // Whole-SIR pre-pass for top-level / pre-FunDef code
    // paths that consult `value_def_idx`. The per-FunDef
    // arm in `translate_insn` rebuilds the map scoped to
    // each function — required because ValueId counters
    // reset per function and the flat map would alias.
    self.reset_value_def_idx();

    for (i, insn) in insns.iter().enumerate() {
      let idx = InsnIdx(i as u32);

      match insn {
        Insn::ConstInt { dst, .. }
        | Insn::ConstFloat { dst, .. }
        | Insn::ConstBool { dst, .. }
        | Insn::Load { dst, .. } => {
          self.value_def_insert(*dst, idx);
        }
        _ => {}
      }
    }

    for (idx, insn) in insns.iter().enumerate() {
      self.current_emit_idx = idx;
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
      // Order matters: dispatchers reference user-emitted
      // handler functions via `function_addr_fixups`, so
      // they must run AFTER the FunDef pass — which
      // happened above in `translate_insn` — but BEFORE
      // we hand off `self.functions` to the layout pass.
      self.generate_template_dispatchers();
      self.generate_ui_entry_point();
    }

    // Drop `LC_LOAD_DYLIB` entries for FFI symbols that
    // were declared (via `pub ffi` in a preload pack) but
    // never actually called. Each unused entry costs dyld
    // a full image mapping + init at process startup —
    // ~20-25ms on macOS for a 60+ MB dylib. Programs that
    // import a pack purely for its types (without calling
    // its FFI surface) get the startup back.
    //
    // `extern_used_set` is the ground truth: a symbol is
    // there iff `emit_extern_call` (or the FFI dispatch
    // arm) actually emitted a `bl` to it.
    self
      .extern_dylib_paths
      .retain(|sym, _| self.extern_used_set.contains(sym));

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

    // Build the vtable blobs now that every FunDef has
    // been laid out. `vtable_fixups` recorded here are
    // resolved a few lines down once `vtable_blob_starts`
    // knows each blob's final position in `code`.
    {
      let defs = std::mem::take(&mut self.abstract_defs);
      let impls = std::mem::take(&mut self.abstract_impls);

      self.emit_vtables(&defs, &impls);

      self.abstract_defs = defs;
      self.abstract_impls = impls;
    }

    let mut string_offsets = HashMap::default();
    let mut template_offsets = HashMap::default();
    let mut vtable_offsets: HashMap<Symbol, usize> = HashMap::default();
    let mut current_offset = code.len();

    for (symbol, bytes) in &self.string_data {
      string_offsets.insert(*symbol, current_offset);

      current_offset += bytes.len();
    }

    for (symbol, bytes) in &self.template_data {
      template_offsets.insert(*symbol, current_offset);

      current_offset += bytes.len();
    }

    // Vtables ride the TEXT-trailing pattern. Each blob
    // must be 8-byte aligned — every slot is read via
    // `LDR Xn, [base, #imm]` which traps on unaligned
    // effective addresses.
    if !self.vtable_data.is_empty() {
      let misalign = current_offset & 7;
      if misalign != 0 {
        current_offset += 8 - misalign;
      }
    }

    for (symbol, bytes) in &self.vtable_data {
      vtable_offsets.insert(*symbol, current_offset);

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

    // Vtable address loaders — `Insn::CoerceToDyn` emits
    // an ADR placeholder; resolve to the vtable's blob
    // offset within the TEXT trailing region.
    for (fixup_pos, vt_sym) in &self.vtable_addr_fixups {
      if let Some(&target_offset) = vtable_offsets.get(vt_sym) {
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

    for (_symbol, bytes) in &self.string_data {
      code.extend_from_slice(bytes);
    }

    for (_symbol, bytes) in &self.template_data {
      code.extend_from_slice(bytes);
    }

    // Mirrors the alignment pad above on `current_offset`
    // so blob positions match `vtable_offsets`.
    if !self.vtable_data.is_empty() {
      let misalign = code.len() & 7;
      if misalign != 0 {
        code.resize(code.len() + (8 - misalign), 0);
      }
    }

    let mut vtable_blob_starts: HashMap<Symbol, usize> = HashMap::default();

    for (symbol, bytes) in &self.vtable_data {
      vtable_blob_starts.insert(*symbol, code.len());
      code.extend_from_slice(bytes);
    }

    // One-shot bare-name → TEXT offset index. Vtable
    // fixups carry method symbols with no owning_pack
    // (apply-method names are pre-mangled as
    // `<Type>::<method>`, so the bare name is already
    // unique). Building this map once turns the per-
    // fixup lookup from O(N) scan into O(1).
    let fun_offset_by_name: HashMap<Symbol, u32> = self
      .functions
      .iter()
      .map(|(&(name, _), &off)| (name, off))
      .collect();

    for fixup in &self.vtable_fixups {
      let blob_start = match vtable_blob_starts.get(&fixup.vtable_sym) {
        Some(&s) => s,
        None => continue,
      };

      let fun_offset = match fun_offset_by_name.get(&fixup.method_key.0) {
        Some(&o) => o as u64,
        None => continue,
      };

      // Slot stores `method_addr − vtable_addr` so the
      // value is invariant under ASLR (both ends slide
      // by the same load bias). `DynDispatch` adds it
      // back to `vtable_addr` at runtime.
      let relative_offset = fun_offset as i64 - blob_start as i64;
      let absolute_slot = blob_start + fixup.slot_offset as usize;

      if absolute_slot + 8 <= code.len() {
        code[absolute_slot..absolute_slot + 8]
          .copy_from_slice(&relative_offset.to_le_bytes());
      }
    }

    Artifact { code }
  }

  /// Hand off codegen state to the linker phase.
  ///
  /// Consumes `self` and the freshly produced `artifact`,
  /// resolves the `main` and `_zo_ui_entry_point` offsets
  /// Emits one vtable blob per `(Abstract, ConcreteType)`
  /// pair in `abstract_impls`. Blob layout:
  /// `[size_of_data : u64][method_0_ptr : u64]..[method_N-1_ptr : u64]`.
  /// Slot 0 is `8` (the inline pointer width every
  /// concrete value carries through the AAPCS); slots
  /// 1..N are zero-initialised placeholders that the
  /// linker fills with `method_addr − vtable_addr`
  /// offsets via `vtable_fixups`. Method order tracks
  /// `impl_entry.methods`, keeping the per-slot index
  /// aligned with `Insn::DynDispatch.method_index`.
  ///
  /// Vtable bytes ride the TEXT-trailing pattern (same
  /// as string / template data); code-side ADR loaders
  /// resolve through `vtable_addr_fixups` at link time.
  pub fn emit_vtables(
    &mut self,
    abstract_defs: &HashMap<Symbol, AbstractDef>,
    abstract_impls: &HashMap<(Symbol, Symbol), AbstractImpl>,
  ) {
    for ((abs_sym, _concrete_sym), impl_entry) in abstract_impls {
      let Some(def) = abstract_defs.get(abs_sym) else {
        continue;
      };
      let n = def.methods.len();
      let blob_len = 8 * (1 + n);
      let mut blob = Vec::with_capacity(blob_len);

      // Slot 0: inline pointer width — every concrete
      // value passed through AAPCS lives as one 8-byte
      // handle (struct heap pointer, primitive value,
      // enum tag). A real `flat_struct_byte_width`
      // helper can replace this when struct layout
      // tracking lands.
      blob.extend_from_slice(&8u64.to_le_bytes());
      blob.resize(blob_len, 0);

      // Slots 1..=N: per-method fixups. `impl_entry
      // .methods[i]` matches the abstract's `methods[i]`
      // by construction (the apply-block parser slices
      // `funs[funs_baseline..]` in declaration order),
      // so the index aligns with
      // `Insn::DynDispatch.method_index`.
      for (i, &method_sym) in impl_entry.methods.iter().take(n).enumerate() {
        self.vtable_fixups.push(VtableSlotFixup {
          vtable_sym: impl_entry.vtable_sym,
          slot_offset: 8 * (1 + i) as u32,
          method_key: (method_sym, None),
        });
      }

      self.vtable_data.push((impl_entry.vtable_sym, blob));
    }
  }

  /// (so the linker doesn't need an interner handle), and
  /// bundles every fixup / symbol table the mach-o
  /// assembler needs into a `MachoLinkObject`. The
  /// resulting object is the only data that crosses the
  /// codegen → linker phase boundary.
  pub fn into_link_object(self, artifact: Artifact) -> MachoLinkObject {
    // Entry-point resolution is flexible on owning_pack:
    // `main` lives under whatever pack the main module
    // belongs to (often the implicit `main` pack derived
    // from the filename). Match by bare name and accept
    // any owning_pack so the linker finds the entry
    // regardless of module-pack identity.
    let main_offset = self.interner.symbol("main").and_then(|s| {
      self
        .functions
        .iter()
        .find(|((name, _), _)| *name == s)
        .map(|(_, off)| *off)
    });

    let ui_entry_offset = if self.has_templates {
      let entry_sym = Symbol(UI_ENTRY_SYMBOL);

      self
        .functions
        .iter()
        .find(|((name, _), _)| *name == entry_sym)
        .map(|(_, off)| *off)
    } else {
      None
    };

    MachoLinkObject {
      code: artifact.code,
      functions: self.functions,
      string_data: self.string_data,
      string_fixups: self.string_fixups,
      function_addr_fixups: self.function_addr_fixups,
      template_data: self.template_data,
      has_templates: self.has_templates,
      extern_used: self.extern_used,
      extern_stub_offsets: self.extern_stub_offsets,
      extern_fixups: self.extern_fixups,
      call_fixups: self.call_fixups,
      main_offset,
      ui_entry_offset,
      extern_dylib_paths: self.extern_dylib_paths,
    }
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
      Insn::ConstString { dst, ty_id, .. }
      | Insn::StringFormat { dst, ty_id, .. } => {
        self.value_types.insert(dst.0, *ty_id);
      }
      Insn::ToStr { dst, .. } => {
        self.value_types.insert(dst.0, TyId(STR_TYPE_ID));
      }
      _ => {}
    }

    match insn {
      Insn::FunDef {
        name,
        params,
        owning_pack,
        ..
      } => {
        let offset = self.emitter.current_offset();

        // Strict `(name, owning_pack)` keying — no
        // bare-name fallback slot. Every `Insn::Call.callee_pack`
        // and `Insn::TaskSpawn.callee_pack` is plumbed
        // from `value::FunDef.owning_pack` at the
        // executor level, so the two sides agree by
        // construction.
        self.functions.insert((*name, *owning_pack), offset);
        self.enter_function(*name, idx, all_insns);

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
              info.string_format_scratch_size,
            )
          });

        if let Some((
          has_calls,
          spill_size,
          struct_size,
          mut_size,
          chan_scratch_size,
          select_scratch_size,
          string_format_scratch_size,
        )) = fn_info
        {
          let has_calls = self.promoted_has_calls(idx as u32, has_calls);

          if has_calls {
            self.emitter.emit_stp(X29, X30, SP, FP_LR_SAVE_OFFSET);
          }

          let param_reserve = params.len() as u32 * STACK_SLOT_SIZE;
          let caller_save = self.caller_save_reserve;
          let promo_save_size = self.promo_save_size();
          let frame = Self::aligned_frame_size(FrameAreas {
            spill_size,
            mut_size,
            param_reserve,
            caller_save,
            struct_size,
            chan_scratch_size,
            select_scratch_size,
            string_format_scratch_size,
            promo_save_size,
          });

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

          // Past every other area so realloc / heap-clone
          // saves can't alias struct slots.
          self.array_push_scratch_base =
            self.select_scratch_base + select_scratch_size;

          self.string_format_scratch_base =
            self.array_push_scratch_base + ARRAY_PUSH_SCRATCH_SIZE;

          // Promoted-register save area sits at the top of the
          // frame, above every scratch region. Save each
          // claimed callee-saved register so the caller's
          // value is restored on return — the ABI requires it.
          self.promo_save_base =
            self.string_format_scratch_base + string_format_scratch_size;

          for i in 0..self.promotion.used_count() {
            let reg = self.promotion.used_reg_at(i);
            let off = self.promo_save_base + i as u32 * STACK_SLOT_SIZE;

            self.emit_str_sp(reg, off);
          }

          let param_base = spill_size + mut_size;

          for (i, (sym, ty_id)) in params.iter().enumerate() {
            let off = param_base + i as u32 * STACK_SLOT_SIZE;
            let is_fp =
              ty_id.0 >= FLOAT_TYPE_ID_MIN && ty_id.0 <= FLOAT_TYPE_ID_MAX;

            // A promoted local that is also a parameter must
            // receive its incoming arg value: copy the arg
            // register into the promotion register before any
            // body code reads it. Without this the promotion
            // register holds the caller's saved x19.. value,
            // not the argument. Only GP scalars are promoted,
            // so `is_fp` params never hit this path.
            let promo = (!is_fp).then(|| self.promotion.reg_of(*sym)).flatten();

            if let Some(reg) = promo {
              self.param_promo_reg.insert(i as u32, reg);
            }

            if i < MAX_REG_ARGS {
              if is_fp {
                let src = FpRegister::new(i as u8);

                self.emit_str_fp_sp(src, off);
              } else {
                let src = Register::new(i as u8);

                self.emit_str_sp(src, off);

                if let Some(dst) = promo {
                  self.emitter.emit_mov_reg(dst, src);
                }
              }
            } else {
              // Overflow param: on the caller's stack, above
              // our working SP. The displacement is `frame`
              // plus the FP/LR frame record — but the record
              // is pushed ONLY by non-leaf functions, so a
              // leaf callee (no `has_calls`) reads at `frame`
              // alone. Adding it unconditionally read 16 bytes
              // past the real arg → uninitialized-stack garbage.
              let record = if has_calls { FRAME_RECORD_SIZE } else { 0 };
              let caller_off = frame
                + record
                + (i as u32 - MAX_REG_ARGS as u32) * STACK_SLOT_SIZE;

              self.emit_ldr_sp(X16, caller_off);
              self.emit_str_sp(X16, off);

              if let Some(dst) = promo {
                self.emitter.emit_mov_reg(dst, X16);
              }
            }

            self.param_slots.insert(i as u32, off);
            self.param_sym_slots.insert(sym.as_u32(), (off, is_fp));
          }

          // Once main's prologue has stored its params,
          // call `zo_state_init` so reactive `mut` writes
          // (the program's first user-visible action) see
          // an allocated buffer. Idempotent on the runtime
          // side; emitting in every function is wasteful
          // but harmless — keep it main-only.
          if let Some(name) = self.current_function
            && self.interner.get(name) == "main"
          {
            self.emit_state_init_prologue();
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
        // FP regs are uniformly 64-bit internally; narrowing
        // happens only at FFI boundaries / explicit casts.
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
        self.string_data_seen.insert(*symbol);

        // String is a single pointer to the struct.
        let ptr_reg = self.reg_for_insn(idx).unwrap_or(X1);
        let fixup_pos = self.emitter.current_offset();

        self.string_fixups.push((fixup_pos, *symbol));
        self.emitter.emit_adr(ptr_reg, 0);
      }

      Insn::Load {
        dst, src, ty_id, ..
      } => match src {
        LoadSource::Local(sym) => {
          let slot = sym.as_u32();

          // Reactive `mut` read: route through the runtime
          // dylib's `zo_state_get(slot)` so closures and
          // main both pull from the process-global state
          // buffer instead of their respective stack
          // frames. Reactive ARRAY slots are excluded — their
          // elements live in `ARR_STATE`, not the scalar buffer,
          // so a bare array read stays a local load (the value
          // is only consumed by a reactive `push`, which routes
          // to the FFI on its own).
          if let Some(&state_slot) = self.reactive_slots.get(sym)
            && !self.reactive_arr_slots.contains(sym)
          {
            if let Some(dst_reg) = self.alloc_reg(*dst) {
              self.emit_state_load(dst_reg, state_slot, ty_id.0 == STR_TYPE_ID);
            }

            return;
          }

          // Register promotion: the local already lives in a
          // callee-saved register. Bind this load's `dst` to
          // that register so consumers read it directly — no
          // memory load, no `mov`. Promoted locals are GP
          // scalars, so none of the enum/tuple/array-block
          // metadata paths below apply.
          if let Some(reg) = self.promotion.reg_of(*sym) {
            self.promo_value_reg.insert(dst.0, reg);

            return;
          }

          // Recover the construction-site enum payload types
          // (if any) so a later `showln(local)` can dispatch
          // on the concrete payload type. Without this, only
          // values that came directly from an `EnumConstruct`
          // SSA dst would have the override.
          if let Some(meta) = self.local_enum_field_tys.get(&slot).cloned() {
            self.value_enum_field_tys.insert(dst.0, meta);
          }

          if let Some(elems) = self.local_tuple_elem_tys.get(&slot).cloned() {
            self.value_tuple_elem_tys.insert(dst.0, elems);
          }

          // `[N]T` inline-storage variables: the value IS
          // the block's address (computed from SP), not an
          // 8-byte pointer loaded from a slot.
          if let Some(&offset) = self.array_var_blocks.get(&slot) {
            if let Some(dst_reg) = self.alloc_reg(*dst) {
              self.emit_add_sp_offset(dst_reg, offset);
            }

            return;
          }

          if let Some(&offset) = self.mutable_slots.get(&slot) {
            if let Some(dst_reg) = self.alloc_reg(*dst) {
              self.emit_ldr_sp(dst_reg, offset);
            } else if let Some(fp_dst) = self
              .alloc_fp_reg(*dst)
              .or_else(|| self.fp_reg_for_insn(idx))
            {
              // Float local: LDR Dt, [SP, #offset].
              self.emit_ldr_fp_sp(fp_dst, offset);
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
                self.emit_ldr_fp_sp(fp_dst, offset);
              }
            } else if let Some(dst_reg) = self.alloc_reg(*dst) {
              self.emit_ldr_sp(dst_reg, offset);
            }
          }
        }
        LoadSource::Param(idx) => {
          // Promoted `mut` parameter: its live value is in the
          // callee-saved register the `Store` writes, not the
          // home slot. Bind this read to that register so the
          // loop observes every update.
          if let Some(&reg) = self.param_promo_reg.get(idx) {
            self.promo_value_reg.insert(dst.0, reg);

            return;
          }

          // Load from parameter spill slot (saved in
          // prologue). This is safe even after registers
          // have been reused for other values.
          if let Some(&off) = self.param_slots.get(idx) {
            if let Some(fp_dst) = self.alloc_fp_reg(*dst) {
              // Float param: load from FP spill slot.
              self.emit_ldr_fp_sp(fp_dst, off);
            } else if let Some(dst_reg) = self.alloc_reg(*dst) {
              // GP param: load from GP spill slot.
              self.emit_ldr_sp(dst_reg, off);
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

      Insn::UnOp {
        dst,
        op,
        rhs,
        ty_id,
      } => {
        let is_flt =
          ty_id.0 >= FLOAT_TYPE_ID_MIN && ty_id.0 <= FLOAT_TYPE_ID_MAX;

        match op {
          // Floats live in the FP file — a GP `SUB` would
          // negate an unrelated X register and leave the value
          // unchanged. Use `FNEG`.
          UnOp::Neg if is_flt => {
            let fp_d = self.alloc_fp_reg(*dst).unwrap_or(D0);
            let fp_r = self.alloc_fp_reg(*rhs).unwrap_or(D0);

            self.emitter.emit_fneg(fp_d, fp_r);
          }
          UnOp::Neg => {
            let d = self.alloc_reg(*dst).unwrap_or(X0);
            let r = self.alloc_reg(*rhs).unwrap_or(X0);

            self.emitter.emit_sub(d, XZR, r);
          }
          UnOp::Not => {
            let d = self.alloc_reg(*dst).unwrap_or(X0);
            let r = self.alloc_reg(*rhs).unwrap_or(X0);

            // !b => b ^ 1 (boolean not).
            self.emitter.emit_mov_imm(X16, 1);
            self.emitter.emit_eor(d, r, X16);
          }
          _ => {}
        }
      }

      Insn::Call {
        dst: call_dst,
        name,
        callee_pack,
        args,
        ty_id: call_ret_ty,
        ..
      } => {
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

          // raylib + misato FFIs flow through the generic
          // AAPCS fallback (`_` arm → `emit_ffi_call`).
          // Misato C symbols already match `_<zo_name>` —
          // the std `pub ffi zo_misato_*` declarations
          // name the C symbol directly, so no mapping is
          // needed.
          "exists" => self.emit_io_exists(args, idx),
          "read_file" => self.emit_io_read_file(args, idx),
          "write_file" => self.emit_io_write_file(args, idx),
          "append_file" => self.emit_io_append_file(args, idx),
          "readln" => self.emit_io_read_stdin(idx, "_zo_io_readln"),
          "read" => self.emit_io_read_stdin(idx, "_zo_io_read"),
          "args" => self.emit_io_args(idx),
          "remove_file" => self.emit_io_remove(args, idx),
          "read_dir" => self.emit_io_read_dir(args, idx),

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
          // TODO: it must be implement in core.
          "Vec::new" => self.emit_vec_new(args, idx),
          "Vec::push" => {
            let elem_ty = args
              .get(1)
              .and_then(|v| self.type_of(*v))
              .unwrap_or(TyId(0));

            self.emit_vec_push(args, idx, elem_ty);
          }
          "Vec::pop" => {
            let elem_ty = self.vec_read_elem_ty(*call_dst, *call_ret_ty);

            self.emit_vec_pop(args, idx, elem_ty);
          }
          "Vec::get" => {
            let elem_ty = self.vec_read_elem_ty(*call_dst, *call_ret_ty);

            self.emit_vec_get(args, idx, elem_ty);
          }
          "Vec::set" => {
            let elem_ty = args
              .get(2)
              .and_then(|v| self.type_of(*v))
              .unwrap_or(TyId(0));

            self.emit_vec_set(args, idx, elem_ty);
          }
          "Vec::remove" => {
            let elem_ty = self.vec_read_elem_ty(*call_dst, *call_ret_ty);

            self.emit_vec_remove(args, idx, elem_ty);
          }

          // HashSet apply-method dispatch. Reuses the
          // `_zo_map_*` runtime allocator with `val_sz=0`.
          "HashSet::new" => self.emit_set_new(args, idx),
          "HashSet::insert" => self.emit_set_insert(args, idx),
          "HashSet::contains" => self.emit_set_contains(args, idx),
          "HashSet::remove" => self.emit_set_remove(args, idx),

          // `[]int::sort` — in-place ascending sort via the
          // runtime. The compiler can't lower
          // `self[i] = ...` inside an apply method body
          // today, so the loop runs in Rust.
          "arr_int::sort" => self.emit_arr_sort_int(args, idx),

          // Non-marshaling raw FFIs. The argument is the
          // already-loaded `*mut ZoMap` / `*mut ZoVec`
          // (from `self.ptr`); pass through to the runtime
          // export with no byte marshaling.
          "zo_map_len_raw" => self.emit_map_len_raw(args, idx),
          "zo_map_free_raw" => self.emit_map_free_raw(args, idx),
          "zo_vec_len_raw" => self.emit_vec_len_raw(args, idx),
          "zo_vec_free_raw" => self.emit_vec_free_raw(args, idx),
          "zo_set_len_raw" => self.emit_set_len_raw(args, idx),
          "zo_set_free_raw" => self.emit_set_free_raw(args, idx),

          // `str.replace(needle, with)` — `apply str` body
          // forwards `(self, needle, with)` to this raw FFI;
          // codegen forwards X0..X2 to `_zo_str_replace`.
          "zo_str_replace" => self.emit_str_replace_raw(args, idx),

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
            let fn_name = self.interner.get(*name);
            let c_sym = libm_c_symbol(fn_name);
            let nargs = libm_arg_count(fn_name);

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

              self.emit_str_sp(reg, off);
            }

            // Emit BL placeholder (offset 0). Will be
            // patched in assemble() to target the stub.
            let fixup_pos = self.emitter.current_offset();

            self.emitter.emit_bl(0);
            self.extern_fixups.push((fixup_pos, c_sym.clone()));

            // Track used libm functions (no duplicates).
            if self.extern_used_set.insert(c_sym.clone()) {
              self.extern_used.push(c_sym);
            }

            // Restore caller-saved regs after BL.
            for i in 0..CALLER_SAVE_COUNT {
              let reg = Register::new(CALLER_SAVE_START + i as u8);
              let off = base + i as u32 * STACK_SLOT_SIZE;

              self.emit_ldr_sp(reg, off);
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
            // Generic AAPCS FFI fallback. If `name`
            // resolves to a `FunctionKind::Intrinsic`
            // FunDef AND the orchestrator wired a
            // `type_view`, classify the signature once
            // and emit through `emit_ffi_call`. This is
            // the path that lets a future `pub ffi` work
            // with zero compiler edits — F4/F5 delete
            // the per-symbol arms above and the calls
            // fall through to here.
            if let Some(view) = self.type_view
              && let Some(sig) = self.ffi_sigs.get(name)
            {
              let abi = crate::abi::classify(
                &sig.params,
                sig.return_ty,
                &crate::abi::TypeQuery {
                  tys: view.tys,
                  ty_table: view.ty_table,
                },
              );
              // Resolve the C symbol. `link_name` from a
              // `%% link_name = "X".` attribute wins;
              // otherwise the legacy `RAYLIB_NAME_MAP`
              // (snake → PascalCase) covers raylib until
              // its bindings migrate; everything else
              // gets the platform-underscore default.
              let link_name = self.ffi_link_names.get(name).copied();
              let c_sym: &'static str =
                c_sym_for(self.interner, *name, link_name).leak();

              self.emit_ffi_call(c_sym, &abi, args, idx, all_insns);
              return;
            }

            // User-function call (zo's own positional
            // calling convention — not full AAPCS).
            // Marshal args into X0-X7 / D0-D7 and the
            // overflow stack region; returns the aligned
            // byte count SP was lowered by (0 when ≤8 args).
            let stack_arg_bytes = self.marshal_user_call_args(args);

            // BL to user-defined function. Strict
            // `(name, callee_pack)` lookup — no fallback.
            // Every emit site stamps `callee_pack` from
            // `callee_pack_of(name)` so the key matches
            // the FunDef's `(name, owning_pack)` insert
            // by construction.
            let key = (*name, *callee_pack);

            if let Some(&func_offset) = self.functions.get(&key) {
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
              self.call_fixups.push((fixup_pos, key));
            }

            // Restore SP so SP-relative offsets after the call
            // match the frame the rest of the body expects.
            if stack_arg_bytes > 0 {
              self.emitter.emit_add_imm(SP, SP, stack_arg_bytes as u16);
            }

            // If callee returns a struct, x0 holds a
            // dangling pointer into the callee's frame.
            // Copy the struct fields into the caller's
            // own struct area before x0 becomes stale.
            // The recorded slot count is the *deep* one
            // (regalloc + this codegen agree via
            // `flat_struct_slots_of`), so nested-struct
            // fields are flattened into adjacent caller
            // slots — without this, the field slot held a
            // pointer back into the freed callee frame.
            if let Some(&deep_slots) = self.struct_return_fns.get(name) {
              let dst_base = self.struct_base + self.next_struct_slot;

              // Walk the call's return type so we know
              // which fields are themselves structs (those
              // need the recursive copy). With no
              // `type_view` we fall back to the flat per-
              // word copy — same semantics as the pre-fix
              // code, safe whenever the program doesn't
              // return nested-struct shapes.
              let ret_ty = *call_ret_ty;

              // Enum returns need a different deep-copy
              // shape: outer slots are shallow-copied,
              // then per-variant logic deep-copies any
              // struct payloads and rewrites the in-
              // payload pointer to the caller's copy.
              // See `emit_enum_deep_copy_after_call`.
              let enum_layout: Option<EnumDeepCopyLayout> =
                self.type_view.and_then(|view| {
                  let Ty::Enum(eid) = resolve_ty(view.tys, ret_ty) else {
                    return None;
                  };
                  let e = view.ty_table.enum_ty(eid)?;
                  // Per-variant substituted struct payload
                  // fields from the regalloc's pre-pass.
                  // The enum's own variant fields are still
                  // unsubstituted generic placeholders, so
                  // we splice the regalloc-recorded struct
                  // types into the matching variant field
                  // slots. The regalloc also propagates
                  // callee entries into caller slots the
                  // caller didn't construct locally — see
                  // `build_struct_return_map`'s callee
                  // fixpoint — so a plain per-fn lookup
                  // covers passthrough functions whose
                  // match arms route through a callee.
                  let payload_overrides =
                    self.enum_payload_struct_fields.get(name);
                  let variants: Vec<EnumVariantInfo> = view
                    .ty_table
                    .enum_variants(e)
                    .iter()
                    .map(|v| {
                      let mut field_tys: Vec<TyId> =
                        view.ty_table.variant_fields(v).to_vec();

                      if let Some(over) = payload_overrides
                        && let Some(variant_over) =
                          over.get(v.discriminant as usize)
                      {
                        for (idx, sty) in variant_over {
                          let i = *idx as usize;
                          if i < field_tys.len() {
                            field_tys[i] = *sty;
                          }
                        }
                      }

                      EnumVariantInfo {
                        discriminant: v.discriminant,
                        field_tys,
                      }
                    })
                    .collect();
                  Some(EnumDeepCopyLayout { variants })
                });

              if let Some(layout) = enum_layout {
                self.emit_enum_deep_copy_after_call(&layout, dst_base);
              } else {
                let outer_field_tys: Option<Vec<TyId>> =
                  self.type_view.and_then(|view| {
                    let Ty::Struct(sid) = resolve_ty(view.tys, ret_ty) else {
                      return None;
                    };
                    let st = view.ty_table.struct_ty(sid)?;
                    Some(
                      view
                        .ty_table
                        .struct_fields(st)
                        .iter()
                        .map(|f| f.ty_id)
                        .collect(),
                    )
                  });

                match outer_field_tys {
                  Some(field_tys) => {
                    let outer_count = field_tys.len() as u32;
                    // Bump cursor past the outer slots
                    // so nested copies can use the
                    // trailing slots within our
                    // reserved budget.
                    let mut inner_cursor =
                      dst_base + outer_count * STACK_SLOT_SIZE;

                    for (i, field_ty) in field_tys.iter().enumerate() {
                      let src_off = (i as u32 * STACK_SLOT_SIZE) as i16;
                      let dst_off = dst_base + i as u32 * STACK_SLOT_SIZE;

                      if self.is_struct_ty(*field_ty) {
                        inner_cursor = self.emit_deep_copy_struct_field(
                          X0,
                          src_off,
                          dst_off,
                          *field_ty,
                          inner_cursor,
                        );
                      } else {
                        self.emitter.emit_ldr(X16, X0, src_off);
                        self.emit_str_sp(X16, dst_off);
                      }
                    }
                  }
                  None => {
                    // Flat fallback — caller didn't
                    // supply a type view, so we can't
                    // tell which fields are structs.
                    // `deep_slots` then equals the flat
                    // field count (see
                    // `build_struct_return_map`'s
                    // `None` branch), so this still
                    // matches the budget.
                    for i in 0..deep_slots {
                      let src_off = (i * STACK_SLOT_SIZE) as i16;
                      let dst_off = dst_base + i * STACK_SLOT_SIZE;

                      self.emitter.emit_ldr(X16, X0, src_off);
                      self.emit_str_sp(X16, dst_off);
                    }
                  }
                }
              }

              // Point result at the caller's copy.
              if let Some(result_reg) = self.reg_for_insn(idx) {
                self.emit_add_sp_offset(result_reg, dst_base);
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
              self.emit_add_sp_offset(X0, dst_base);
              self.composite_value_slots.insert(*call_dst, dst_base);

              self.next_struct_slot += deep_slots * STACK_SLOT_SIZE;
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

      Insn::CallIndirect { callee, args, .. } => {
        // Marshal args into the positional ABI slots exactly
        // like a direct call. The callee pointer is loaded
        // AFTER this so materializing it can't clobber an arg
        // register (x0..x7) — and X16/X17 (the move scratch)
        // are intra-procedure scratch, never arg/return regs.
        let stack_arg_bytes = self.marshal_user_call_args(args);

        // Bring the 64-bit code pointer into X16. Prefer
        // re-emitting the producing load from its stable slot
        // (the same path const/local args use); fall back to
        // the allocator's register when the callee is a
        // computed value the materializer can't replay.
        //
        // `marshal_user_call_args` already lowered SP by
        // `stack_arg_bytes` for the overflow args, so any
        // SP-relative callee load must add that bias to reach
        // the real frame slot. The fallback `alloc_reg` path is
        // bias-free: it's a pure register lookup (no memory
        // load emitted here) — the allocator's spill reload for
        // a value live across this call was emitted by
        // `emit_spills(Before)` while SP was still at rest.
        if !self.materialize_value_into_x16(*callee, all_insns, stack_arg_bytes)
          && let Some(reg) = self.alloc_reg(*callee)
          && reg != X16
        {
          self.emitter.emit_mov_reg(X16, reg);
        }

        // Branch through the pointer. AAPCS64 reserves
        // x16/x17 for exactly this intra-procedure indirect
        // branch, so the callee can clobber them freely.
        self.emitter.emit_blr(X16);

        // Restore SP so SP-relative offsets after the call
        // match the frame the rest of the body expects.
        if stack_arg_bytes > 0 {
          self.emitter.emit_add_imm(SP, SP, stack_arg_bytes as u16);
        }

        // Result lands in X0 (GP) or D0 (FP) per AAPCS — move
        // it to dst's allocated register if different. A
        // `Fn` value never returns an aggregate, so the
        // struct deep-copy path the direct call needs does
        // not apply here.
        if let Some(fp_result) = self.fp_reg_for_insn(idx) {
          if fp_result != D0 {
            self.emitter.emit_fmov_fp(fp_result, D0);
          }
        } else if let Some(result_reg) = self.reg_for_insn(idx)
          && result_reg != X0
        {
          self.emitter.emit_mov_reg(result_reg, X0);
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
          } else if let Some(src_reg) = self.alloc_reg(*vid) {
            if src_reg != X0 {
              self.emitter.emit_mov_reg(X0, src_reg);
            }
          } else if let Some(&offset) = self.composite_value_slots.get(vid) {
            // Composite value (struct/enum) with no
            // register — its bytes still live at
            // `SP + offset` on this frame. Materialize
            // the pointer so the caller's deep-copy can
            // read it through `X0`. Without this, the
            // callee returned with stale X0 and the
            // caller bound garbage.
            self.emit_add_sp_offset(X0, offset);
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
                info.string_format_scratch_size,
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
          string_format_scratch_size,
        )) = epi_info
        {
          // Mirror the prologue's promotion so the
          // epilogue restores FP/LR + tears down the
          // matching frame size.
          let has_calls = self
            .current_fn_start
            .map(|s| self.promoted_has_calls(s as u32, has_calls))
            .unwrap_or(has_calls);
          let param_reserve = self.param_slots.len() as u32 * STACK_SLOT_SIZE;
          let caller_save = self.caller_save_reserve;
          let promo_save_size = self.promo_save_size();
          let frame = Self::aligned_frame_size(FrameAreas {
            spill_size,
            mut_size,
            param_reserve,
            caller_save,
            struct_size,
            chan_scratch_size,
            select_scratch_size,
            string_format_scratch_size,
            promo_save_size,
          });

          // Restore the caller's callee-saved registers from
          // the top-of-frame save area before tearing the
          // frame down — `promo_save_base` is still valid (the
          // SP hasn't moved since the prologue saved them).
          for index in 0..self.promotion.used_count() {
            let reg = self.promotion.used_reg_at(index);
            let off = self.promo_save_base + index as u32 * STACK_SLOT_SIZE;

            self.emit_ldr_sp(reg, off);
          }

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

      Insn::Store { name, value, ty_id } => {
        // Forward concrete enum payload types from the rhs
        // SSA value to this local so a later `Load` of `name`
        // can recover them. Mirrors how `value_types` flows
        // through stores for enum pretty-printing.
        if let Some(meta) = self.value_enum_field_tys.get(&value.0).cloned() {
          self.local_enum_field_tys.insert(name.as_u32(), meta);
        }

        if let Some(elems) = self.value_tuple_elem_tys.get(&value.0).cloned() {
          self.local_tuple_elem_tys.insert(name.as_u32(), elems);
        }

        // Reactive `mut` write: route through the runtime
        // dylib's `zo_state_set(slot, value)` so closure
        // and main-thread updates both land in the
        // process-global state buffer that
        // `refresh_bindings` reads. Reactive ARRAY slots are
        // excluded — `arr.push` already routes to `ARR_STATE`
        // via the FFI, and the `mut todos = []` initialiser's
        // heap-pointer store stays a harmless local.
        if let Some(&slot) = self.reactive_slots.get(name)
          && !self.reactive_arr_slots.contains(name)
        {
          let value_reg = match self.alloc_reg(*value) {
            Some(r) => r,
            None => return,
          };

          self.emit_state_store(slot, value_reg, ty_id.0 == STR_TYPE_ID);

          return;
        }

        // Register promotion: the local lives in a dedicated
        // callee-saved register, so the write is a register
        // move with no memory store and no slot minting. The
        // value is already materialized in a GP register by
        // its producing instruction (we only promote GP
        // scalars). A `mov dst, dst` no-op is skipped.
        if let Some(dst) = self.promotion.reg_of(*name) {
          if let Some(src) = self.alloc_reg(*value) {
            if src != dst {
              self.emitter.emit_mov_reg(dst, src);
            }
          } else if self.materialize_value_into_x16(*value, all_insns, 0) {
            self.emitter.emit_mov_reg(dst, X16);
          }

          return;
        }

        let slot_key = name.as_u32();

        // `[N]T` value-semantics path: the variable owns its
        // own `[len:8][cap:8][e0:8]...[eN:8]` block in the
        // frame; assignment memcopies from the source's
        // block. Without this, a `mut row: [N]T = lit;
        // mut next: [N]T = lit2; row = next;` made `row`
        // alias `next`'s literal block — writes through one
        // were observed through the other.
        if let Some(n) = self.array_metas.get(&ty_id.0).and_then(|m| m.size) {
          let block_off =
            if let Some(&off) = self.array_var_blocks.get(&slot_key) {
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

              self.array_var_blocks.insert(slot_key, off);
              // Reserve the block's full footprint so a later
              // scalar Store doesn't land inside it.
              self.next_mut_slot += 2 + n;

              off
            };

          let src_reg = match self.alloc_reg(*value) {
            Some(r) => r,
            None => return,
          };

          // Word-by-word memcpy. N is bounded by what the
          // type literal carried, so the unrolled loop stays
          // tight for typical sizes.
          for i in 0..(2 + n) {
            let src_word_off = (i * STACK_SLOT_SIZE) as i16;

            self.emitter.emit_ldr(X16, src_reg, src_word_off);
            self.emit_str_sp(X16, block_off + i * STACK_SLOT_SIZE);
          }

          return;
        }

        // Scalar (or `[]T` heap-pointer) path: STR the value
        // to a single 8-byte slot. Allocate on first Store,
        // reuse after.
        //
        // For `mut` parameters, alias the mutable slot to
        // the param's spill slot — Loads still come through
        // `LoadSource::Param(idx)` from that same offset,
        // so reads see the latest write. Without this,
        // `Store` would mint a fresh slot via `next_mut_slot`
        // and writes would never reach the location reads
        // come from, so a `while n > 1 { n = n / 2; }` loop
        // over a `mut n: int` arg would read the original
        // arg register every iteration and never terminate.
        let offset = if let Some(&off) = self.mutable_slots.get(&slot_key) {
          off
        } else if let Some(&(param_off, _)) =
          self.param_sym_slots.get(&slot_key)
        {
          self.mutable_slots.insert(slot_key, param_off);

          param_off
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
          self.emit_str_sp(src_reg, offset);
        } else if let Some(&slot) = self.composite_value_slots.get(value) {
          // Composite (struct/enum from a call): the
          // value lives on the stack, not in a register.
          // Materialize its pointer and store that.
          self.emit_add_sp_offset(X16, slot);
          self.emit_str_sp(X16, offset);
        } else if let Some(fp_src) = self
          .alloc_fp_reg(*value)
          .or_else(|| self.scan_fp_reg_back(idx))
        {
          // Float variable: STR Dt, [SP, #offset].
          self.emit_str_fp_sp(fp_src, offset);
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

        if zo_ui_protocol::is_render_directive(n) {
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

      Insn::ArrayLiteral {
        elements, ty_id, ..
      } => {
        // Two paths, picked from `Insn::ArrayTyDef.size`:
        //
        // - `[N]T` static (size = Some): stack-allocate the
        //   `[len:8][cap:8][e0:8]...[eN:8]` block in the
        //   function frame. Type checker has already
        //   coerced any literal flowing into a dynamic
        //   binding to `[]T` (see
        //   `finalize_pending_decl::rewrite_array_literal_ty`),
        //   so a static hit here genuinely means the value
        //   won't be `push`ed.
        //
        // - `[]T` dynamic (size = None): heap-allocate via
        //   `_malloc`. Pushable, growable via `_realloc`,
        //   freeable.
        let n = elements.len() as u32;

        if self
          .array_metas
          .get(&ty_id.0)
          .is_some_and(|m| m.size.is_some())
        {
          let base = self.struct_base + self.next_struct_slot;

          self.emitter.emit_mov_imm(X16, n as u16);
          self.emit_str_sp(X16, base);
          self.emit_str_sp(X16, base + STACK_SLOT_SIZE);

          for (i, elem) in elements.iter().enumerate() {
            let off =
              base + ARRAY_HEADER_SIZE as u32 + i as u32 * STACK_SLOT_SIZE;

            self.emit_array_element_store_sp(*elem, all_insns, off);
          }

          if let Some(dst) = self.reg_for_insn(idx) {
            self.emit_add_sp_offset(dst, base);
          }

          self.next_struct_slot += (2 + n) * STACK_SLOT_SIZE;

          return;
        }

        // Empty arrays over-provision (cap = 1024) so the
        // first 1024 pushes don't pay realloc. Non-empty
        // literals pay one realloc on the first push past
        // their initial size — accepted; tightening the
        // initial allocation reduces idle bytes per
        // function.
        let initial_cap: u32 = if n == 0 { 1024 } else { n };
        let alloc_size =
          (ARRAY_HEADER_SIZE as u32 + initial_cap * STACK_SLOT_SIZE) as u64;

        // The most recent call result lives in X0 (not
        // covered by the X1..X15 caller-save). If any
        // element was assigned X0 by the allocator, save
        // it before malloc clobbers X0 with the heap
        // pointer, then reload into X20 after.
        let x0_elem = elements
          .iter()
          .find(|e| self.alloc_reg(**e) == Some(X0))
          .copied();

        if x0_elem.is_some() {
          let save_off =
            self.caller_save_base + CALLER_SAVE_COUNT as u32 * STACK_SLOT_SIZE;

          self.emit_str_sp(X0, save_off);
        }

        self.emit_mov_imm_64(X0, alloc_size);
        self.emit_extern_call("_malloc");

        let r_buf = Register::new(19);

        if let Some(vid) = x0_elem {
          let save_off =
            self.caller_save_base + CALLER_SAVE_COUNT as u32 * STACK_SLOT_SIZE;
          let r_reload = Register::new(20);

          self.emit_ldr_sp(r_reload, save_off);
          self
            .reload_overrides
            .insert((self.current_fn_start.unwrap_or(0) as u32, vid.0), 20);
        }

        self.emitter.emit_mov_reg(r_buf, X0);

        // Header: len = n, cap = initial_cap.
        self.emit_mov_imm_64(X16, n as u64);
        self.emitter.emit_str(X16, r_buf, 0);
        self.emit_mov_imm_64(X16, initial_cap as u64);
        self.emitter.emit_str(X16, r_buf, 8);

        for (i, elem) in elements.iter().enumerate() {
          let off_u16 =
            ARRAY_HEADER_SIZE + (i as u16) * (STACK_SLOT_SIZE as u16);

          self.emit_array_element_store(*elem, all_insns, r_buf, off_u16);
        }

        // Spill the heap pointer to a stack slot so later
        // Store/Load can find it. Same shape as the original
        // empty path — only one slot per literal.
        let base = self.struct_base + self.next_struct_slot;

        self.emit_str_sp(r_buf, base);

        if let Some(dst) = self.reg_for_insn(idx) {
          self.emitter.emit_mov_reg(dst, r_buf);
        }

        self.next_struct_slot += STACK_SLOT_SIZE;
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
        owner,
      } => {
        // Reactive `[]str` array: the elements live in the
        // runtime's `ARR_STATE`, so push routes through the FFI
        // (which copies the str + marks the slot dirty) instead
        // of the local realloc path.
        if let Some(sym) = owner
          && self.reactive_arr_slots.contains(sym)
          && let Some(&slot) = self.reactive_slots.get(sym)
        {
          let val_reg = self.alloc_reg(*value).unwrap_or(X1);

          self.emit_state_arr_push(slot, val_reg);

          return;
        }

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
        // Save into the array-push scratch (past struct
        // slots) so a `val_reg` pointing into a struct
        // can't be stored over the struct itself.
        let push_scratch = self.array_push_scratch_base;

        self.emit_str_sp(X17, push_scratch);
        self.emit_str_sp(val_reg, push_scratch + 8);
        self.emit_extern_call("_realloc");
        // X0 = new pointer. Restore new_cap + value.
        self.emit_ldr_sp(X17, push_scratch);
        self.emit_ldr_sp(val_reg, push_scratch + 8);
        // Store new cap.
        self.emitter.emit_str(X17, X0, 8);
        // Update arr_reg to new pointer.
        self.emitter.emit_mov_reg(arr_reg, X0);
        // Write the new pointer back to the array's local
        // slot. The executor stamped the receiver's
        // `Symbol` onto `Insn::ArrayPush.owner` when the
        // receiver was a bare ident, so the codegen reads
        // it directly — no SIR scan.
        if let Some(sym) = owner
          && let Some(&off) = self.mutable_slots.get(&sym.as_u32())
        {
          self.emit_str_sp(arr_reg, off);
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

        // For struct elements, snapshot the bytes at push
        // time — `val_reg` points into the function frame,
        // and the next iteration would reuse that slot,
        // aliasing every prior element. Field-access codegen
        // already dereferences through the array slot, so
        // storing a heap pointer is transparent to readers.
        //
        // Lookup is via the VALUE's type — the array's
        // `ty_id` can be a narrow / alias form while
        // `value_types` records the canonical struct ty
        // stamped by the StructConstruct.
        let struct_slots = self
          .value_types
          .get(&value.0)
          .and_then(|vt| self.struct_metas.get(&vt.0))
          .map(|m| m.fields.len() as u32);

        if is_flt {
          let fp = self.alloc_fp_reg(*value).unwrap_or(D0);
          self.emitter.emit_str_fp(fp, X17, 0);
        } else if let Some(slots) = struct_slots {
          let bytes = slots * STACK_SLOT_SIZE;
          let push_scratch = self.array_push_scratch_base;

          // Save arr_reg + the dest slot (X17). `len` lives
          // in the array's len header word and we reload it
          // from there after the BL, so X16 doesn't need
          // saving.
          self.emit_str_sp(arr_reg, push_scratch);
          self.emit_str_sp(X17, push_scratch + 8);

          self.emitter.emit_mov_reg(X0, val_reg);
          self.emit_mov_imm_64(X1, bytes as u64);
          self.emit_extern_call("_zo_box_alloc");

          self.emit_ldr_sp(arr_reg, push_scratch);
          self.emit_ldr_sp(X17, push_scratch + 8);
          // Reload len from the array header — the BL
          // clobbered X16, and the post-store increment
          // needs the freshly-loaded value anyway.
          self.emitter.emit_ldr(X16, arr_reg, 0);
          self.emitter.emit_str(X0, X17, 0);
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
      Insn::StructDef {
        name,
        ty_id,
        fields,
        ..
      } => {
        self.register_struct_meta(*name, *ty_id, fields);
      }
      Insn::ConstDef { .. }
      | Insn::ArrayTyDef { .. }
      | Insn::MapTyDef { .. } => {}

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
        dst,
        variant,
        fields,
        ..
      } => {
        let slot_count = 1 + fields.len() as u32;
        let base = self.struct_base + self.next_struct_slot;

        // Pin down the per-construction payload types so the
        // pretty-printer can dispatch on the concrete payload
        // type rather than the enum template's generic `$T`.
        // Without this, `Maybe<str>::Some("hi")` falls through
        // to `emit_itoa_and_write` and prints the str header
        // pointer.
        let concrete_field_tys: Vec<TyId> = fields
          .iter()
          .map(|f| self.type_of(*f).unwrap_or(TyId(0)))
          .collect();

        self
          .value_enum_field_tys
          .insert(dst.0, (*variant, concrete_field_tys));

        // Store discriminant at base.
        self.emitter.emit_mov_imm(X16, *variant as u16);
        self.emit_str_sp(X16, base);

        // Store fields (if any) at base + (i+1)*8.
        for (i, field) in fields.iter().enumerate() {
          let off = base + (i as u32 + 1) * STACK_SLOT_SIZE;

          self.emit_array_element_store_sp(*field, all_insns, off);
        }

        if let Some(dst_reg) = self.reg_for_insn(idx) {
          self.emit_add_sp_offset(dst_reg, base);
        }

        // Persist the slot for `Insn::Return`'s fallback —
        // if the regalloc didn't pick a register for this
        // composite value, Return rebuilds the SP+offset
        // pointer from this map.
        self.composite_value_slots.insert(*dst, base);

        self.next_struct_slot += slot_count * STACK_SLOT_SIZE;
      }

      // Struct construction: store fields into
      // pre-allocated frame slots. struct_base +
      // next_struct_slot is this struct's start offset
      // from SP.
      Insn::StructConstruct { dst, fields, .. } => {
        let base = self.struct_base + self.next_struct_slot;

        for (i, field) in fields.iter().enumerate() {
          let off = base + i as u32 * STACK_SLOT_SIZE;

          self.emit_array_element_store_sp(*field, all_insns, off);
        }

        // Set dst register to point at this struct's
        // base. Use ADD (not MOV) because ARM64 MOV
        // via ORR encodes register 31 as XZR, not SP.
        if let Some(dst_reg) = self.reg_for_insn(idx) {
          self.emit_add_sp_offset(dst_reg, base);
        }

        // Same fallback map as `Insn::EnumConstruct` so
        // `Insn::Return` can rebuild the SP+offset pointer
        // when no GP register was assigned.
        self.composite_value_slots.insert(*dst, base);

        self.next_struct_slot += fields.len() as u32 * STACK_SLOT_SIZE;
      }

      // Struct/tuple field access: load from
      // base + index * 8.
      // Tuple construction: same layout as structs.
      // Store each element at pre-allocated frame slots.
      Insn::TupleLiteral { dst, elements, .. } => {
        let base = self.struct_base + self.next_struct_slot;

        for (i, elem) in elements.iter().enumerate() {
          let off = base + i as u32 * STACK_SLOT_SIZE;

          self.emit_array_element_store_sp(*elem, all_insns, off);
        }

        if let Some(dst_reg) = self.reg_for_insn(idx) {
          self.emit_add_sp_offset(dst_reg, base);
        }

        self.next_struct_slot += elements.len() as u32 * STACK_SLOT_SIZE;

        let elem_tys: Vec<TyId> = elements
          .iter()
          .map(|e| self.value_types.get(&e.0).copied().unwrap_or(TyId(0)))
          .collect();

        self.value_tuple_elem_tys.insert(dst.0, elem_tys);
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

      // Struct field write: store value to base + index * 8.
      // Float fields live in the FP file — a GP `STR` would
      // spill an uninitialized X register, so pick the store
      // by field type, mirroring the `TupleIndex` read.
      Insn::FieldStore {
        base,
        index,
        value,
        ty_id,
      } => {
        let base_reg = self.alloc_reg(*base).unwrap_or(X0);
        let offset = (*index as i16) * (STACK_SLOT_SIZE as i16);
        let is_flt =
          ty_id.0 >= FLOAT_TYPE_ID_MIN && ty_id.0 <= FLOAT_TYPE_ID_MAX;

        if is_flt {
          let fp_src = self.alloc_fp_reg(*value).unwrap_or(D0);

          self.emitter.emit_str_fp(fp_src, base_reg, offset as u16);
        } else {
          let val_reg = self.alloc_reg(*value).unwrap_or(X1);

          self.emitter.emit_str(val_reg, base_reg, offset);
        }
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

        // FLOAT_TYPE_ID_MAX is `Arch`, NOT F64 — match the
        // f64 case against the dedicated `F64_TYPE_ID`.
        let is_from_f32 = from == FLOAT_TYPE_ID_MIN;
        let is_from_f64 = from == F64_TYPE_ID;
        let is_to_f32 = to == FLOAT_TYPE_ID_MIN;
        let is_to_f64 = to == F64_TYPE_ID;

        match (is_from_float, is_to_float) {
          // FP regs are uniformly 64-bit internally — f32→f64
          // is just a register move (no FCVT); only the
          // narrowing direction emits a real conversion.
          (true, true) if from != to => {
            let fp_src = self.alloc_fp_reg(*src).unwrap_or(D0);
            let fp_dst = self.alloc_fp_reg(*dst).unwrap_or(D0);

            if is_from_f64 && is_to_f32 {
              self.emitter.emit_fcvt_d_to_s(fp_dst, fp_src);
            } else if is_from_f32 && is_to_f64 && fp_dst != fp_src {
              self.emitter.emit_fmov_fp(fp_dst, fp_src);
            }
          }
          // float → int: FCVTZS Xd, Ds.
          (true, false) => {
            let fp_src = self.alloc_fp_reg(*src).unwrap_or(D0);
            let gp_dst = self.alloc_reg(*dst).unwrap_or(X0);

            self.emitter.emit_fcvtzs(gp_dst, fp_src);
          }
          // int → float: SCVTF Dd, Xs.
          (false, true) => {
            let gp_src = self.alloc_reg(*src).unwrap_or(X0);
            let fp_dst = self.alloc_fp_reg(*dst).unwrap_or(D0);

            self.emitter.emit_scvtf(fp_dst, gp_src);
          }
          // GP → GP: int/char/bytes/bool share the GP file;
          // MOV between distinct regs, no-op otherwise.
          (false, false) => {
            let src_reg = self.alloc_reg(*src).unwrap_or(X0);
            let dst_reg = self.alloc_reg(*dst).unwrap_or(X0);

            if src_reg != dst_reg {
              self.emitter.emit_mov_reg(dst_reg, src_reg);
            }
          }
          // Same-width float cast (e.g. `cast f64 as f64`).
          (true, true) => {}
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
        let slot = self.chan_scratch_base;

        if let Some(src_reg) = self.alloc_reg(*value) {
          self.emit_str_sp(src_reg, slot);
        }

        if let Some(ch_reg) = self.alloc_reg(*channel)
          && ch_reg != X0
        {
          self.emitter.emit_mov_reg(X0, ch_reg);
        }

        self.emit_add_sp_offset(X1, slot);
        self.emit_extern_call("_zo_chan_send");
      }
      Insn::ChannelRecv {
        dst,
        channel,
        ty_id,
      } => {
        // ABI: `_zo_chan_recv(chan, dst: *mut u8)`.
        // The runtime writes `elem_sz` bytes into the
        // scratch slot; we load the value at the correct
        // width so upper bits are zero-extended.
        let slot = self.chan_scratch_base;

        if let Some(ch_reg) = self.alloc_reg(*channel)
          && ch_reg != X0
        {
          self.emitter.emit_mov_reg(X0, ch_reg);
        }

        self.emit_add_sp_offset(X1, slot);
        self.emit_extern_call("_zo_chan_recv");

        if let Some(dst_reg) = self.alloc_reg(*dst) {
          let elem_sz = self.size_of_ty(*ty_id);

          if elem_sz <= 4 && slot.is_multiple_of(4) {
            self.emitter.emit_ldr_w(dst_reg, SP, slot as i16);
          } else {
            self.emit_ldr_sp(dst_reg, slot);
          }
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
      Insn::FnAddr {
        dst,
        callee,
        callee_pack,
      } => {
        // Materialize the callee's code address into `dst`'s
        // register via an ADR placeholder, then record the
        // same user-function-address fixup `TaskSpawn` uses.
        // The fixup reads `rd` back from the emitted ADR, so
        // any destination register is preserved when patched.
        if let Some(dst_reg) = self.alloc_reg(*dst) {
          let adr_pos = self.emitter.current_offset();

          self.emitter.emit_adr(dst_reg, 0);
          self
            .function_addr_fixups
            .push((adr_pos, (*callee, *callee_pack)));
        }
      }
      Insn::TaskSpawn {
        dst,
        kind,
        callee,
        callee_pack,
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
        self
          .function_addr_fixups
          .push((adr_pos, (*callee, *callee_pack)));

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

            self.emit_str_sp(src, off);
          }
        }

        // Zero the out buffer so the post-call LDR
        // reads any bytes the runtime didn't touch as
        // zero instead of stale stack contents. Wider
        // elem types (`elem_sz > 8`) are a later scope;
        // today the ABI tops out at 8-byte scalars and
        // pointer-backed 8-byte handles.
        self.emit_str_sp(XZR, out_base);

        // X0 = chans_array_ptr.
        self.emit_add_sp_offset(X0, chans_base);
        // X1 = nchans.
        self.emit_mov_imm_64(X1, nchans as u64);
        // X2 = out_value_ptr.
        self.emit_add_sp_offset(X2, out_base);
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
          self.emit_ldr_sp(dst_reg, off);
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

      Insn::ToStr { dst, src, src_ty } => {
        if src_ty.0 == STR_TYPE_ID {
          let src_reg = self.alloc_reg(*src).unwrap_or(X0);
          let dst_reg = self.alloc_reg(*dst).unwrap_or(X0);

          if src_reg != dst_reg {
            self.emitter.emit_mov_reg(dst_reg, src_reg);
          }
        } else {
          let is_flt =
            src_ty.0 >= FLOAT_TYPE_ID_MIN && src_ty.0 <= FLOAT_TYPE_ID_MAX;

          if is_flt {
            if let Some(fp_src) = self.alloc_fp_reg(*src)
              && fp_src != D0
            {
              self.emitter.emit_fmov_fp(D0, fp_src);
            }

            self.emit_extern_call("_zo_float_to_str");
          } else if src_ty.0 == BOOL_TYPE_ID {
            if let Some(src_reg) = self.alloc_reg(*src)
              && src_reg != X0
            {
              self.emitter.emit_mov_reg(X0, src_reg);
            }

            self.emit_extern_call("_zo_bool_to_str");
          } else if src_ty.0 == CHAR_TYPE_ID {
            if let Some(src_reg) = self.alloc_reg(*src)
              && src_reg != X0
            {
              self.emitter.emit_mov_reg(X0, src_reg);
            }

            self.emit_extern_call("_zo_char_to_str");
          } else {
            if let Some(src_reg) = self.alloc_reg(*src)
              && src_reg != X0
            {
              self.emitter.emit_mov_reg(X0, src_reg);
            }

            self.emit_extern_call("_zo_int_to_str");
          }

          if let Some(dst_reg) = self.alloc_reg(*dst)
            && dst_reg != X0
          {
            self.emitter.emit_mov_reg(dst_reg, X0);
          }
        }
      }

      Insn::StringFormat { dst, segments, .. } => {
        let count = segments.len();
        let base = self.string_format_scratch_base;

        for (i, seg) in segments.iter().enumerate() {
          if let Some(reg) = self.alloc_reg(*seg) {
            let off = base + (i as u32) * STACK_SLOT_SIZE;

            self.emit_str_sp(reg, off);
          }
        }

        self.emit_mov_imm_64(X0, count as u64);
        self.emit_add_sp_offset(X1, base);
        self.emit_extern_call("_zo_str_multi_concat");

        if let Some(dst_reg) = self.alloc_reg(*dst)
          && dst_reg != X0
        {
          self.emitter.emit_mov_reg(dst_reg, X0);
        }
      }

      // `any <Abstract>` boxing — lowers to
      // `_zo_dyn_box(src, vtable_ptr) -> fat_ptr`. X0
      // carries src, X1 the link-time-resolved vtable
      // address; the return fat-pointer is moved to dst.
      Insn::CoerceToDyn {
        dst,
        src,
        abstract_name,
        concrete_ty,
      } => 'arm: {
        let Some(vtable_sym) =
          self.resolve_vtable_sym(*abstract_name, *concrete_ty)
        else {
          break 'arm;
        };

        if let Some(src_reg) = self.alloc_reg(*src)
          && src_reg != X0
        {
          self.emitter.emit_mov_reg(X0, src_reg);
        }

        let adr_pos = self.emitter.current_offset();

        self.emitter.emit_adr(X1, 0);
        self.vtable_addr_fixups.push((adr_pos, vtable_sym));

        self.emit_extern_call("_zo_dyn_box");

        if let Some(dst_reg) = self.alloc_reg(*dst)
          && dst_reg != X0
        {
          self.emitter.emit_mov_reg(dst_reg, X0);
        }
      }

      // Dynamic dispatch through a vtable. Receiver in
      // `recv` is a fat-pointer (heap-boxed 16 bytes:
      // data_ptr + vtable_ptr). Two LDRs unpack the
      // pair, a third LDR reads the method's address
      // out of the vtable, BLR jumps. Other args go in
      // X1..XN per the standard AAPCS.
      Insn::DynDispatch {
        dst,
        recv,
        method_index,
        args,
        ..
      } => {
        // Load recv (fat-pointer base) into a scratch.
        // Reuse X16 — the BL/BLR scratch register the
        // call ABI documents as caller-clobbered.
        if let Some(recv_reg) = self.alloc_reg(*recv)
          && recv_reg != X16
        {
          self.emitter.emit_mov_reg(X16, recv_reg);
        }

        // X0 ← data_ptr = [X16 + 0] (the `self`
        // argument for the dispatched method).
        self.emitter.emit_ldr(X0, X16, 0);

        // X16 ← vtable_ptr = [X16 + 8].
        self.emitter.emit_ldr(X16, X16, 8);

        // X17 ← slot_value = [X16 + 8 + idx*8]. The
        // `+ 8` skips the vtable's size_of_data slot;
        // each method slot is 8 bytes. The slot stores
        // `method_addr − vtable_addr` (a signed
        // 64-bit relative offset) so the value
        // survives ASLR — both ends slide by the same
        // load bias.
        let slot_offset: i16 = 8 + (*method_index as i16 * 8);

        self.emitter.emit_ldr(X17, X16, slot_offset);

        // X16 ← X16 (vtable_addr) + X17 (offset to
        // method) = absolute method address at the
        // CURRENT load base.
        self.emitter.emit_add(X16, X16, X17);

        // Place explicit args into X1..XN. Limited to
        // 7 regs (X1..X7) — extra args spill to stack
        // via the existing per-call helper. For now,
        // direct-register only; extension to spill is
        // a follow-up that mirrors `Insn::Call`'s arg
        // handling.
        // Explicit args go in X1..X7. AAPCS_ARG_REGS[0]
        // (X0) is already occupied by `self` (data_ptr).
        for (i, arg) in args.iter().enumerate() {
          let Some(&target) = AAPCS_ARG_REGS.get(i + 1) else {
            break;
          };

          if let Some(arg_reg) = self.alloc_reg(*arg)
            && arg_reg != target
          {
            self.emitter.emit_mov_reg(target, arg_reg);
          }
        }

        self.emitter.emit_blr(X16);

        // Result lands in X0 per AAPCS — move to dst's
        // allocated register if different.
        if let Some(dst_reg) = self.alloc_reg(*dst)
          && dst_reg != X0
        {
          self.emitter.emit_mov_reg(dst_reg, X0);
        }
      }

      Insn::TestBegin { count } => {
        self.emitter.emit_mov_imm(X0, *count as u16);
        self.emit_extern_call("_zo_test_begin");
      }
      Insn::TestRun {
        callee,
        callee_pack,
      } => {
        // X0 = callee function address (same ADR+fixup
        // mechanism as TaskSpawn).
        let callee_adr = self.emitter.current_offset();

        self.emitter.emit_adr(X0, 0);
        self
          .function_addr_fixups
          .push((callee_adr, (*callee, *callee_pack)));

        // X1 = name string pointer. Reuse the string
        // literal mechanism — emit a length-prefixed
        // string and an ADR fixup. The runtime reads
        // raw bytes via (ptr, len).
        let name_str = self.interner.get(*callee);
        let name_len = name_str.len();

        if !self.string_data_seen.contains(callee) {
          let mut buffer = Buffer::new();
          let len_bytes = (name_len as u64).to_le_bytes();

          buffer.bytes(&len_bytes);
          buffer.bytes(name_str.as_bytes());
          buffer.bytes(b"\0");
          self.string_data.push((*callee, buffer.finish()));
          self.string_data_seen.insert(*callee);
        }

        let name_adr = self.emitter.current_offset();

        self.string_fixups.push((name_adr, *callee));
        self.emitter.emit_adr(X1, 0);
        // Skip the 8-byte length prefix — runtime
        // receives raw bytes, not the zo string layout.
        self.emitter.emit_add_imm(X1, X1, 8);

        // X2 = name length.
        self.emitter.emit_mov_imm(X2, name_len as u16);
        self.emit_extern_call("_zo_test_run_one");
      }
      Insn::TestSummary => {
        self.emit_extern_call("_zo_test_summary");
      }

      _ => {}
    }
  }

  /// Maps a `TyId` to the `Symbol` that identifies its
  /// concrete nominal type — the same key
  /// `abstract_impls` uses. Struct / enum return their
  /// own name. Unknown shapes return `None`.
  fn concrete_ty_sym(&self, ty_id: TyId) -> Option<Symbol> {
    let view = self.type_view?;
    let ty = *view.tys.get(ty_id.0 as usize)?;

    match ty {
      Ty::Struct(sid) => view.ty_table.struct_ty(sid).map(|s| s.name),
      Ty::Enum(eid) => view.ty_table.enum_ty(eid).map(|e| e.name),
      _ => None,
    }
  }

  /// Looks up the pre-interned `__zo_vtable_<Abs>__<Ty>`
  /// symbol for an `(abstract, concrete TyId)` pair.
  /// Returns `None` when the type doesn't name a nominal
  /// or no `apply Abstract for Type` impl is in scope —
  /// in both cases `Insn::CoerceToDyn` lowering bails
  /// out without emitting the call.
  fn resolve_vtable_sym(
    &self,
    abstract_name: Symbol,
    concrete_ty: TyId,
  ) -> Option<Symbol> {
    let concrete_sym = self.concrete_ty_sym(concrete_ty)?;
    self
      .abstract_impls
      .get(&(abstract_name, concrete_sym))
      .map(|im| im.vtable_sym)
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
      // FP regs are uniformly 64-bit internally (see
      // `ConstFloat` / `Cast`), so D0 already holds the f64
      // bit pattern even for an `f32`-typed value — no FCVT
      // is needed before the call. `_zo_float_to_str` takes
      // its f64 argument in D0 (AAPCS64) and returns a zo
      // `str` pointer in X0; `emit_extern_call` reloads only
      // X1..X15, leaving that return value intact.
      if let Some(fp_src) = arg_vid.and_then(|v| self.alloc_fp_reg(v))
        && fp_src != D0
      {
        self.emitter.emit_fmov_fp(D0, fp_src);
      }

      self.emit_extern_call("_zo_float_to_str");
      self.emit_zo_str_write(X0, fd);
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
    } else if let Some((kf, vf)) = arg_vid.and_then(|v| self.is_map_value(v))
      && let Some(vid) = arg_vid
    {
      // HashMap — load `m.ptr` and hand the iteration off
      // to `_zo_map_show` in the runtime. The runtime walks
      // occupied slots and formats each `key: value` using
      // the `MapFmt` discriminants we passed; codegen never
      // sees the slot bytes itself.
      self.emit_map_write(vid, kf, vf, fd);
    } else if let Some(elem_fmt) = arg_vid.and_then(|v| self.is_vec_value(v))
      && let Some(vid) = arg_vid
    {
      // `Vec<$T>` — same shape as map: load `v.ptr`, hand
      // off to `_zo_vec_show` with the element kind.
      // Dispatched BEFORE `is_struct_value` because Vec is
      // structurally `struct Vec { ptr: int }` — the struct
      // walker would print `Vec { ptr: <int> }` instead of
      // the elements.
      self.emit_vec_write(vid, elem_fmt, fd);
    } else if let Some(key_fmt) = arg_vid.and_then(|v| self.is_set_value(v))
      && let Some(vid) = arg_vid
    {
      // `HashSet<$K>` — same shape as Vec, routed to
      // `_zo_set_show`. Same struct-walker shadowing
      // concern as Vec.
      self.emit_set_write(vid, key_fmt, fd);
    } else if let Some(elem_tys) = arg_vid.and_then(|v| self.is_tuple_value(v))
      && let Some(vid) = arg_vid
    {
      // Tuple — `(e0, e1, ...)`. Same memory layout as a
      // struct (8-byte slots in declaration order); we walk
      // and dispatch each slot through `emit_field_write`.
      self.emit_tuple_write(vid, &elem_tys, fd);
    } else if let Some(ty_id) = arg_vid.and_then(|v| self.is_struct_value(v))
      && let Some(vid) = arg_vid
    {
      // Struct — `Name { f0: v0, f1: v1, ... }`. Receiver
      // register holds the struct base pointer; field
      // labels and types come from the per-StructDef
      // `struct_metas` entry.
      self.emit_struct_write(vid, ty_id, fd);
    } else if !is_str && arg_vid.is_some() {
      if let Some(src) = arg_vid.and_then(|v| self.alloc_reg(v))
        && src != X0
      {
        self.emitter.emit_mov_reg(X0, src);
      }

      self.emit_itoa_and_write(fd);
    } else if is_str {
      // String: single pointer to [len: u64][bytes][null].
      let ptr = arg_vid.and_then(|v| self.alloc_reg(v)).unwrap_or(X1);

      self.emit_zo_str_write(ptr, fd);
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
      self.string_data_seen.insert(display_sym);

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
    if self.string_data_seen.contains(&sym) {
      return;
    }

    let mut buf = Buffer::new();

    buf.bytes(&(text.len() as u64).to_le_bytes());
    buf.bytes(text);
    buf.bytes(b"\0");

    self.string_data.push((sym, buf.finish()));
    self.string_data_seen.insert(sym);
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

    let mut variants = meta.variants_view();

    // Substitute the construction-site payload types for the
    // matching variant. Generic enums register the template's
    // `Ty::Infer($T)` field types in `enum_metas`; without
    // this override the str payload of `Maybe<str>::Some("hi")`
    // would dispatch through the integer writer.
    if let Some((variant, concrete)) = self.value_enum_field_tys.get(&vid.0)
      && let Some(slot) =
        variants.iter_mut().find(|(disc, _, _)| disc == variant)
      && slot.2.len() == concrete.len()
    {
      slot.2 = concrete.clone();
    }

    let src = self.alloc_reg(vid).unwrap_or(X0);

    // Save enum pointer in X19 (callee-saved, outside
    // allocator pool) so it survives write syscalls.
    self.emitter.emit_mov_reg(Register::new(19), src);
    self.emit_enum_walk_from_x19(&variants, fd);
  }

  /// Body of the enum pretty-printer factored to assume the
  /// enum's pointer already lives in X19. This is the
  /// recursion entry point: when a struct or array element is
  /// itself an enum, `emit_field_write` pushes the outer
  /// X19..X22, moves X0 → X19, calls this, then pops.
  ///
  /// All enum values — unit and tuple — are heap/stack
  /// allocations laid out as `[disc:u64][f0:u64]...`, so X19
  /// is always a valid pointer to the discriminant slot.
  fn emit_enum_walk_from_x19(
    &mut self,
    variants: &[(u32, Symbol, Vec<TyId>)],
    fd: u16,
  ) {
    self.emitter.emit_ldr(X17, Register::new(19), 0);

    // Take the scratch vec out so we can mutate it while
    // also calling other `&mut self` methods (the
    // emitter, synth-str helpers) inside the loop. The
    // capacity is preserved across enum walks via the
    // restore at the end.
    let mut done_fixups = std::mem::take(&mut self.enum_walk_done_fixups);

    done_fixups.clear();

    for (disc, display_sym, field_tys) in variants {
      self.emitter.emit_cmp_imm(X17, *disc as u16);

      let bne_pos = self.emitter.current_offset();
      self.emitter.emit_bne(0);

      self.emit_synthetic_str_write(*display_sym, fd);

      if !field_tys.is_empty() {
        self.emit_synthetic_str_write(ENUM_OPEN_PAREN_SYM, fd);

        for (i, field_ty) in field_tys.iter().enumerate() {
          let offset =
            ((i as i16) + ENUM_PAYLOAD_BASE_SLOT) * STACK_SLOT_SIZE as i16;

          self.emitter.emit_ldr(X0, Register::new(19), offset);

          self.emit_field_write(*field_ty, fd, true);

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

    for &pos in &done_fixups {
      self.emitter.patch_b_at(pos, done_label - pos as i32);
    }

    // Restore the scratch vec so its capacity carries
    // over to the next enum walk.
    self.enum_walk_done_fixups = done_fixups;
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

  fn emit_field_write(&mut self, ty_id: TyId, fd: u16, quoted: bool) {
    // Nested aggregates (array / struct / enum) all use the
    // same recursion shape: save outer X19..X22, move the
    // payload pointer X0 → X19, run the walker, restore. The
    // metadata lookups are lazy and mutually exclusive —
    // probing in order short-circuits before the next one
    // runs, and the enum branch only allocates its variants
    // snapshot when it actually fires.
    if let Some(elem_ty) = self.array_metas.get(&ty_id.0).map(|m| m.elem_ty) {
      self.emit_pp_state_push();
      self.emitter.emit_mov_reg(Register::new(19), X0);
      self.emit_array_walk_from_x19(elem_ty, fd);
      self.emit_pp_state_pop();

      return;
    }

    if self.struct_metas.contains_key(&ty_id.0) {
      self.emit_pp_state_push();
      self.emitter.emit_mov_reg(Register::new(19), X0);
      self.emit_struct_walk_from_x19(ty_id, fd);
      self.emit_pp_state_pop();

      return;
    }

    if let Some(meta) = self.enum_metas.get(&ty_id.0) {
      let variants = meta.variants_view();

      self.emit_pp_state_push();
      self.emitter.emit_mov_reg(Register::new(19), X0);
      self.emit_enum_walk_from_x19(&variants, fd);
      self.emit_pp_state_pop();

      return;
    }

    let is_str = ty_id.0 == STR_TYPE_ID;
    let is_float = ty_id.0 >= FLOAT_TYPE_ID_MIN && ty_id.0 <= FLOAT_TYPE_ID_MAX;
    let is_bool = ty_id.0 == BOOL_TYPE_ID;
    let is_char = ty_id.0 == CHAR_TYPE_ID;

    if is_str {
      // Mirror Rust Debug — strings nested inside an enum
      // / array / map carry surrounding `"`s; the top-level
      // `showln(s)` path passes `quoted = false`. The
      // payload pointer arrives in X0 from the caller's
      // load, so save it in a scratch outside the array /
      // enum writer's working set (X19..X22) across the
      // punctuation syscall, which trashes X0-X17.
      if quoted {
        self.register_punctuation_sym(STR_DQUOTE_SYM, b"\"");
        self.emitter.emit_mov_reg(Register::new(23), X0);
        self.emit_synthetic_str_write(STR_DQUOTE_SYM, fd);
        self.emitter.emit_mov_reg(X0, Register::new(23));
      }

      self.emitter.emit_ldr(X2, X0, 0);
      self.emitter.emit_add_imm(X1, X0, 8);
      self.emitter.emit_mov_imm(X16, SYS_WRITE);
      self.emitter.emit_mov_imm(X0, fd);
      self.emitter.emit_svc(0);

      if quoted {
        self.emit_synthetic_str_write(STR_DQUOTE_SYM, fd);
      }
    } else if is_float {
      // The field's raw 64-bit slot arrives in X0 as an f64
      // bit pattern; move it to D0 and hand off to the
      // shortest-round-trip formatter. `_zo_float_to_str`
      // returns a zo `str` pointer in X0 — same write shape
      // as the `is_str` arm above.
      self.emitter.emit_fmov_gp_to_fp(D0, X0);
      self.emit_extern_call("_zo_float_to_str");
      self.emit_zo_str_write(X0, fd);
    } else if is_bool {
      self.emit_bool_and_write(fd);
    } else if is_char {
      self.emit_char_and_write(fd);
    } else {
      self.emit_itoa_and_write(fd);
    }
  }

  /// Push the four pretty-printer state registers
  /// (X19 / X20 / X21 / X22) onto the stack ahead of a
  /// recursive `emit_*_walk_from_x19` call. Uses pre-indexed
  /// STP so each pair drops 16 bytes in one instruction. The
  /// peer `emit_pp_state_pop` undoes both pushes in reverse
  /// order with post-indexed LDP.
  fn emit_pp_state_push(&mut self) {
    self
      .emitter
      .emit_stp(Register::new(19), Register::new(20), SP, -16);
    self
      .emitter
      .emit_stp(Register::new(21), Register::new(22), SP, -16);
  }

  /// Pop the four pretty-printer state registers in the
  /// reverse order of `emit_pp_state_push`.
  fn emit_pp_state_pop(&mut self) {
    self
      .emitter
      .emit_ldp(Register::new(21), Register::new(22), SP, 16);
    self
      .emitter
      .emit_ldp(Register::new(19), Register::new(20), SP, 16);
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

    self.register_punctuation_sym(ARRAY_OPEN_BRACKET_SYM, b"[");
    self.register_punctuation_sym(ARRAY_CLOSE_BRACKET_SYM, b"]");
    self.register_punctuation_sym(ENUM_COMMA_SPACE_SYM, b", ");

    // Top-level entry: move the receiver into X19 so the
    // walk can use it as the base register. Recursive
    // entries (`emit_field_write` → array element that is
    // itself an array) call `emit_array_walk_from_x19`
    // directly after pushing the outer caller's X19..X22.
    self.emitter.emit_mov_reg(Register::new(19), src);
    self.emit_array_walk_from_x19(elem_ty, fd);
  }

  /// Body of the array pretty-printer factored to assume the
  /// array's base pointer already lives in X19. This shape is
  /// the recursion entry point: when an element is itself an
  /// array, `emit_field_write` pushes X19..X22 onto the stack,
  /// loads the inner base into X19, and re-enters here. The
  /// outer state survives because every level pushes and pops
  /// its own X19..X22 pair.
  fn emit_array_walk_from_x19(&mut self, elem_ty: TyId, fd: u16) {
    let r_base = Register::new(19);
    let r_len = Register::new(20);
    let r_idx = Register::new(21);
    let r_tmp = Register::new(22);

    // Load length from base[0], zero the index.
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
    self.emit_field_write(elem_ty, fd, true);

    // Reload index into X21 — `emit_field_write` may have
    // recursed into another array / struct printer that
    // popped its own pushed X21 back, but the re-pop
    // restores the OUTER caller's X21 (this loop's index)
    // before this point in the sequence. No reload needed.

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

  /// Pretty-print a tuple value as `(e0, e1, ...)`.
  ///
  /// Layout matches `Insn::TupleLiteral` codegen: 8-byte
  /// slots in declaration order, base pointer in the
  /// receiver register. Element count is statically known
  /// from the construction-site override, so this is a
  /// straight-line unrolled walk — no loop, no length
  /// header to read.
  fn emit_tuple_write(&mut self, vid: ValueId, elem_tys: &[TyId], fd: u16) {
    self.register_punctuation_sym(TUPLE_OPEN_PAREN_SYM, b"(");
    self.register_punctuation_sym(TUPLE_CLOSE_PAREN_SYM, b")");
    self.register_punctuation_sym(ENUM_COMMA_SPACE_SYM, b", ");

    let src = self.alloc_reg(vid).unwrap_or(X0);

    // Save the tuple base in X19 (callee-saved, outside
    // the allocator pool) — every write syscall trashes
    // X0..X17, so the base must live somewhere stable for
    // the per-element load loop.
    self.emitter.emit_mov_reg(Register::new(19), src);

    self.emit_synthetic_str_write(TUPLE_OPEN_PAREN_SYM, fd);

    for (i, ty) in elem_tys.iter().enumerate() {
      let off = (i as i16) * STACK_SLOT_SIZE as i16;

      self.emitter.emit_ldr(X0, Register::new(19), off);
      self.emit_field_write(*ty, fd, true);

      if i + 1 < elem_tys.len() {
        self.emit_synthetic_str_write(ENUM_COMMA_SPACE_SYM, fd);
      }
    }

    self.emit_synthetic_str_write(TUPLE_CLOSE_PAREN_SYM, fd);
  }

  /// Record `Insn::StructDef` metadata and pre-bake the
  /// struct header (`"Name { "`) plus each field label
  /// (`"field: "`) into `string_data` under synthetic symbols.
  /// `emit_struct_write` consumes these to produce
  /// `Name { f0: v0, f1: v1, ... }` without any runtime
  /// formatting.
  fn register_struct_meta(
    &mut self,
    name: Symbol,
    ty_id: TyId,
    fields: &[(Symbol, TyId, bool)],
  ) {
    if self.struct_metas.contains_key(&ty_id.0) {
      return;
    }

    let struct_str = self.interner.get(name).to_owned();
    let header = format!("{struct_str} {{ ");
    let header_sym = Symbol(self.next_enum_sym);

    self.next_enum_sym += 1;

    self.intern_synthetic_str(header_sym, header.as_bytes());

    let mut field_metas = Vec::with_capacity(fields.len());

    for (fname, fty, _has_default) in fields {
      let fname_str = self.interner.get(*fname);
      let label = format!("{fname_str} = ");
      let label_sym = Symbol(self.next_enum_sym);

      self.next_enum_sym += 1;

      self.intern_synthetic_str(label_sym, label.as_bytes());

      field_metas.push(StructFieldMeta {
        label_sym,
        ty_id: *fty,
      });
    }

    self.register_punctuation_sym(STRUCT_CLOSE_BRACE_SYM, b" }");
    self.register_punctuation_sym(ENUM_COMMA_SPACE_SYM, b", ");

    self.struct_metas.insert(
      ty_id.0,
      StructMeta {
        header_sym,
        fields: field_metas,
      },
    );
  }

  /// Helper used by `register_struct_meta` to push a length-
  /// prefixed string into `string_data` under a synthetic
  /// symbol, matching the format `emit_synthetic_str_write`
  /// expects.
  fn intern_synthetic_str(&mut self, sym: Symbol, bytes: &[u8]) {
    let mut buf = Buffer::new();

    buf.bytes(&(bytes.len() as u64).to_le_bytes());
    buf.bytes(bytes);
    buf.bytes(b"\0");

    self.string_data.push((sym, buf.finish()));
    self.string_data_seen.insert(sym);
  }

  /// Pretty-print a struct value as `Name { f0: v0, ... }`.
  ///
  /// Layout matches `Insn::StructConstruct` codegen: fields
  /// in declaration order, 8-byte slots, base pointer in the
  /// receiver register. Field count and types are statically
  /// known from `struct_metas`, so this is an unrolled walk
  /// — no length header, no loop. Each field dispatches
  /// through `emit_field_write` (same per-type branching the
  /// enum / tuple / array printers already use).
  fn emit_struct_write(&mut self, vid: ValueId, ty_id: TyId, fd: u16) {
    if !self.struct_metas.contains_key(&ty_id.0) {
      if let Some(src) = self.alloc_reg(vid)
        && src != X0
      {
        self.emitter.emit_mov_reg(X0, src);
      }

      self.emit_itoa_and_write(fd);

      return;
    }

    let src = self.alloc_reg(vid).unwrap_or(X0);

    // Top-level entry: move the receiver into X19 so the
    // walk can use it as the base register. Recursive
    // entries (`emit_field_write` → struct field that is
    // itself a struct) call `emit_struct_walk_from_x19`
    // directly after pushing the outer caller's X19..X22.
    self.emitter.emit_mov_reg(Register::new(19), src);
    self.emit_struct_walk_from_x19(ty_id, fd);
  }

  /// Body of the struct pretty-printer factored to assume the
  /// struct's base pointer already lives in X19. Same role as
  /// `emit_array_walk_from_x19`: recursion entry point for
  /// `emit_field_write` after the outer X19..X22 are saved.
  fn emit_struct_walk_from_x19(&mut self, ty_id: TyId, fd: u16) {
    let Some(meta) = self.struct_metas.get(&ty_id.0) else {
      return;
    };

    let header_sym = meta.header_sym;
    let fields: Vec<(Symbol, TyId)> =
      meta.fields.iter().map(|f| (f.label_sym, f.ty_id)).collect();

    self.emit_synthetic_str_write(header_sym, fd);

    for (i, (label_sym, fty)) in fields.iter().enumerate() {
      self.emit_synthetic_str_write(*label_sym, fd);

      let off = (i as i16) * STACK_SLOT_SIZE as i16;

      self.emitter.emit_ldr(X0, Register::new(19), off);
      self.emit_field_write(*fty, fd, true);

      if i + 1 < fields.len() {
        self.emit_synthetic_str_write(ENUM_COMMA_SPACE_SYM, fd);
      }
    }

    self.emit_synthetic_str_write(STRUCT_CLOSE_BRACE_SYM, fd);
  }

  /// Pretty-print a `HashMap<K, V>` as `{k0: v0, ...}`.
  ///
  /// The receiver register holds the map's struct
  /// address (`{ ptr }` shape, see `emit_map_new`); we
  /// load `m.ptr` from offset 0, set the runtime ABI
  /// args (X0=map, X1=fd, X2=key_fmt, X3=val_fmt), and
  /// hand the formatting to `_zo_map_show`. Iteration
  /// order is the map's bucket order — implementation-
  /// defined, identical to Rust's `HashMap` Debug.
  ///
  /// Doing the iteration in Rust rather than ASM keeps
  /// the codegen footprint flat: bucket walk, per-slot
  /// occupied check, and per-side scalar formatting all
  /// live in one place where they can share buffer
  /// state and emit a single `write` syscall.
  fn emit_map_write(
    &mut self,
    vid: ValueId,
    key_fmt: u32,
    val_fmt: u32,
    fd: u16,
  ) {
    let recv = self.alloc_reg(vid).unwrap_or(X0);

    // X0 = m.ptr; X1 = fd; X2 = key_fmt; X3 = val_fmt.
    self.emitter.emit_ldr(X0, recv, 0);
    self.emitter.emit_mov_imm(X1, fd);
    self.emitter.emit_mov_imm(X2, key_fmt as u16);
    self.emitter.emit_mov_imm(X3, val_fmt as u16);
    self.emit_extern_call("_zo_map_show");
  }

  /// Pretty-print a `Vec<$T>` as `[e0, e1, ...]`. Same shape
  /// as `emit_map_write` — receiver register holds the
  /// `Vec { ptr }` struct address; we load `v.ptr` and hand
  /// off to `_zo_vec_show` with the element kind. The
  /// runtime walks the live elements, formats each with
  /// `MapFmt::format_bytes`, and emits a single `write`
  /// syscall.
  fn emit_vec_write(&mut self, vid: ValueId, elem_fmt: u32, fd: u16) {
    let recv = self.alloc_reg(vid).unwrap_or(X0);

    self.emitter.emit_ldr(X0, recv, 0);
    self.emitter.emit_mov_imm(X1, fd);
    self.emitter.emit_mov_imm(X2, elem_fmt as u16);
    self.emit_extern_call("_zo_vec_show");
  }

  /// Pretty-print a `HashSet<$K>` as `{k0, k1, ...}`. Same
  /// shape as `emit_vec_write` but routed through
  /// `_zo_set_show`, which walks the underlying `ZoMap`
  /// (sets reuse the map allocator) and prints just the
  /// keys — no `: value` per entry.
  fn emit_set_write(&mut self, vid: ValueId, key_fmt: u32, fd: u16) {
    let recv = self.alloc_reg(vid).unwrap_or(X0);

    self.emitter.emit_ldr(X0, recv, 0);
    self.emitter.emit_mov_imm(X1, fd);
    self.emitter.emit_mov_imm(X2, key_fmt as u16);
    self.emit_extern_call("_zo_set_show");
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
    // Inline UTF-8 encoder for `show(c: char)` — codepoint
    // in X0, branches on magnitude to emit 1/2/3/4 bytes.
    // Without this every non-ASCII char would silently
    // truncate to its low byte.
    self.emitter.emit_mov_reg(X9, X0); // X9 = codepoint
    self.emitter.emit_sub_imm(X1, SP, NEWLINE_BUFFER_OFFSET);

    self.emitter.emit_cmp_imm(X9, 0x80);
    let to_one = self.emitter.forward_blt();

    self.emit_mov_imm_64(X16, 0x800);
    self.emitter.emit_cmp(X9, X16);
    let to_two = self.emitter.forward_blt();

    self.emit_mov_imm_64(X16, 0x10000);
    self.emitter.emit_cmp(X9, X16);
    let to_three = self.emitter.forward_blt();

    // Fall-through: 4-byte encoding.
    self.emit_utf8_byte(X9, 18, 0xF0, X1, 0);
    self.emit_utf8_byte(X9, 12, 0x80, X1, 1);
    self.emit_utf8_byte(X9, 6, 0x80, X1, 2);
    self.emit_utf8_byte(X9, 0, 0x80, X1, 3);
    self.emitter.emit_mov_imm(X2, 4);
    let to_write_4 = self.emitter.forward_b();

    self.emitter.bind_here(to_three);
    self.emit_utf8_byte(X9, 12, 0xE0, X1, 0);
    self.emit_utf8_byte(X9, 6, 0x80, X1, 1);
    self.emit_utf8_byte(X9, 0, 0x80, X1, 2);
    self.emitter.emit_mov_imm(X2, 3);
    let to_write_3 = self.emitter.forward_b();

    self.emitter.bind_here(to_two);
    self.emit_utf8_byte(X9, 6, 0xC0, X1, 0);
    self.emit_utf8_byte(X9, 0, 0x80, X1, 1);
    self.emitter.emit_mov_imm(X2, 2);
    let to_write_2 = self.emitter.forward_b();

    // 1-byte (ASCII) — store low byte verbatim.
    self.emitter.bind_here(to_one);
    self.emitter.emit_strb(X9, X1, 0);
    self.emitter.emit_mov_imm(X2, 1);

    self.emitter.bind_here(to_write_4);
    self.emitter.bind_here(to_write_3);
    self.emitter.bind_here(to_write_2);
    self.emitter.emit_mov_imm(X16, SYS_WRITE);
    self.emitter.emit_mov_imm(X0, fd);
    self.emitter.emit_svc(0);
  }

  /// Emit one UTF-8 byte at `[buf_reg + buf_off]`.
  /// Continuation bytes (`tag == 0x80`) keep only the low 6
  /// bits of the shifted codepoint; leading bytes use `tag`
  /// directly. Clobbers X16/X17.
  fn emit_utf8_byte(
    &mut self,
    cp_reg: Register,
    shift: u8,
    tag: u16,
    buf_reg: Register,
    buf_off: i16,
  ) {
    if shift > 0 {
      self.emitter.emit_lsr(X16, cp_reg, shift);
    } else {
      self.emitter.emit_mov_reg(X16, cp_reg);
    }

    if tag == 0x80 {
      self.emitter.emit_mov_imm(X17, 0x3F);
      self.emitter.emit_and(X16, X16, X17);
    }

    self.emitter.emit_mov_imm(X17, tag);
    self.emitter.emit_orr(X16, X16, X17);
    self.emitter.emit_strb(X16, buf_reg, buf_off);
  }

  /// Write a zo `str` whose pointer lives in `ptr_reg` to
  /// file descriptor `fd`.
  ///
  /// @note — a zo `str` is a pointer to `[len: u64][bytes]
  /// [null]`. The pointer moves to X16 first so reusing X1 /
  /// X2 for the `write` syscall args cannot clobber it.
  fn emit_zo_str_write(&mut self, ptr_reg: Register, fd: u16) {
    if ptr_reg != X16 {
      self.emitter.emit_mov_reg(X16, ptr_reg);
    }

    // LDR X2, [X16, #0] — length header at offset 0.
    self.emitter.emit_ldr(X2, X16, 0);
    // ADD X1, X16, #8 — payload starts at offset 8.
    self.emitter.emit_add_imm(X1, X16, 8);
    self.emitter.emit_mov_imm(X16, SYS_WRITE);
    self.emitter.emit_mov_imm(X0, fd);
    self.emitter.emit_svc(0);
  }

  /// Emit bool-to-string write: prints "true" or "false".
  /// X0 holds the bool value (0 or 1).
  fn emit_bool_and_write(&mut self, fd: u16) {
    let sym_true = Symbol(0xFFFD);
    let sym_false = Symbol(0xFFFC);

    // Register "true" and "false" string data once.
    if !self.string_data_seen.contains(&sym_true) {
      let mut buf = Buffer::new();
      let len = 4u64; // "true".len()

      buf.bytes(&len.to_le_bytes());
      buf.bytes(b"true");
      buf.bytes(b"\0");

      self.string_data.push((sym_true, buf.finish()));
      self.string_data_seen.insert(sym_true);
    }

    if !self.string_data_seen.contains(&sym_false) {
      let mut buf = Buffer::new();
      let len = 5u64; // "false".len()

      buf.bytes(&len.to_le_bytes());
      buf.bytes(b"false");
      buf.bytes(b"\0");

      self.string_data.push((sym_false, buf.finish()));
      self.string_data_seen.insert(sym_false);
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

  /// Runtime string concatenation: `a ++ b`.
  ///
  /// Both operands are pointers to `[len:u64][bytes][null]`.
  /// Result is a fresh heap-allocated zo str owned by the
  /// runtime (see `_zo_str_concat` in `zo-runtime/src/str`).
  ///
  /// @note — the previous inline body permanently lowered
  /// SP by a runtime-computed amount and relied on the
  /// epilogue's fixed-constant `ADD SP, SP, frame` to
  /// clean it up. SP stayed unbalanced, so the epilogue's
  /// `LDP X29, X30, [SP]` read garbage and `RET` jumped
  /// to a junk address — observed as a hang on any
  /// `literal ++ runtime_str` after an FFI call. Routing
  /// through the runtime keeps SP stable and matches the
  /// `_zo_str_slice` allocation model.
  fn emit_str_concat(&mut self, dst: Register, lhs: Register, rhs: Register) {
    self.emit_safe_int_arg_moves(&[(X0, lhs), (X1, rhs)]);
    self.emit_extern_call("_zo_str_concat");

    if dst != X0 {
      self.emitter.emit_mov_reg(dst, X0);
    }
  }

  /// Clobber-safe int arg marshaling. Given (dst, src)
  /// pairs, emits a sequence of `mov` that always lands
  /// the right value in each `dst`, even if a later
  /// `dst` is some other move's `src`. Two scratch slots
  /// (X16, X17) cover up to a full 2-cycle (`X0←X1,
  /// X1←X0`). AAPCS declares both caller-saved, and no
  /// live value occupies them at a call-site boundary.
  fn emit_safe_int_arg_moves(&mut self, moves: &[(Register, Register)]) {
    let scratches = [X16, X17];
    let mut saved: Vec<(Register, Register)> = Vec::new();

    for j in 0..moves.len() {
      let (_, src) = moves[j];

      let is_clobbered = moves
        .iter()
        .enumerate()
        .any(|(k, (dst, _))| k != j && *dst == src);

      if is_clobbered
        && !saved.iter().any(|(orig, _)| *orig == src)
        && saved.len() < scratches.len()
      {
        let scratch = scratches[saved.len()];

        self.emitter.emit_mov_reg(scratch, src);
        saved.push((src, scratch));
      }
    }

    for &(dst, src) in moves {
      let actual_src = saved
        .iter()
        .find(|(orig, _)| *orig == src)
        .map(|(_, scratch)| *scratch)
        .unwrap_or(src);

      if dst != actual_src {
        self.emitter.emit_mov_reg(dst, actual_src);
      }
    }
  }

  // ================================================================
  // AAPCS marshaling primitives shared by `emit_ffi_call`
  // (the generic FFI path). FP register moves use the same
  // clobber-safe pattern as `emit_safe_int_arg_moves` —
  // stash through D16 when a destination would overwrite a
  // still-needed source.
  // ================================================================

  /// Clobber-safe FP register marshaling — mirrors
  /// `emit_safe_int_arg_moves`. Stashes through D16 when a
  /// later destination would overwrite a still-needed
  /// source. AAPCS uses D0-D7 for FP args (caller-saved);
  /// D16-D31 are also caller-saved, so D16 is a free
  /// scratch slot during call-site marshaling.
  fn emit_safe_fp_arg_moves(&mut self, moves: &[(FpRegister, FpRegister)]) {
    let mut saved_reg: Option<FpRegister> = None;

    for j in 0..moves.len() {
      let (_, src) = moves[j];

      let is_clobbered = moves
        .iter()
        .enumerate()
        .any(|(k, (dst, _))| k != j && *dst == src);

      if is_clobbered && saved_reg.is_none() {
        self.emitter.emit_fmov_fp(D16, src);
        saved_reg = Some(src);
      }
    }

    for &(dst, src) in moves {
      let actual_src = if Some(src) == saved_reg { D16 } else { src };

      if dst != actual_src {
        self.emitter.emit_fmov_fp(dst, actual_src);
      }
    }
  }

  // ================================================================
  // Generic AAPCS-driven FFI call.
  //
  // Consumes an `AbiCall` produced by `abi::classify` and
  // emits the entire marshaling sequence. Drives codegen
  // from the `pub ffi` declaration's type signature
  // instead of a per-symbol handler.
  //
  // Ordering rationale:
  //   1. Stack reservation (if any indirect args / return
  //      slot).
  //   2. HFA / Composite loads first — these READ from
  //      struct-base GP regs but WRITE to FP regs and
  //      composite-arg GP regs. Doing them first means a
  //      later step that overwrites the struct-base reg
  //      can't trip on a still-pending load.
  //   3. Clobber-safe GP moves for plain `Gp` args —
  //      `emit_safe_int_arg_moves` handles cross-class
  //      cycles via the X16 scratch.
  //   4. FCVT-narrow per `Fp { narrow: true }` arg. Each
  //      narrow is in-place (S writes the low half of V),
  //      so this never clobbers a sibling FP arg.
  //   5. `BL c_sym`.
  //   6. Return-value placement.
  //   7. Stack restore.
  // ================================================================

  /// Emit a C call described by `abi`. `args` aligns
  /// 1:1 with `abi.args`; `idx` is the SIR instruction
  /// index, used to look up the destination register
  /// via `reg_for_insn` / `fp_reg_for_insn`.
  fn emit_ffi_call(
    &mut self,
    c_sym: &str,
    abi: &crate::abi::AbiCall,
    args: &[ValueId],
    idx: usize,
    all_insns: &[Insn],
  ) {
    use crate::abi::{AbiArg, AbiRet};

    if abi.stack_bytes > 0 {
      self.emitter.emit_sub_imm(SP, SP, abi.stack_bytes as u16);
    }

    let mut gp_moves: Vec<(Register, Register)> = Vec::new();
    let mut fp_moves: Vec<(FpRegister, FpRegister)> = Vec::new();
    // Composite-arg loads have to run AFTER `gp_moves`
    // (`v_base` is queued there for safe relocation to the
    // arg's first dst-reg). Each tuple records the dst
    // registers and the field count so the post-move pass
    // emits the right `ldr` sequence in reverse field order.
    let mut composite_loads: Vec<Vec<Register>> = Vec::new();
    // Scalar FP args whose source value isn't in an FP
    // register: materialized from memory AFTER the
    // register-resident FP moves (Step 3b) so a still-live
    // FP-move source can't be clobbered by a materialize
    // that writes the same physical register first.
    let mut fp_materializes: Vec<(FpRegister, ValueId)> = Vec::new();

    for (i, abi_arg) in abi.args.iter().enumerate() {
      let arg_value = args[i];

      match abi_arg {
        AbiArg::Gp(dst_reg) => {
          let src = self.alloc_reg(arg_value).unwrap_or(*dst_reg);
          gp_moves.push((*dst_reg, src));
        }

        AbiArg::Fp { reg: dst_reg, .. } => {
          // When the value is already in an FP register, queue
          // a clobber-safe move. Otherwise defer a load from
          // its stable home (constant pool, struct base, spill
          // / param slot) to run AFTER the register-resident
          // moves — the old no-op `(dst, dst)` fallback left
          // the arg register holding garbage.
          if let Some(src) = self.alloc_fp_reg(arg_value) {
            fp_moves.push((*dst_reg, src));
          } else {
            fp_materializes.push((*dst_reg, arg_value));
          }
        }

        AbiArg::Hfa { regs, .. } => {
          // zo stores struct fields at successive
          // 8-byte slots. The argument's source register
          // holds the struct's base address.
          let v_base = self.alloc_reg(arg_value).unwrap_or(X0);
          let narrow = matches!(
            abi_arg,
            AbiArg::Hfa {
              width: zo_ty::FloatWidth::F32,
              ..
            }
          );
          for (j, reg) in regs.iter().enumerate() {
            self.emitter.emit_ldr_fp(
              *reg,
              v_base,
              (j as u32 * STACK_SLOT_SIZE) as u16,
            );
            if narrow {
              self.emitter.emit_fcvt_d_to_s(*reg, *reg);
            }
          }
        }

        AbiArg::Composite { regs, .. } => {
          // Route the struct base through the safe-move
          // pass: queue `v_base → regs[0]` so any cross-arg
          // register conflict (regalloc put one arg's
          // v_base in another arg's dst-reg) gets resolved
          // by `emit_safe_int_arg_moves`. The actual field
          // loads then run AFTER the moves with `regs[0]`
          // as the base; emitting them in reverse field
          // order so the final `ldr regs[0], [regs[0], 0]`
          // safely overwrites the base after every other
          // field has been read.
          let v_base = self.alloc_reg(arg_value).unwrap_or(regs[0]);
          gp_moves.push((regs[0], v_base));
          composite_loads.push(regs.clone());
        }

        AbiArg::Indirect { .. } => {
          // > 16B composite — caller memcpys onto the
          // reserved stack slot and passes a pointer to
          // it. Not exercised by any current FFI; will
          // implement when a `pub ffi` introduces a
          // `Camera3D`-by-value parameter.
          todo!(
            "AAPCS Indirect arg marshaling — \
             needed when an FFI declares a > 16B \
             struct param (e.g. raylib Camera3D)"
          );
        }

        AbiArg::Stack { .. } => {
          // > 8 same-class args — overflows to stack.
          // No current FFI hits this.
          todo!(
            "AAPCS Stack arg marshaling — \
             needed when an FFI has > 8 GP or > 8 FP args"
          );
        }
      }
    }

    // Step 3: clobber-safe GP moves all at once.
    if !gp_moves.is_empty() {
      self.emit_safe_int_arg_moves(&gp_moves);
    }

    // Step 3a: composite-arg field loads. `regs[0]` now
    // holds `v_base` (placed there by the safe-move pass).
    // Load fields in reverse order so the final write to
    // `regs[0]` happens last, after every other field has
    // been read.
    for regs in &composite_loads {
      for j in (0..regs.len()).rev() {
        self.emitter.emit_ldr(regs[j], regs[0], (j as i16) * 8);
      }
    }

    // Step 3b: clobber-safe FP moves all at once.
    if !fp_moves.is_empty() {
      self.emit_safe_fp_arg_moves(&fp_moves);
    }

    // Step 3c: memory-sourced FP args. Runs after the
    // register moves so a still-live FP-move source can't be
    // overwritten by a load into the same physical register.
    for (dst_fp, value) in std::mem::take(&mut fp_materializes) {
      self.materialize_fp_value_into(dst_fp, value, all_insns);
    }

    // Step 4: per-arg post-marshaling narrow.
    for abi_arg in abi.args.iter() {
      if let AbiArg::Fp { reg, narrow: true } = abi_arg {
        self.emitter.emit_fcvt_d_to_s(*reg, *reg);
      }
    }

    // Step 5 + 6: call + return. Composite returns share
    // their result with the caller-save GP set (X0/X1), so
    // the lift must run BETWEEN `BL` and the GP reload — a
    // monolithic `emit_extern_call` would clobber X1 before
    // we could read it.
    let composite_ret = matches!(abi.ret, AbiRet::Composite { .. });

    if composite_ret {
      self.emit_caller_save_spill();
      self.emit_extern_bl(c_sym);
    } else {
      self.emit_extern_call(c_sym);
    }

    match &abi.ret {
      AbiRet::Void => {}

      AbiRet::Gp { reg, sign_extend } => {
        if *sign_extend {
          self.emitter.emit_sxtw(*reg, *reg);
        }

        if let Some(dst) = self.reg_for_insn(idx)
          && dst != *reg
        {
          self.emitter.emit_mov_reg(dst, *reg);
        }
      }

      AbiRet::Fp { reg, widen } => {
        if *widen {
          self.emitter.emit_fcvt_s_to_d(*reg, *reg);
        }
        if let Some(dst) = self.fp_reg_for_insn(idx)
          && dst != *reg
        {
          self.emitter.emit_fmov_fp(dst, *reg);
        }
      }

      AbiRet::Hfa { regs, width } => {
        // raylib returns an HFA in the same regs the call
        // sites uses to PASS one. Widen back to zo's f64
        // (in place) and stash into a fresh struct slot.
        let widen = matches!(width, zo_ty::FloatWidth::F32);
        let base = self.struct_base + self.next_struct_slot;

        for (i, reg) in regs.iter().enumerate() {
          if widen {
            self.emitter.emit_fcvt_s_to_d(*reg, *reg);
          }
          self.emit_str_fp_sp(*reg, base + i as u32 * STACK_SLOT_SIZE);
        }
        self.next_struct_slot += regs.len() as u32 * STACK_SLOT_SIZE;

        if let Some(dst) = self.reg_for_insn(idx) {
          self.emit_add_sp_offset(dst, base);
        }
      }

      AbiRet::Composite { regs, .. } => {
        // Inverse of `AbiArg::Composite` marshal: each
        // successive GP return register holds an 8-byte
        // field; stash into a fresh struct slot before the
        // caller-save reload clobbers them.
        let base = self.struct_base + self.next_struct_slot;

        for (i, reg) in regs.iter().enumerate() {
          self.emit_str_sp(*reg, base + i as u32 * STACK_SLOT_SIZE);
        }
        self.next_struct_slot += regs.len() as u32 * STACK_SLOT_SIZE;

        self.emit_caller_save_reload();

        if let Some(dst) = self.reg_for_insn(idx) {
          self.emit_add_sp_offset(dst, base);
        }
      }

      AbiRet::Indirect { .. } => {
        // > 16B return — X8 was set before the call.
        // The slot is already at `slot_offset` on the
        // current stack frame. Not exercised today.
        todo!(
          "AAPCS Indirect return — \
           needed when an FFI returns a > 16B struct"
        );
      }
    }

    // Step 7: restore stack.
    if abi.stack_bytes > 0 {
      self.emitter.emit_add_imm(SP, SP, abi.stack_bytes as u16);
    }
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
    // The shared buffer must be reserved before the
    // Result frame so they never alias when this is the
    // first IO read in the function.
    let buf_off = self.allocate_io_shared_buf();
    let result_base = self.struct_base + self.next_struct_slot;

    // Per-call frame (relative to result_base):
    //   [+0]  Result tag
    //   [+8]  Result field (heap str ptr or errno)
    //   [+16] scratch (saved bytes_read)
    //
    // Read buffer at `buf_off` is shared across every
    // IO read in this function.
    let scratch_off = result_base + 2 * STACK_SLOT_SIZE;

    // --- open ---
    self.emitter.emit_add_imm(X0, path, 8);
    self.emitter.emit_mov_imm(X1, O_READ_ONLY);
    self.emitter.emit_mov_imm(X2, 0);
    self.emitter.emit_mov_imm(X16, SYS_OPEN);
    self.emitter.emit_svc(0);

    let open_err = self.emitter.forward_bcs();

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

    self.finalize_io_result_str(idx, result_base, buf_off, open_err, false);
  }

  /// `args() -> []str` — call `_zo_args` which builds the
  /// array on the heap and returns its base pointer in X0.
  /// Codegen treats the return value like any other heap
  /// `[]str`; no on-stack scratch frame.
  fn emit_io_args(&mut self, idx: usize) {
    self.emit_extern_call("_zo_args");

    if let Some(dst) = self.reg_for_insn(idx) {
      self.emitter.emit_mov_reg(dst, X0);
    }
  }

  /// `remove_file(path: str) -> bool` via inline SYS_unlink.
  ///
  /// @note — `path + 8` skips the zo str header so the
  /// kernel sees the NUL-terminated payload directly.
  fn emit_io_remove(&mut self, args: &[ValueId], idx: usize) {
    let path = args.first().and_then(|v| self.alloc_reg(*v)).unwrap_or(X0);

    self.emitter.emit_add_imm(X0, path, 8);
    self.emitter.emit_mov_imm(X16, SYS_UNLINK);
    self.emitter.emit_svc(0);

    if let Some(dst) = self.reg_for_insn(idx) {
      self.emitter.emit_cmp_imm(X0, 0);
      self.emitter.emit_cset(dst, COND_EQ);
    }
  }

  /// `read_dir(path: str) -> []str` via `_zo_io_read_dir`.
  ///
  /// @note — `path + 8` skips the zo str header so the
  /// runtime receives the NUL-terminated payload directly.
  fn emit_io_read_dir(&mut self, args: &[ValueId], idx: usize) {
    let path = args.first().and_then(|v| self.alloc_reg(*v)).unwrap_or(X0);

    self.emitter.emit_add_imm(X0, path, 8);
    self.emit_extern_call("_zo_io_read_dir");

    if let Some(dst) = self.reg_for_insn(idx) {
      self.emitter.emit_mov_reg(dst, X0);
    }
  }

  /// `readln() -> Result<str, int>` and `read() -> Result<str,
  /// int>` lower through one runtime helper each
  /// (`_zo_io_readln` / `_zo_io_read`), which returns the byte
  /// count (`>= 0`) or `-errno` (`< 0`) in `X0`.
  fn emit_io_read_stdin(&mut self, idx: usize, helper_sym: &str) {
    let buf_off = self.allocate_io_shared_buf();
    let result_base = self.struct_base + self.next_struct_slot;
    let scratch_off = result_base + 2 * STACK_SLOT_SIZE;

    self.emit_add_sp_offset(X0, buf_off);
    self.emit_mov_imm_64(X1, READ_FILE_BUF_SIZE as u64);
    self.emit_extern_call(helper_sym);
    self.emit_str_sp(X0, scratch_off);

    self.emitter.emit_cmp_imm(X0, 0);

    let err_branch = self.emitter.forward_blt();

    self.emit_ldr_sp(X2, scratch_off);

    self.finalize_io_result_str(idx, result_base, buf_off, err_branch, true);
  }

  /// Reserve the shared 4096-byte IO read buffer if this
  /// function hasn't allocated one yet, returning its
  /// SP-relative offset. Subsequent calls return the
  /// memoized offset.
  ///
  /// The buffer is sized to cover the longest single
  /// `read()` syscall response; the str payload is
  /// heap-copied via `_zo_str_alloc` so the buffer can
  /// be safely reused by the next IO call.
  fn allocate_io_shared_buf(&mut self) -> u32 {
    if let Some(off) = self.io_shared_buf_offset {
      return off;
    }

    let off = self.struct_base + self.next_struct_slot;

    self.next_struct_slot += IO_SHARED_BUF_SLOTS * STACK_SLOT_SIZE;
    self.io_shared_buf_offset = Some(off);

    off
  }

  /// Build `Result<str, int>` on the stack from a length in
  /// `X2` (already loaded) and an errno path branched to from
  /// `err_branch_pos`. Shared by `emit_io_read_file` and
  /// `emit_io_read_stdin` — same layout, same Ok/Err merge,
  /// same `next_struct_slot` accounting.
  ///
  /// `negate_errno = true` rebuilds errno as `-X0` from the
  /// scratch slot (stdin helpers return `-errno` as a signed
  /// int); `false` uses `X0` directly (the syscall path).
  fn finalize_io_result_str(
    &mut self,
    idx: usize,
    result_base: u32,
    buf_off: u32,
    err_branch: PatchSite,
    negate_errno: bool,
  ) {
    // Ok path: heap-copy the buffer payload via
    // `_zo_str_alloc(buf, n)` so the next IO call can
    // overwrite the shared buffer without aliasing this
    // result's str.
    self.emit_add_sp_offset(X0, buf_off);
    self.emitter.emit_mov_reg(X1, X2);
    self.emit_extern_call("_zo_str_alloc");

    self.emit_str_sp(XZR, result_base);
    self.emit_str_sp(X0, result_base + STACK_SLOT_SIZE);

    let ok_done = self.emitter.forward_b();

    self.emitter.bind_here(err_branch);

    self.emitter.emit_mov_imm(X16, 1);
    self.emit_str_sp(X16, result_base);

    if negate_errno {
      let scratch_off = result_base + 2 * STACK_SLOT_SIZE;

      self.emit_ldr_sp(X0, scratch_off);
      self.emitter.emit_sub(X0, XZR, X0);
    }

    self.emit_str_sp(X0, result_base + STACK_SLOT_SIZE);

    self.emitter.bind_here(ok_done);

    if let Some(dst) = self.reg_for_insn(idx) {
      self.emit_add_sp_offset(dst, result_base);
    }

    self.next_struct_slot += IO_RESULT_FRAME_SLOTS * STACK_SLOT_SIZE;
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
    let ok_label = 0x80000000 | self.emitter.current_offset();

    self
      .branch_fixups
      .push((self.emitter.current_offset(), ok_label));

    self.emitter.emit_cbnz(X0, 0);

    // Fail path: panic via runtime. `catch_unwind` in
    // `task_shim` captures the panic so a failed check
    // inside a green task doesn't kill the process.
    self.emit_extern_call("_zo_check_fail");

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
  fn emit_map_new(&mut self, args: &[ValueId], idx: usize) {
    // Allocate the struct (one ptr field).
    let struct_base = self.struct_base + self.next_struct_slot;

    self.next_struct_slot += STACK_SLOT_SIZE;

    // The executor prepends three `ConstInt`s carrying
    // `(key_kind, key_sz, val_sz)` derived from the
    // binding's `HashMap<K, V>` annotation. Move them into
    // X0/X1/X2; default to the legacy MVP triple
    // `(0, 4, 4)` for any program that compiles without
    // those args.
    let kk = args.first().and_then(|v| self.alloc_reg(*v));
    let ks = args.get(1).and_then(|v| self.alloc_reg(*v));
    let vs = args.get(2).and_then(|v| self.alloc_reg(*v));

    if let Some(r) = kk {
      if r != X0 {
        self.emitter.emit_mov_reg(X0, r);
      }
    } else {
      self.emitter.emit_mov_imm(X0, 0);
    }

    if let Some(r) = ks {
      if r != X1 {
        self.emitter.emit_mov_reg(X1, r);
      }
    } else {
      self.emitter.emit_mov_imm(X1, 4);
    }

    if let Some(r) = vs {
      if r != X2 {
        self.emitter.emit_mov_reg(X2, r);
      }
    } else {
      self.emitter.emit_mov_imm(X2, 4);
    }

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

  /// `zo_map_len_raw(ptr)` — pass-through to
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

  /// `zo_str_replace(src, needle, with) -> str` — direct
  /// pass-through to the runtime helper. Three pointer args
  /// in X0..X2; result pointer in X0.
  fn emit_str_replace_raw(&mut self, args: &[ValueId], idx: usize) {
    let arg_regs = [X0, X1, X2];

    for (i, dst) in arg_regs.iter().enumerate() {
      if let Some(src) = args.get(i).and_then(|v| self.alloc_reg(*v))
        && src != *dst
      {
        self.emitter.emit_mov_reg(*dst, src);
      }
    }

    self.emit_extern_call("_zo_str_replace");

    if let Some(dst) = self.reg_for_insn(idx)
      && dst != X0
    {
      self.emitter.emit_mov_reg(dst, X0);
    }
  }

  /// `zo_map_free_raw(ptr)` — pass-through to
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
  fn emit_vec_new(&mut self, args: &[ValueId], idx: usize) {
    let struct_base = self.struct_base + self.next_struct_slot;

    self.next_struct_slot += STACK_SLOT_SIZE;

    // Executor-injected `(elem_kind, elem_sz, _val_sz)`
    // triple. Vec ignores `val_sz` (it's always passed as
    // `0`) but accepts the third arg for ABI symmetry with
    // HashMap/HashSet's prepend.
    let ek = args.first().and_then(|v| self.alloc_reg(*v));
    let es = args.get(1).and_then(|v| self.alloc_reg(*v));

    if let Some(r) = ek {
      if r != X0 {
        self.emitter.emit_mov_reg(X0, r);
      }
    } else {
      self.emitter.emit_mov_imm(X0, 0);
    }

    if let Some(r) = es {
      if r != X1 {
        self.emitter.emit_mov_reg(X1, r);
      }
    } else {
      self.emitter.emit_mov_imm(X1, 8);
    }

    self.emitter.emit_mov_imm(X2, 8);
    self.emit_extern_call("_zo_vec_new");

    self.emit_str_sp(X0, struct_base);

    if let Some(dst) = self.reg_for_insn(idx) {
      self.emit_add_sp_offset(dst, struct_base);
    }
  }

  /// Number of top-level fields of a struct type — the slot
  /// width of its live outer layout. `0` when `ty` isn't a
  /// struct. Allocation-free; prefer over
  /// `struct_field_tys(..).len()` when only the count is
  /// needed.
  fn struct_field_count(&self, ty: TyId) -> u32 {
    self
      .type_view
      .and_then(|view| {
        let Ty::Struct(sid) = resolve_ty(view.tys, ty) else {
          return None;
        };
        let st = view.ty_table.struct_ty(sid)?;

        Some(view.ty_table.struct_fields(st).len() as u32)
      })
      .unwrap_or(0)
  }

  /// Top-level field types of a struct type, in
  /// declaration order. Empty when `ty` isn't a struct or
  /// the type view is unavailable — callers treat an empty
  /// list as "not a struct element" and stay on the scalar
  /// path.
  fn struct_field_tys(&self, ty: TyId) -> Vec<TyId> {
    self
      .type_view
      .and_then(|view| {
        let Ty::Struct(sid) = resolve_ty(view.tys, ty) else {
          return None;
        };
        let st = view.ty_table.struct_ty(sid)?;

        Some(
          view
            .ty_table
            .struct_fields(st)
            .iter()
            .map(|f| f.ty_id)
            .collect(),
        )
      })
      .unwrap_or_default()
  }

  /// Serialized leaf-word count of a vec element type — the
  /// number of 8-byte words the runtime stores per slot.
  /// Mirrors the executor's `Vec::new` `elem_sz`
  /// computation (both route through `struct_leaf_words`).
  fn vec_elem_leaf_words(&self, ty: TyId) -> u32 {
    self
      .type_view
      .map(|view| struct_leaf_words(ty, view.tys, view.ty_table))
      .unwrap_or(1)
      .max(1)
  }

  /// Concrete element type of a vec read, from the executor's
  /// `vec_elem_tys` record. The generic `Option<$T>` return
  /// carries none, so struct elements need the side channel;
  /// scalars fall back to the `Option` payload.
  fn vec_read_elem_ty(&self, call_dst: ValueId, option_ty: TyId) -> TyId {
    self
      .vec_elem_tys
      .get(&call_dst.0)
      .copied()
      .unwrap_or_else(|| self.vec_elem_of_option(option_ty))
  }

  /// Payload type of the `Some` variant of an `Option<T>`;
  /// `TyId(0)` (scalar path) when unresolvable.
  fn vec_elem_of_option(&self, option_ty: TyId) -> TyId {
    self
      .type_view
      .and_then(|view| {
        let Ty::Enum(eid) = resolve_ty(view.tys, option_ty) else {
          return None;
        };
        let e = view.ty_table.enum_ty(eid)?;

        view
          .ty_table
          .enum_variants(e)
          .iter()
          .find_map(|v| view.ty_table.variant_fields(v).first().copied())
      })
      .unwrap_or(TyId(0))
  }

  /// Serialize the live struct at `[X17]` into a flat leaf
  /// buffer at `SP + dst_off`; returns the end offset. `X16`
  /// is transfer scratch.
  ///
  /// A nested walk reloads `X17`, so each recursing level
  /// first saves its base to a stack slot — keeps the walk
  /// correct at any nesting depth.
  fn emit_vec_flatten_struct(&mut self, ty: TyId, dst_off: u32) -> u32 {
    let field_tys = self.struct_field_tys(ty);
    let mut cursor = dst_off;
    let mut save_slot: Option<u32> = None;

    for (i, field_ty) in field_tys.iter().enumerate() {
      let off_i = (i as u32 * STACK_SLOT_SIZE) as i16;

      if self.is_struct_ty(*field_ty) {
        if save_slot.is_none() {
          save_slot = Some(self.struct_base + self.next_struct_slot);
          self.next_struct_slot += STACK_SLOT_SIZE;
        }

        let slot = save_slot.unwrap();

        self.emit_str_sp(X17, slot);
        self.emitter.emit_ldr(X17, X17, off_i);
        cursor = self.emit_vec_flatten_struct(*field_ty, cursor);
        self.emit_ldr_sp(X17, slot);
      } else {
        self.emitter.emit_ldr(X16, X17, off_i);
        self.emit_str_sp(X16, cursor);
        cursor += STACK_SLOT_SIZE;
      }
    }

    cursor
  }

  /// Rebuild the live pointer layout of `ty` at
  /// `SP + out_base` from the flat buffer at `SP + flat_off`
  /// (leaf order matches `emit_vec_flatten_struct`).
  /// `inner_cursor` is the next free slot for nested blocks;
  /// returns it advanced. The flat source is contiguous and
  /// statically shaped, so every address is an SP offset —
  /// no base register or pointer-following. `X16` scratch.
  fn emit_vec_rematerialize_struct(
    &mut self,
    ty: TyId,
    flat_off: u32,
    out_base: u32,
    mut inner_cursor: u32,
  ) -> u32 {
    let field_tys = self.struct_field_tys(ty);
    let mut flat_cursor = flat_off;

    for (i, field_ty) in field_tys.iter().enumerate() {
      let out_slot = out_base + i as u32 * STACK_SLOT_SIZE;

      if self.is_struct_ty(*field_ty) {
        let inner_block = inner_cursor;
        let inner_outer = self.struct_field_count(*field_ty);

        inner_cursor += inner_outer * STACK_SLOT_SIZE;
        inner_cursor = self.emit_vec_rematerialize_struct(
          *field_ty,
          flat_cursor,
          inner_block,
          inner_cursor,
        );
        flat_cursor += self.vec_elem_leaf_words(*field_ty) * STACK_SLOT_SIZE;

        self.emit_add_sp_offset(X16, inner_block);
        self.emit_str_sp(X16, out_slot);
      } else {
        self.emit_ldr_sp(X16, flat_cursor);
        self.emit_str_sp(X16, out_slot);
        flat_cursor += STACK_SLOT_SIZE;
      }
    }

    inner_cursor
  }

  /// Build the `Option<T>` aggregate at `SP + opt_base` after
  /// a vec read: success bool in `X0`, value bytes at
  /// `SP + val_off`. A struct payload is re-materialized at
  /// `SP + live_base` and stored as a pointer — the by-pointer
  /// convention `EnumConstruct` uses for struct payloads; a
  /// scalar payload is stored inline.
  fn emit_vec_option_tail(
    &mut self,
    elem_ty: TyId,
    val_off: u32,
    opt_base: u32,
    live_base: u32,
    idx: usize,
  ) {
    self.emitter.emit_mov_imm(X16, 1);
    self.emitter.emit_eor(X16, X16, X0);
    self.emit_str_sp(X16, opt_base);

    if self.is_struct_ty(elem_ty) {
      let outer = self.struct_field_count(elem_ty);
      let inner_cursor = live_base + outer * STACK_SLOT_SIZE;

      self.emit_vec_rematerialize_struct(
        elem_ty,
        val_off,
        live_base,
        inner_cursor,
      );
      self.emit_add_sp_offset(X16, live_base);
      self.emit_str_sp(X16, opt_base + STACK_SLOT_SIZE);
    } else {
      self.emit_ldr_sp(X16, val_off);
      self.emit_str_sp(X16, opt_base + STACK_SLOT_SIZE);
    }

    if let Some(dst) = self.reg_for_insn(idx) {
      self.emit_add_sp_offset(dst, opt_base);
    }
  }

  /// Reserve the scratch a vec read needs: the flat value
  /// buffer (`elem` leaf words), the 2-word `Option`
  /// aggregate, and — for a struct element — the live
  /// layout the payload pointer targets. Zeroes the flat
  /// buffer so a miss leaves a deterministic payload.
  /// Returns `(val_off, opt_base, live_base)`.
  fn reserve_vec_read_scratch(&mut self, elem_ty: TyId) -> (u32, u32, u32) {
    let is_struct = self.is_struct_ty(elem_ty);
    let val_words = if is_struct {
      self.vec_elem_leaf_words(elem_ty)
    } else {
      1
    };

    let val_off = self.struct_base + self.next_struct_slot;
    self.next_struct_slot += val_words * STACK_SLOT_SIZE;

    let opt_base = self.struct_base + self.next_struct_slot;
    self.next_struct_slot += 2 * STACK_SLOT_SIZE;

    let live_base = self.struct_base + self.next_struct_slot;

    if is_struct {
      let live_words = self
        .type_view
        .and_then(|view| flat_struct_slots_of(elem_ty, view.tys, view.ty_table))
        .unwrap_or(val_words);

      self.next_struct_slot += live_words * STACK_SLOT_SIZE;
    }

    for j in 0..val_words {
      self.emit_str_sp(XZR, val_off + j * STACK_SLOT_SIZE);
    }

    (val_off, opt_base, live_base)
  }

  /// `v.push(value)` — pass the element bytes to
  /// `_zo_vec_push`. A struct is flattened so the runtime
  /// stores no stack pointers; a scalar spills one word.
  fn emit_vec_push(&mut self, args: &[ValueId], _idx: usize, elem_ty: TyId) {
    let recv = args.first().and_then(|v| self.alloc_reg(*v)).unwrap_or(X0);
    let v = args.get(1).and_then(|v| self.alloc_reg(*v)).unwrap_or(X1);

    let val_off = if self.is_struct_ty(elem_ty) {
      let words = self.vec_elem_leaf_words(elem_ty);
      let flat_base = self.struct_base + self.next_struct_slot;

      self.next_struct_slot += words * STACK_SLOT_SIZE;

      if v != X17 {
        self.emitter.emit_mov_reg(X17, v);
      }

      self.emit_vec_flatten_struct(elem_ty, flat_base);

      flat_base
    } else {
      let v_off = self.struct_base + self.next_struct_slot;

      self.next_struct_slot += STACK_SLOT_SIZE;
      self.emit_str_sp(v, v_off);

      v_off
    };

    self.emitter.emit_ldr(X0, recv, 0);
    self.emit_add_sp_offset(X1, val_off);
    self.emit_extern_call("_zo_vec_push");
  }

  /// `v.pop()` — call `_zo_vec_pop` into a flat scratch,
  /// then build the `Option<T>` aggregate.
  fn emit_vec_pop(&mut self, args: &[ValueId], idx: usize, elem_ty: TyId) {
    let recv = args.first().and_then(|v| self.alloc_reg(*v)).unwrap_or(X0);

    let (val_off, opt_base, live_base) = self.reserve_vec_read_scratch(elem_ty);

    self.emitter.emit_ldr(X0, recv, 0);
    self.emit_add_sp_offset(X1, val_off);
    self.emit_extern_call("_zo_vec_pop");

    self.emit_vec_option_tail(elem_ty, val_off, opt_base, live_base, idx);
  }

  /// Lower a `Vec<T>` apply-method that follows the
  /// runtime's `(ptr, idx, val_out_ptr) -> bool` ABI and
  /// reports its result as `Option<T>`. Today: `Vec::get`
  /// (read-only) and `Vec::remove` (read + shift-down +
  /// `len--`). From codegen's POV the contract is
  /// identical — only the runtime entry point and its
  /// side effects on the vec differ.
  fn emit_vec_option_idx_call(
    &mut self,
    args: &[ValueId],
    idx: usize,
    runtime_call: &str,
    elem_ty: TyId,
  ) {
    let recv = args.first().and_then(|v| self.alloc_reg(*v)).unwrap_or(X0);
    let i = args.get(1).and_then(|v| self.alloc_reg(*v)).unwrap_or(X1);

    let (val_off, opt_base, live_base) = self.reserve_vec_read_scratch(elem_ty);

    self.emitter.emit_ldr(X0, recv, 0);

    if i != X1 {
      self.emitter.emit_mov_reg(X1, i);
    }

    self.emit_add_sp_offset(X2, val_off);
    self.emit_extern_call(runtime_call);

    self.emit_vec_option_tail(elem_ty, val_off, opt_base, live_base, idx);
  }

  /// `v.get(idx)` — read-only lookup, returns `Option<T>`.
  fn emit_vec_get(&mut self, args: &[ValueId], idx: usize, elem_ty: TyId) {
    self.emit_vec_option_idx_call(args, idx, "_zo_vec_get", elem_ty);
  }

  /// `v.remove(idx)` — same shape as `get` plus the
  /// runtime shifts the tail down by one and decrements
  /// `len`.
  fn emit_vec_remove(&mut self, args: &[ValueId], idx: usize, elem_ty: TyId) {
    self.emit_vec_option_idx_call(args, idx, "_zo_vec_remove", elem_ty);
  }

  /// `v.set(idx, value)` — hand the element bytes to
  /// `_zo_vec_set` as `(ptr, idx, &val)`. Scalars spill one
  /// word; structs flatten like `push`. Returns the
  /// runtime's `bool` (true on hit, false on OOB).
  fn emit_vec_set(&mut self, args: &[ValueId], idx: usize, elem_ty: TyId) {
    let recv = args.first().and_then(|v| self.alloc_reg(*v)).unwrap_or(X0);
    let i = args.get(1).and_then(|v| self.alloc_reg(*v)).unwrap_or(X1);
    let v = args.get(2).and_then(|v| self.alloc_reg(*v)).unwrap_or(X2);

    let val_off = if self.is_struct_ty(elem_ty) {
      let words = self.vec_elem_leaf_words(elem_ty);
      let flat_base = self.struct_base + self.next_struct_slot;

      self.next_struct_slot += words * STACK_SLOT_SIZE;

      if v != X17 {
        self.emitter.emit_mov_reg(X17, v);
      }

      self.emit_vec_flatten_struct(elem_ty, flat_base);

      flat_base
    } else {
      let v_off = self.struct_base + self.next_struct_slot;

      self.next_struct_slot += STACK_SLOT_SIZE;
      self.emit_str_sp(v, v_off);

      v_off
    };

    self.emitter.emit_ldr(X0, recv, 0);

    if i != X1 {
      self.emitter.emit_mov_reg(X1, i);
    }

    self.emit_add_sp_offset(X2, val_off);
    self.emit_extern_call("_zo_vec_set");

    if let Some(dst) = self.reg_for_insn(idx)
      && dst != X0
    {
      self.emitter.emit_mov_reg(dst, X0);
    }
  }

  /// `zo_vec_len_raw(ptr)` — pass-through to
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

  /// `zo_vec_free_raw(ptr)` — pass-through to
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
  fn emit_set_new(&mut self, args: &[ValueId], idx: usize) {
    let struct_base = self.struct_base + self.next_struct_slot;

    self.next_struct_slot += STACK_SLOT_SIZE;

    // Executor-injected `(key_kind, key_sz, val_sz)` —
    // HashSet always passes `val_sz = 0` so the runtime's
    // value-byte path is a no-op. The first two come from
    // the binding's `HashSet<K>` annotation.
    let kk = args.first().and_then(|v| self.alloc_reg(*v));
    let ks = args.get(1).and_then(|v| self.alloc_reg(*v));
    let vs = args.get(2).and_then(|v| self.alloc_reg(*v));

    if let Some(r) = kk {
      if r != X0 {
        self.emitter.emit_mov_reg(X0, r);
      }
    } else {
      self.emitter.emit_mov_imm(X0, 0);
    }

    if let Some(r) = ks {
      if r != X1 {
        self.emitter.emit_mov_reg(X1, r);
      }
    } else {
      self.emitter.emit_mov_imm(X1, 4);
    }

    if let Some(r) = vs {
      if r != X2 {
        self.emitter.emit_mov_reg(X2, r);
      }
    } else {
      self.emitter.emit_mov_imm(X2, 0);
    }

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

  /// `zo_set_len_raw` and `zo_set_free_raw` route
  /// directly to the shared map exports — sets reuse
  /// the map allocator wholesale.
  fn emit_set_len_raw(&mut self, args: &[ValueId], idx: usize) {
    self.emit_map_len_raw(args, idx);
  }

  fn emit_set_free_raw(&mut self, args: &[ValueId], idx: usize) {
    self.emit_map_free_raw(args, idx);
  }

  /// `arr.sort()` for `[]int` — in-place ascending sort
  /// via `_zo_arr_sort_i32`. Array layout is
  /// `[len:8][cap:8][i32 data...]`; pass the data pointer
  /// (`arr + 16`) as `X0` and the length (`*(arr + 0)`)
  /// as `X1`.
  fn emit_arr_sort_int(&mut self, args: &[ValueId], _idx: usize) {
    let recv = args.first().and_then(|v| self.alloc_reg(*v)).unwrap_or(X0);

    // Stash `recv` in X16 first. If the register allocator
    // placed it in X1, the `LDR X1, [recv, #0]` step below
    // would overwrite it before the `ADD X0, recv, #16`
    // could run — sort then gets `X0 = len + 16` (a bogus
    // low address) instead of the data pointer. X16 is the
    // intra-procedure scratch (AAPCS), unreachable by
    // allocator placement.
    self.emitter.emit_mov_reg(X16, recv);
    self.emitter.emit_ldr(X1, X16, 0);
    self.emitter.emit_add_imm(X0, X16, 16u16);

    self.emit_extern_call("_zo_arr_sort_i32");
  }

  /// Marshal a call's argument values into the positional
  /// ABI slots — X0-X7 (GP) / D0-D7 (FP) plus the overflow
  /// stack region for args beyond 8. Returns the aligned
  /// byte count SP was lowered by (0 when ≤8 args); the
  /// caller restores SP by the same amount after the branch.
  ///
  /// Shared by `Insn::Call` (direct `BL`) and
  /// `Insn::CallIndirect` (`BLR x16`). The callee pointer of
  /// an indirect call is loaded into X16 *after* this returns
  /// so the move-scratch use of X16/X17 here can't clobber it.
  fn marshal_user_call_args(&mut self, args: &[ValueId]) -> u32 {
    let stack_arg_count = args.len().saturating_sub(MAX_REG_ARGS);
    let stack_arg_bytes = if stack_arg_count > 0 {
      let bytes = stack_arg_count as u32 * STACK_SLOT_SIZE;
      (bytes + 15) & !15
    } else {
      0
    };

    // Stage overflow arg values into dedicated caller-save
    // area slots (past the X1..X15 region) BEFORE register
    // moves clobber their source registers. The staging slots
    // survive across register moves AND caller-save spill.
    let overflow_staging_base =
      self.caller_save_base + (CALLER_SAVE_COUNT as u32 + 1) * STACK_SLOT_SIZE;

    for (i, arg) in args.iter().enumerate() {
      if i < MAX_REG_ARGS {
        continue;
      }

      if let Some(src_reg) = self.alloc_reg(*arg) {
        let stage_off =
          overflow_staging_base + (i - MAX_REG_ARGS) as u32 * STACK_SLOT_SIZE;
        self.emit_str_sp(src_reg, stage_off);
      }
    }

    // Collect register moves for args 0..7.
    let mut gp_moves: [(Register, Register); MAX_REG_ARGS] =
      [(X0, X0); MAX_REG_ARGS];
    let mut gp_moves_len: usize = 0;

    for (i, arg) in args.iter().enumerate() {
      if i >= MAX_REG_ARGS {
        continue;
      }

      if let Some(fp_src) = self.alloc_fp_reg(*arg) {
        let fp_dst = FpRegister::new(i as u8);

        if fp_src != fp_dst {
          self.emitter.emit_fmov_fp(fp_dst, fp_src);
        }
      } else if let Some(src_reg) = self.alloc_reg(*arg) {
        let dst_reg = Register::new(i as u8);

        if src_reg != dst_reg {
          gp_moves[gp_moves_len] = (dst_reg, src_reg);
          gp_moves_len += 1;
        }
      }
    }

    let gp_moves = &gp_moves[..gp_moves_len];

    // Pre-save: if any move's src is also another move's dst,
    // save the src to a scratch register before any moves
    // happen. X16 and X17 handle up to two simultaneous
    // clobbers.
    let mut saved: [(Register, Register); 2] = [(X0, X0); 2];
    let mut saved_count = 0usize;

    for j in 0..gp_moves.len() {
      let (_, src) = gp_moves[j];

      let is_clobbered = gp_moves
        .iter()
        .enumerate()
        .any(|(k, (dst, _))| k != j && *dst == src);

      if is_clobbered
        && saved_count < 2
        && !saved[..saved_count].iter().any(|(s, _)| *s == src)
      {
        let scratch = if saved_count == 0 { X16 } else { X17 };

        self.emitter.emit_mov_reg(scratch, src);
        saved[saved_count] = (src, scratch);
        saved_count += 1;
      }
    }

    for (dst, src) in gp_moves {
      let actual_src = saved[..saved_count]
        .iter()
        .find(|(s, _)| *s == *src)
        .map(|(_, scratch)| *scratch)
        .unwrap_or(*src);

      self.emitter.emit_mov_reg(*dst, actual_src);
    }

    // No blanket caller-save here. The register allocator
    // already spilled every value live across this call into
    // its own slot via `spill_ops` (emitted by
    // `emit_spills(Before)`) and reloads them after, so the
    // flat X1..X15 store/reload that used to bracket the
    // branch only ever saved dead registers — pure frame
    // bloat that overflowed the stack on deep zo→zo chains.

    // Adjust SP and copy overflow args from staging slots to
    // the stack arg region.
    if stack_arg_bytes > 0 {
      self.emitter.emit_sub_imm(SP, SP, stack_arg_bytes as u16);

      for i in 0..stack_arg_count {
        let stage_off =
          stack_arg_bytes + overflow_staging_base + i as u32 * STACK_SLOT_SIZE;
        let dest_off = i as u32 * STACK_SLOT_SIZE;

        self.emit_ldr_sp(X16, stage_off);
        self.emitter.emit_str(X16, SP, dest_off as i16);
      }
    }

    stack_arg_bytes
  }

  /// Materialize a value into X16 by re-emitting the
  /// producing instruction's load / constant.
  ///
  /// Returns `true` when X16 holds the value and the caller
  /// can `STR X16, [...]`. Returns `false` for computed
  /// values (BinOp, Call, etc.) — caller falls back to the
  /// register allocator's reg.
  ///
  /// Why: above 14 simultaneously-live element values the
  /// allocator's forward-pass spill semantics break down.
  /// The eviction picks a victim that's still consumed at
  /// the aggregate instruction, the victim's register gets
  /// reassigned, and the spill captures a register that the
  /// codegen has already overwritten with someone else's
  /// def. Symptoms: pointer-shaped garbage in the aggregate
  /// slots, sometimes hangs (sort follows on garbage data).
  ///
  /// Bypass: re-emit the def at the consumption point —
  /// constants reload into X16 directly, locals/params
  /// reload from their stable stack slot. Both sources are
  /// stable across register clobbers because they live in
  /// memory or are immediately materializable. Computed
  /// values still need the allocator (rare in aggregate
  /// literals; users typically pre-bind to locals first).
  ///
  /// X16 is the AAPCS intra-procedure scratch — outside the
  /// allocator pool, so the bypass never conflicts with a
  /// live value.
  ///
  /// Used by `ArrayLiteral`, `StructConstruct`,
  /// `TupleLiteral`, and `EnumConstruct` — all four hit the
  /// same regalloc bug at high arity.
  ///
  /// `sp_bias` is added to every SP-relative load this emits.
  /// The frame slot offsets are computed against the function's
  /// resting SP, but `Insn::CallIndirect` with 9+ args lowers
  /// SP by `stack_arg_bytes` (overflow args) BEFORE loading the
  /// callee pointer. Without the bias the load reads
  /// `[lowered_sp + offset]` — the wrong address. Callers whose
  /// SP is at its resting position (array/struct/tuple/enum
  /// element stores) pass `0`.
  fn materialize_value_into_x16(
    &mut self,
    elem: ValueId,
    all_insns: &[Insn],
    sp_bias: u32,
  ) -> bool {
    let Some(InsnIdx(def_pos)) = self.value_def_idx.get(elem) else {
      return false;
    };

    match &all_insns[def_pos as usize] {
      Insn::ConstInt { value, .. } => {
        self.emit_mov_imm_64(X16, *value);

        true
      }
      Insn::ConstFloat { value, .. } => {
        self.emit_mov_imm_64(X16, value.to_bits());

        true
      }
      Insn::ConstBool { value, .. } => {
        self.emit_mov_imm_64(X16, u64::from(*value));

        true
      }
      Insn::Load { src, .. } => match src {
        LoadSource::Local(sym) => {
          let slot = sym.as_u32();

          // Promoted local: its value lives in a callee-saved
          // register, not on the stack. Copy it into X16.
          if let Some(reg) = self.promotion.reg_of(*sym) {
            self.emitter.emit_mov_reg(X16, reg);

            return true;
          }

          if let Some(&offset) = self.mutable_slots.get(&slot) {
            self.emit_ldr_sp(X16, offset + sp_bias);

            return true;
          }

          if let Some(&(offset, _)) = self.param_sym_slots.get(&slot) {
            self.emit_ldr_sp(X16, offset + sp_bias);

            return true;
          }

          false
        }
        LoadSource::Param(pidx) => {
          // Promoted `mut` param: value lives in its
          // callee-saved register, not the home slot.
          if let Some(&reg) = self.param_promo_reg.get(pidx) {
            self.emitter.emit_mov_reg(X16, reg);

            return true;
          }

          if let Some(&offset) = self.param_slots.get(pidx) {
            self.emit_ldr_sp(X16, offset + sp_bias);

            return true;
          }

          false
        }
      },
      _ => false,
    }
  }

  /// Materialize a scalar FP argument value into `dst_fp`
  /// by re-emitting its producing instruction's load /
  /// constant. The FP analog of `materialize_value_into_x16`.
  ///
  /// Returns `true` when `dst_fp` holds the value. Returns
  /// `false` for computed values (BinOp on floats, Call,
  /// etc.) — the caller falls back to the register
  /// allocator's reg.
  ///
  /// Why: the generic AAPCS `AbiArg::Fp` arm assumed the
  /// argument was already resident in an FP register and
  /// fell back to a no-op `(dst, dst)` move when
  /// `alloc_fp_reg` missed it (a liveness / visit_uses gap,
  /// or a value the allocator parked in memory). The dst FP
  /// arg register then held garbage. A struct-field float
  /// passed straight to an FP-arg FFI (`draw_circle(...,
  /// roid.size, ...)`) hit exactly this. Re-emitting the
  /// producer at the call site loads the right bits into the
  /// arg register from a stable source (constant pool, struct
  /// base, or spill / param slot) before the f32 narrow.
  ///
  /// X16 is the AAPCS intra-procedure scratch — outside the
  /// allocator pool — so the const path's `MOVK X16` never
  /// clobbers a live value during call-site marshaling.
  fn materialize_fp_value_into(
    &mut self,
    dst_fp: FpRegister,
    value: ValueId,
    all_insns: &[Insn],
  ) -> bool {
    let Some(InsnIdx(def_pos)) = self.value_def_idx.get(value) else {
      return false;
    };

    match &all_insns[def_pos as usize] {
      Insn::ConstFloat { value, .. } => {
        self.emit_mov_imm_64(X16, value.to_bits());
        self.emitter.emit_fmov_gp_to_fp(dst_fp, X16);

        true
      }
      // Struct / tuple float field — load from base + slot.
      Insn::TupleIndex {
        tuple,
        index,
        ty_id,
        ..
      } if ty_id.0 >= FLOAT_TYPE_ID_MIN && ty_id.0 <= FLOAT_TYPE_ID_MAX => {
        let Some(base) = self.alloc_reg(*tuple) else {
          return false;
        };
        let offset = (*index * STACK_SLOT_SIZE) as u16;

        self.emitter.emit_ldr_fp(dst_fp, base, offset);

        true
      }
      Insn::Load { src, .. } => match src {
        LoadSource::Local(sym) => {
          let slot = sym.as_u32();

          if let Some(&offset) = self.mutable_slots.get(&slot) {
            self.emit_ldr_fp_sp(dst_fp, offset);

            return true;
          }

          if let Some(&(offset, _)) = self.param_sym_slots.get(&slot) {
            self.emit_ldr_fp_sp(dst_fp, offset);

            return true;
          }

          false
        }
        LoadSource::Param(pidx) => {
          if let Some(&offset) = self.param_slots.get(pidx) {
            self.emit_ldr_fp_sp(dst_fp, offset);

            return true;
          }

          false
        }
      },
      _ => false,
    }
  }

  /// Emit `STR <elem>, [r_buf, off]` for one array literal
  /// element on the heap path. See
  /// `materialize_array_elem_into_x16` for the bypass
  /// rationale.
  fn emit_array_element_store(
    &mut self,
    elem: ValueId,
    all_insns: &[Insn],
    r_buf: Register,
    off: u16,
  ) {
    if self.materialize_value_into_x16(elem, all_insns, 0) {
      self.emitter.emit_str(X16, r_buf, off as i16);

      return;
    }

    if let Some(fp) = self.alloc_fp_reg(elem) {
      self.emitter.emit_str_fp(fp, r_buf, off);
    } else if let Some(reg) = self.alloc_reg(elem) {
      self.emitter.emit_str(reg, r_buf, off as i16);
    }
  }

  /// Deep-copy one nested-struct field at a struct-return
  /// call site. `src_ptr_reg` holds the pointer to the
  /// outer callee block; `src_off` is the offset where
  /// that block stores its pointer to the inner struct.
  /// Walks the inner struct's fields, allocating slots in
  /// the caller's struct area starting at `inner_cursor`
  /// (which the caller bumps past the outer slots). Inner
  /// struct fields recurse — same shape, deeper level.
  ///
  /// After the copy, the outer slot at `dst_off` holds a
  /// pointer to the *caller's* inner block, so the
  /// callee-frame pointer never survives the return.
  ///
  /// Returns the updated `inner_cursor` past the inner
  /// block (and any recursive inner-inner blocks).
  /// True when `ty` resolves to `Ty::Struct(_)` against
  /// the wired `type_view`. Used by the call-site +
  /// recursive deep-copy paths to decide which fields
  /// need a recursive copy vs a single pointer-word
  /// store. Returns `false` if `type_view` is unset.
  fn is_struct_ty(&self, ty: TyId) -> bool {
    self
      .type_view
      .is_some_and(|view| matches!(resolve_ty(view.tys, ty), Ty::Struct(_)))
  }

  /// Recursive deep-copy of enum payload struct fields
  /// emitted in the caller's frame immediately after `BL`.
  ///
  /// The callee returns with `X0` pointing at its own
  /// frame's enum slot — only the outer slots
  /// (`discriminant + max(variant.field_count)` words)
  /// were actually constructed there. Any struct-typed
  /// payload lives at a SEPARATE address in the callee
  /// frame, and the payload slot holds only a pointer to
  /// it. That callee frame disappears as soon as the
  /// next stack write happens in the caller, so the
  /// pointer immediately dangles.
  ///
  /// To make `match` patterns on the returned enum read
  /// real bytes, this routine:
  ///
  /// Sequence:
  ///
  /// - Shallow-copy the outer slots from the callee
  ///   frame into the caller's `dst_base` block.
  /// - For every variant whose payload contains struct
  ///   fields, branch on the discriminant and: (a)
  ///   dereference the still-valid callee pointer to
  ///   read the struct bytes; (b) write them into the
  ///   caller's nested-slot scratch area (reserved by
  ///   `flat_struct_slots_of` in the enum branch); (c)
  ///   rewrite the payload slot in the shallow copy to
  ///   point at the caller-side bytes.
  ///
  /// After this, the enum on the caller frame is fully
  /// self-contained — subsequent field reads through
  /// the payload pointer land in valid memory.
  ///
  /// Scratch register convention (mirrors the existing
  /// `emit_deep_copy_struct_field`): `X16` for value
  /// transfer / address materialisation, `X17` for the
  /// callee-side struct pointer. Both are AAPCS-IPC
  /// scratches with no live values across the call.
  fn emit_enum_deep_copy_after_call(
    &mut self,
    layout: &EnumDeepCopyLayout,
    dst_base: u32,
  ) {
    let outer_slots = layout.outer_slots();

    // Step 1: shallow-copy outer enum slots.
    for i in 0..outer_slots {
      let src_off = (i * STACK_SLOT_SIZE) as i16;
      let dst_off = dst_base + i * STACK_SLOT_SIZE;
      self.emitter.emit_ldr(X16, X0, src_off);
      self.emit_str_sp(X16, dst_off);
    }

    // Step 2: per-variant deep-copy + pointer rewrite.
    let view = match self.type_view {
      Some(v) => v,
      None => return,
    };

    let mut nested_cursor = dst_base + outer_slots * STACK_SLOT_SIZE;
    let mut end_fixups: Vec<usize> = Vec::new();

    for variant in &layout.variants {
      let has_struct = variant
        .field_tys
        .iter()
        .any(|f| matches!(resolve_ty(view.tys, *f), Ty::Struct(_)));

      if !has_struct {
        continue;
      }

      // CMP discriminant, BNE skip.
      self.emit_ldr_sp(X16, dst_base);
      self.emitter.emit_cmp_imm(X16, variant.discriminant as u16);

      let bne_pos = self.emitter.current_offset();
      self.emitter.emit_bne(0);

      // Per-field deep-copy.
      for (i, &field_ty) in variant.field_tys.iter().enumerate() {
        if !matches!(resolve_ty(view.tys, field_ty), Ty::Struct(_)) {
          continue;
        }

        let payload_off = dst_base + (i as u32 + 1) * STACK_SLOT_SIZE;
        let struct_slots =
          flat_struct_slots_of(field_ty, view.tys, view.ty_table).unwrap_or(1);

        // X17 = callee-side struct pointer (still
        // dangling but valid until the next caller
        // stack write).
        self.emit_ldr_sp(X17, payload_off);

        // Copy struct bytes word-by-word.
        for j in 0..struct_slots {
          self
            .emitter
            .emit_ldr(X16, X17, (j * STACK_SLOT_SIZE) as i16);
          self.emit_str_sp(X16, nested_cursor + j * STACK_SLOT_SIZE);
        }

        // Rewrite the payload pointer to target the
        // caller-side copy.
        self.emit_add_sp_offset(X16, nested_cursor);
        self.emit_str_sp(X16, payload_off);

        nested_cursor += struct_slots * STACK_SLOT_SIZE;
      }

      // Done with this variant — skip the remaining
      // variant comparisons.
      let b_pos = self.emitter.current_offset();
      self.emitter.emit_b(0);
      end_fixups.push(b_pos as usize);

      // Patch BNE forward to the next variant's
      // comparison (or the end label).
      let after_body = self.emitter.current_offset() as i32;
      self
        .emitter
        .patch_bcond_at(bne_pos as usize, after_body - bne_pos as i32);
    }

    // Patch every variant's B end-fixup to one shared
    // end-of-dispatch label.
    let end_label = self.emitter.current_offset() as i32;
    for pos in end_fixups {
      self.emitter.patch_b_at(pos, end_label - pos as i32);
    }
  }

  fn emit_deep_copy_struct_field(
    &mut self,
    src_ptr_reg: Register,
    src_off: i16,
    dst_off: u32,
    field_ty: TyId,
    inner_cursor: u32,
  ) -> u32 {
    // Three preconditions for the deep copy — `type_view`
    // wired, field resolves to a struct, struct entry
    // exists in the table. Any failure falls back to a
    // single pointer-word copy (still dangling at runtime,
    // but at least the codegen doesn't crash on a partial
    // type table).
    let inner_field_tys: Option<Vec<TyId>> = self.type_view.and_then(|view| {
      let Ty::Struct(sid) = resolve_ty(view.tys, field_ty) else {
        return None;
      };
      let st = view.ty_table.struct_ty(sid)?;

      Some(
        view
          .ty_table
          .struct_fields(st)
          .iter()
          .map(|f| f.ty_id)
          .collect(),
      )
    });

    let Some(inner_field_tys) = inner_field_tys else {
      self.emitter.emit_ldr(X16, src_ptr_reg, src_off);
      self.emit_str_sp(X16, dst_off);
      return inner_cursor;
    };

    let inner_outer_count = inner_field_tys.len() as u32;
    let inner_block_base = inner_cursor;
    // Bump past the outer inner slots so recursive
    // inner-inner copies use the trailing slots.
    let mut next_cursor = inner_cursor + inner_outer_count * STACK_SLOT_SIZE;

    // Stash the inner-src pointer into X9 — it must
    // survive across our scratch-register usage in the
    // copy loop. X9 is caller-saved per AAPCS, no
    // active value lives there at a call return.
    self.emitter.emit_ldr(X9, src_ptr_reg, src_off);

    for (i, inner_ty) in inner_field_tys.iter().enumerate() {
      let inner_src_off = (i as u32 * STACK_SLOT_SIZE) as i16;
      let inner_dst_off = inner_block_base + i as u32 * STACK_SLOT_SIZE;

      if self.is_struct_ty(*inner_ty) {
        next_cursor = self.emit_deep_copy_struct_field(
          X9,
          inner_src_off,
          inner_dst_off,
          *inner_ty,
          next_cursor,
        );
      } else {
        self.emitter.emit_ldr(X16, X9, inner_src_off);
        self.emit_str_sp(X16, inner_dst_off);
      }
    }

    // Write the pointer-to-caller's-inner-block into the
    // outer slot so subsequent loads through that slot
    // land in the caller's frame, not the callee's.
    self.emit_add_sp_offset(X16, inner_block_base);
    self.emit_str_sp(X16, dst_off);

    next_cursor
  }

  /// SP-relative variant of `emit_array_element_store` for
  /// the static `[N]T` path (stack frame).
  fn emit_array_element_store_sp(
    &mut self,
    elem: ValueId,
    all_insns: &[Insn],
    off: u32,
  ) {
    if self.materialize_value_into_x16(elem, all_insns, 0) {
      self.emit_str_sp(X16, off);

      return;
    }

    if let Some(fp) = self.alloc_fp_reg(elem) {
      self.emit_str_fp_sp(fp, off);
    } else if let Some(reg) = self.alloc_reg(elem) {
      self.emit_str_sp(reg, off);
    }
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

  /// Emit a call into the runtime dylib that opens the
  /// window. Builds a 56-byte `ZoRuntimeContext` on the stack
  /// (template ptr via PC-relative `adr`, template len as
  /// immediate, the dispatcher / text-binding / list-binding
  /// pointers, then the `TextBinding` + `ListBindingAbi`
  /// arrays it points at), sets `x0` to its address, and `bl`s
  /// `_zo_run_native`. The call blocks until the user closes
  /// the window.
  ///
  /// We don't go through `emit_extern_call` here because
  /// the `sub sp` invalidates its `caller_save_base`
  /// offsets — programs with caller-save liveness across
  /// `#render` are unsupported (the directive is positioned
  /// at the directive site; the call blocks anyway).
  ///
  /// Side effect: registers `_zo_run_native` in
  /// `extern_dylib_paths` so the linker routes it through
  /// the runtime `LC_LOAD_DYLIB` and selects the full UI
  /// dylib for staging.
  fn emit_render_call(&mut self, value: ValueId) {
    self.ensure_runtime_dylib_registered();

    let template_symbol = Symbol(value.0 + TEMPLATE_SYMBOL_OFFSET);
    let template_len = self
      .template_data
      .iter()
      .find_map(|(s, b)| (*s == template_symbol).then_some(b.len()))
      .unwrap_or(0) as u64;
    let has_handlers = self
      .template_handlers
      .get(&value)
      .is_some_and(|hs| !hs.is_empty());
    let bindings = self
      .template_text_bindings
      .get(&value)
      .cloned()
      .unwrap_or_default();
    let bindings_count = bindings.len();
    // (cmd_idx, items_slot, recipe_sym, recipe_len) per list
    // binding — cloned out so the emit loop can mutate `self`.
    let list_bindings: Vec<(u32, u32, Symbol, u32)> = self
      .template_list_bindings
      .get(&value)
      .map(|entries| {
        entries
          .iter()
          .map(|e| (e.cmd_idx, e.items_slot, e.recipe_sym, e.recipe_len))
          .collect()
      })
      .unwrap_or_default();
    let list_count = list_bindings.len();
    // (cmd_idx, attr_idx, slot, is_str) per attr binding.
    let attr_bindings: Vec<(u32, u32, u32, bool)> = self
      .template_attr_bindings
      .get(&value)
      .cloned()
      .unwrap_or_default();
    let attr_count = attr_bindings.len();

    // Stack layout (mirrors `ZoRuntimeContext` in
    // `zo-runtime-render::aot`):
    //   [sp +  0..8 ] template_ptr
    //   [sp +  8..16] template_len
    //   [sp + 16..24] handle_event
    //   [sp + 24..32] text_bindings_ptr
    //   [sp + 32..40] text_bindings_count
    //   [sp + 40..48] list_bindings_ptr
    //   [sp + 48..56] list_bindings_count
    //   [sp + 56..64] attr_bindings_ptr
    //   [sp + 64..72] attr_bindings_count
    //   [sp + 72 .. +16*T] TextBinding[T] — 16B each:
    //         cmd_idx u32 @0, slot_id u32 @4, is_str u32 @8,
    //         _pad u32 @12.
    //   [.. +24*L] ListBindingAbi[L] — 24B each: cmd_idx u32 @0,
    //         items_slot u32 @4, recipe_ptr @8, recipe_len @16.
    //   [.. +16*A] AttrBindingAbi[A] — 16B each: cmd_idx u32 @0,
    //         attr_idx u32 @4, slot u32 @8, is_str u32 @12.
    const CTX_BYTES: i16 = 72;
    const TEXT_BASE: i16 = CTX_BYTES;
    const TEXT_STRIDE: i16 = 16;
    const LIST_STRIDE: i16 = 24;
    const ATTR_STRIDE: i16 = 16;

    let text_bytes = (bindings_count as i16) * TEXT_STRIDE;
    let list_base = TEXT_BASE + text_bytes;
    let list_bytes = (list_count as i16) * LIST_STRIDE;
    let attr_base = list_base + list_bytes;
    let attr_bytes = (attr_count as i16) * ATTR_STRIDE;
    let total = CTX_BYTES + text_bytes + list_bytes + attr_bytes;
    // Align up to 16; AArch64 needs sp 16-byte aligned.
    let stack_reserve = ((total + 15) & !15) as u16;

    // x9 = &template (PC-relative; ADR fixup patches
    // against the appended postcard payload).
    let fixup_pos = self.emitter.current_offset();

    self.string_fixups.push((fixup_pos, template_symbol));
    self.emitter.emit_adr(X9, 0);

    // x10 = template_len.
    self.emit_mov_imm_64(X10, template_len);

    // x11 = &_zo_dispatch_<id> (PC-relative). Only when
    // the template has event handlers.
    if has_handlers {
      let dispatcher_symbol =
        Symbol(value.0 + TEMPLATE_DISPATCHER_SYMBOL_OFFSET);
      let adr_pos = self.emitter.current_offset();

      self.emitter.emit_adr(X11, 0);
      // Synthetic dispatchers live in the global
      // namespace — they're not pack-owned.
      self
        .function_addr_fixups
        .push((adr_pos, (dispatcher_symbol, None)));
    }

    self.emitter.emit_sub_imm(SP, SP, stack_reserve);
    self.emitter.emit_str(X9, SP, 0);
    self.emitter.emit_str(X10, SP, 8);

    if has_handlers {
      self.emitter.emit_str(X11, SP, 16);
    } else {
      self.emitter.emit_str(XZR, SP, 16);
    }

    if bindings_count > 0 {
      // 16-byte `#[repr(C)] TextBinding` per entry,
      // emitted as two `i64` halves: low = cmd_idx |
      // slot_id<<32, high = is_str (with _pad=0).
      for (i, &(cmd_idx, slot_id, is_str)) in bindings.iter().enumerate() {
        let entry_base = TEXT_BASE + (i as i16) * TEXT_STRIDE;
        let lo = (cmd_idx as u64) | ((slot_id as u64) << 32);
        let hi = is_str as u64;

        self.emit_mov_imm_64(X9, lo);
        self.emitter.emit_str(X9, SP, entry_base);
        self.emit_mov_imm_64(X9, hi);
        self.emitter.emit_str(X9, SP, entry_base + 8);
      }

      // text_bindings_ptr = SP + TEXT_BASE.
      self.emitter.emit_add_imm(X9, SP, TEXT_BASE as u16);
      self.emitter.emit_str(X9, SP, 24);

      // text_bindings_count.
      self.emit_mov_imm_64(X9, bindings_count as u64);
      self.emitter.emit_str(X9, SP, 32);
    } else {
      self.emitter.emit_str(XZR, SP, 24);
      self.emitter.emit_str(XZR, SP, 32);
    }

    if list_count > 0 {
      // 24-byte `#[repr(C)] ListBindingAbi` per entry: low 8
      // bytes = cmd_idx | items_slot<<32; recipe_ptr (ADR
      // fixup) @8; recipe_len @16.
      for (i, &(cmd_idx, items_slot, recipe_sym, recipe_len)) in
        list_bindings.iter().enumerate()
      {
        let entry_base = list_base + (i as i16) * LIST_STRIDE;
        let lo = (cmd_idx as u64) | ((items_slot as u64) << 32);

        self.emit_mov_imm_64(X9, lo);
        self.emitter.emit_str(X9, SP, entry_base);

        // recipe_ptr — PC-relative ADR resolved against the
        // embedded recipe blob via `string_fixups`.
        let adr_pos = self.emitter.current_offset();

        self.string_fixups.push((adr_pos, recipe_sym));
        self.emitter.emit_adr(X9, 0);
        self.emitter.emit_str(X9, SP, entry_base + 8);

        self.emit_mov_imm_64(X9, recipe_len as u64);
        self.emitter.emit_str(X9, SP, entry_base + 16);
      }

      // list_bindings_ptr = SP + list_base.
      self.emitter.emit_add_imm(X9, SP, list_base as u16);
      self.emitter.emit_str(X9, SP, 40);

      // list_bindings_count.
      self.emit_mov_imm_64(X9, list_count as u64);
      self.emitter.emit_str(X9, SP, 48);
    } else {
      self.emitter.emit_str(XZR, SP, 40);
      self.emitter.emit_str(XZR, SP, 48);
    }

    if attr_count > 0 {
      // 16-byte `#[repr(C)] AttrBindingAbi` per entry, two i64
      // halves: low = cmd_idx | attr_idx<<32; high = slot |
      // is_str<<32.
      for (i, &(cmd_idx, attr_idx, slot, is_str)) in
        attr_bindings.iter().enumerate()
      {
        let entry_base = attr_base + (i as i16) * ATTR_STRIDE;
        let lo = (cmd_idx as u64) | ((attr_idx as u64) << 32);
        let hi = (slot as u64) | ((is_str as u64) << 32);

        self.emit_mov_imm_64(X9, lo);
        self.emitter.emit_str(X9, SP, entry_base);
        self.emit_mov_imm_64(X9, hi);
        self.emitter.emit_str(X9, SP, entry_base + 8);
      }

      // attr_bindings_ptr = SP + attr_base.
      self.emitter.emit_add_imm(X9, SP, attr_base as u16);
      self.emitter.emit_str(X9, SP, 56);

      // attr_bindings_count.
      self.emit_mov_imm_64(X9, attr_count as u64);
      self.emitter.emit_str(X9, SP, 64);
    } else {
      self.emitter.emit_str(XZR, SP, 56);
      self.emitter.emit_str(XZR, SP, 64);
    }

    // `MOV X0, SP` — AArch64 MOV (register) is `ORR Rd,
    // XZR, Rm`; SP and XZR share encoding 31 so ORR
    // would zero X0. Use `ADD X0, SP, #0` (the "MOV
    // from/to SP" idiom).
    self.emitter.emit_add_imm(X0, SP, 0);

    self.emit_extern_call_no_spill(self.run_symbol());
    self.emitter.emit_add_imm(SP, SP, stack_reserve);
  }

  /// The runtime entry `#render` calls: the wry webview entry for a
  /// `--target webview` build, the eframe entry otherwise.
  fn run_symbol(&self) -> &'static str {
    match self.webviewing {
      Webviewing::Yes => SYM_RUN_WEB,
      Webviewing::No => SYM_RUN,
    }
  }

  /// Like `emit_extern_call` but without saving / restoring
  /// caller-save registers. Used by `emit_render_call`,
  /// which moves `sp` to allocate its on-stack
  /// `ZoRuntimeContext` — the spill offsets in
  /// `emit_extern_call` are relative to the original `sp`
  /// and would clobber the newly-allocated struct. Callers
  /// must ensure no live values are in `X9..X17` at the
  /// call site.
  fn emit_extern_call_no_spill(&mut self, c_sym: &str) {
    let bl_pos = self.emitter.current_offset();
    let sym = c_sym.to_string();

    self.emitter.emit_bl(0);
    self.extern_fixups.push((bl_pos, sym.clone()));

    if self.extern_used_set.insert(sym.clone()) {
      self.extern_used.push(sym);
    }
  }

  /// Return the effective `has_calls` for the function
  /// starting at `idx`: the allocator's view OR'd with
  /// our reactive-state promotion. The override exists
  /// because inserted `bl _zo_state_*` calls clobber X30
  /// and need the caller-save spill area, but the allocator
  /// is unaware of these synthesised calls.
  fn promoted_has_calls(&self, idx: u32, base: bool) -> bool {
    base || self.fns_needing_calls.contains(&InsnIdx(idx))
  }

  /// Idempotently route every UI runtime symbol to the
  /// canonical runtime dylib (`RUNTIME_DYLIB_FILE`,
  /// `@executable_path/libzo_runtime.dylib`). The linker
  /// rewrites that to `@loader_path/deps/libzo_runtime.dylib`
  /// and folds it into the single runtime `LC_LOAD_DYLIB`,
  /// so a `#render` program loads one staged dylib rather than
  /// a parallel absolute-path native reference. Importing
  /// any of these symbols is what flips the linker's
  /// `RuntimeKind` to `Full`.
  ///
  /// The registered path is a compile-time literal, so this
  /// only checks / inserts into the `extern_dylib_paths`
  /// map — no syscall.
  fn ensure_runtime_dylib_registered(&mut self) {
    for sym in [
      self.run_symbol(),
      SYM_STATE_INIT,
      SYM_STATE_GET,
      SYM_STATE_SET,
      SYM_STATE_GET_STR,
      SYM_STATE_SET_STR,
      SYM_STATE_ARR_PUSH,
    ] {
      self
        .extern_dylib_paths
        .entry(sym.to_string())
        .or_insert_with(|| RUNTIME_DYLIB_FILE.to_string());
    }
  }

  /// Emit `bl _zo_state_init(N)` at the START of main's
  /// body — runs once at program startup, before any
  /// reactive `mut` initialiser fires. Idempotent on the
  /// runtime side (`zo_state_init` is a `resize` upward).
  /// No-op when no reactive state was detected by the
  /// pre-pass.
  fn emit_state_init_prologue(&mut self) {
    let state_count = self.reactive_slots.len();

    if state_count == 0 {
      return;
    }

    self.ensure_runtime_dylib_registered();
    self.emitter.emit_mov_imm(X0, state_count as u16);
    self.emit_extern_call(SYM_STATE_INIT);
  }

  /// Emit `mov w0, slot; bl _zo_state_get*; mov dst, x0`
  /// — the read side of a reactive `mut`. Used in place of
  /// the stack-frame `ldr` when the symbol carries a slot
  /// in `reactive_slots`. `is_str` picks the str-typed FFI
  /// (returns a pointer into a `STR_STATE` clone the
  /// runtime sizes per slot for stable address) over the
  /// int FFI (returns `i64` from `STATE`).
  fn emit_state_load(&mut self, dst: Register, slot: u32, is_str: bool) {
    self.ensure_runtime_dylib_registered();
    self.emitter.emit_mov_imm(X0, slot as u16);
    self.emit_extern_call(if is_str {
      SYM_STATE_GET_STR
    } else {
      SYM_STATE_GET
    });

    if dst != X0 {
      self.emitter.emit_mov_reg(dst, X0);
    }
  }

  /// Emit `mov w0, slot; mov x1, value; bl _zo_state_set*`
  /// — the write side of a reactive `mut`. Used in place
  /// of the stack-frame `str` when the symbol carries a
  /// slot in `reactive_slots`. The str-typed FFI variant
  /// reads the length-prefix from `value` (a zo `str`
  /// pointer) and copies the bytes into `STR_STATE[slot]`,
  /// so the closure caller's frame doesn't need to keep
  /// the source buffer alive past this call.
  fn emit_state_store(&mut self, slot: u32, value: Register, is_str: bool) {
    self.ensure_runtime_dylib_registered();

    // Order matters: X1 first (so a later `mov W0, imm`
    // doesn't clobber the value before we move it), then
    // X0 for the slot. If `value` is already X1 we skip
    // the move.
    if value != X1 {
      self.emitter.emit_mov_reg(X1, value);
    }

    self.emitter.emit_mov_imm(X0, slot as u16);
    self.emit_extern_call(if is_str {
      SYM_STATE_SET_STR
    } else {
      SYM_STATE_SET
    });
  }

  /// Emit `mov x1, value; mov w0, slot; bl _zo_state_arr_push` —
  /// the reactive `[]str` push. `value` is a zo `str` pointer
  /// (length-prefixed); the runtime copies its bytes onto
  /// `ARR_STATE[slot]` and marks the slot dirty so the list
  /// re-renders. Mirrors `emit_state_store`'s X1-first ordering.
  fn emit_state_arr_push(&mut self, slot: u32, value: Register) {
    self.ensure_runtime_dylib_registered();

    if value != X1 {
      self.emitter.emit_mov_reg(X1, value);
    }

    self.emitter.emit_mov_imm(X0, slot as u16);
    self.emit_extern_call(SYM_STATE_ARR_PUSH);
  }

  /// The reactive slot id for `sym`, minting a fresh one (the
  /// next free index) when the symbol isn't bound yet. Slots
  /// share one id space across scalar, string, and array state —
  /// the runtime's `STATE` / `STR_STATE` / `ARR_STATE` are all
  /// indexed by the same slot id.
  fn reactive_slot_for(&mut self, sym: Symbol) -> u32 {
    if let Some(&slot) = self.reactive_slots.get(&sym) {
      slot
    } else {
      let slot = self.reactive_slots.len() as u32;

      self.reactive_slots.insert(sym, slot);
      slot
    }
  }

  /// Generate a complete "Hello, World" executable.
  // TODO: move this to common.rs in tests folder.
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
    macho.add_main(CODE_OFFSET as u64);
    macho.add_dyld_info();
    macho.finish()
  }

  /// Generate a complete "Hello, World" executable with
  /// code signature.
  // TODO: move this to common.rs in tests folder.
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
    macho.add_main(CODE_OFFSET as u64);
    macho.add_dyld_info();
    macho.finish_with_signature()
  }
}

impl<'a> zo_codegen_backend::Backend for ARM64Gen<'a> {
  fn generate(&mut self, sir: &Sir) -> Artifact {
    self.generate(sir)
  }
}
