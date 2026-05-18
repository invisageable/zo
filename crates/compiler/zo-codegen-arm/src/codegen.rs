pub(crate) mod template;

use zo_buffer::Buffer;
use zo_codegen_backend::{Artifact, MachoLinkObject};
use zo_emitter_arm::{
  ARM64Emitter, COND_CC, COND_CS, COND_EQ, COND_GE, COND_GT, COND_HI, COND_LE,
  COND_LS, COND_LT, COND_NE, COND_VC, COND_VS, D0, D1, D16, FpRegister,
  PatchSite, Register, SP, X0, X1, X2, X3, X9, X10, X11, X16, X17, X29, X30,
  XZR,
};
use zo_interner::{DenseMap, Interner, Sentinel, Symbol};
use zo_register_allocation::{
  EmitTiming, IO_RESULT_FRAME_SLOTS, IO_SHARED_BUF_SLOTS, RegAlloc,
  RegisterClass, SpillKind, resolve_ty,
};
use zo_sir::{BinOp, Insn, LoadSource, Sir, SpawnKind, UnOp};
use zo_ty::{Ty, TyId, TyTable};
use zo_value::{FunctionKind, ValueId};
use zo_writer_macho::{CODE_OFFSET, DebugFrameEntry, MachO, TEXT_SECTION_BASE};

use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

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
const CALLER_SAVE_RESERVE: u32 = 120;
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

// Runtime dylib symbols: every `#dom` program emits calls
// into `libzo_runtime_native.dylib`. Names match the
// `#[no_mangle]` exports in `zo-runtime-native::ffi`
// (Mach-O leading-underscore convention).
const RUNTIME_DYLIB_FILE: &str = "libzo_runtime_native.dylib";
const SYM_RUN: &str = "_zo_run_native";
const SYM_STATE_INIT: &str = "_zo_state_init";
const SYM_STATE_GET: &str = "_zo_state_get";
const SYM_STATE_SET: &str = "_zo_state_set";
// Str-typed reactive slots route through a separate
// `Vec<Vec<u8>>` (length-prefixed copies) — the i64 STATE
// can't hold a string value.
const SYM_STATE_GET_STR: &str = "_zo_state_get_str";
const SYM_STATE_SET_STR: &str = "_zo_state_set_str";

/// Locate `libzo_runtime_native.dylib` next to the running
/// `zo` binary, falling back to the sibling cargo profile
/// when only one of `target/debug` / `target/release` has
/// been built. `cargo run --bin zo --release` produces
/// `target/release/zo` but doesn't compile the cdylib
/// (it's not a rlib dep), so the dylib only exists under
/// `target/debug/` — without the fallback the compiled
/// program's `LC_LOAD_DYLIB` would be a bare basename
/// that dyld can't resolve at run time.
///
/// Candidate order:
///   1. `<exe-dir>/<dylib>` — same profile.
///   2. `<exe-dir>/../debug/<dylib>` — release-zo + debug-dylib.
///   3. `<exe-dir>/../release/<dylib>` — debug-zo + release-dylib.
///   4. `<exe-dir>/../lib/<dylib>` — installed layout
///      (`tasks/zo-install.sh` will write here).
///
/// Returns `None` when none of the candidates exist; the
/// caller falls back to a bare basename so the linker
/// still records something and dyld surfaces a clean
/// "image not found" diagnostic at runtime.
fn resolve_runtime_dylib_path() -> Option<String> {
  let exe = std::env::current_exe().ok()?;
  let exe_dir = exe.parent()?;
  let candidates = [
    exe_dir.join(RUNTIME_DYLIB_FILE),
    exe_dir.join("..").join("debug").join(RUNTIME_DYLIB_FILE),
    exe_dir.join("..").join("release").join(RUNTIME_DYLIB_FILE),
    exe_dir.join("..").join("lib").join(RUNTIME_DYLIB_FILE),
  ];

  for candidate in &candidates {
    if candidate.exists() {
      let canon = candidate
        .canonicalize()
        .unwrap_or_else(|_| candidate.clone());

      return Some(canon.to_string_lossy().into_owned());
    }
  }

  None
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
  pub(super) functions: HashMap<Symbol, u32>,
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
  function_addr_fixups: Vec<(u32, Symbol)>,
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
  /// the `ZoRuntimeContext` at `#dom` time. Built by the
  /// same pre-pass that populates `reactive_slots`.
  template_text_bindings: HashMap<ValueId, Vec<(u32, u32, bool)>>,
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
  /// Cached `libzo_runtime_native.dylib` path used for
  /// the `LC_LOAD_DYLIB` entry. `ensure_runtime_dylib_
  /// registered` resolves it once via `current_exe()` +
  /// `parent()` + `join()` + `exists()` (a syscall on
  /// Apple) and reuses the result across every subsequent
  /// call site (`emit_state_init_prologue`,
  /// `emit_state_load`, `emit_state_store`,
  /// `emit_render_call`).
  runtime_dylib_path: Option<String>,
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
  /// Current function's start index into SIR instructions.
  current_fn_start: Option<usize>,
  /// Mutable variable stack slots: name → offset from SP.
  mutable_slots: HashMap<u32, u32>,
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
      template_data: Vec::new(),
      template_handlers: HashMap::default(),
      reactive_slots: HashMap::default(),
      template_text_bindings: HashMap::default(),
      fns_needing_calls: HashSet::default(),
      runtime_dylib_path: None,
      has_templates: false,
      labels: HashMap::default(),
      branch_fixups: Vec::new(),
      reg_alloc: None,
      spill_offsets: Vec::new(),
      current_fn_start: None,
      mutable_slots: HashMap::default(),
      array_var_blocks: HashMap::default(),
      param_slots: HashMap::default(),
      param_sym_slots: HashMap::default(),
      caller_save_base: 0,
      next_mut_slot: 0,
      struct_base: 0,
      chan_scratch_base: 0,
      select_scratch_base: 0,
      array_push_scratch_base: 0,
      next_struct_slot: 0,
      io_shared_buf_offset: None,
      struct_return_fns: HashMap::default(),
      last_was_math_intrinsic: false,
      extern_used: Vec::new(),
      extern_used_set: HashSet::default(),
      extern_stub_offsets: HashMap::default(),
      extern_fixups: Vec::new(),
      call_fixups: Vec::new(),
      enum_metas: HashMap::default(),
      next_enum_sym: ENUM_SYNTHETIC_SYM_BASE,
      value_types: HashMap::default(),
      array_metas: HashMap::default(),
      map_metas: HashMap::default(),
      vec_metas: HashMap::default(),
      set_metas: HashMap::default(),
      value_enum_field_tys: HashMap::default(),
      local_enum_field_tys: HashMap::default(),
      value_tuple_elem_tys: HashMap::default(),
      local_tuple_elem_tys: HashMap::default(),
      struct_metas: HashMap::default(),
      enum_walk_done_fixups: Vec::new(),
      value_def_idx: DenseMap::new(),
      ffi_sigs: HashMap::default(),
      ffi_link_names: HashMap::default(),
      extern_dylib_paths: HashMap::default(),
      type_view: None,
    }
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

  // --- Register allocation helpers ---

  /// Look up the allocated register for a ValueId.
  fn alloc_reg(&self, vid: ValueId) -> Option<Register> {
    // Assignments are scoped per FunDef: ValueId counters
    // reset across functions.
    let fn_start = self.current_fn_start? as u32;

    self
      .reg_alloc
      .as_ref()
      .and_then(|a| a.get(fn_start, vid))
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
    let fn_start = self.current_fn_start? as u32;

    self
      .reg_alloc
      .as_ref()
      .and_then(|a| a.get_fp(fn_start, vid))
      .map(FpRegister::new)
  }

  /// Bind per-function state at the start of every FunDef:
  /// records the current function context, then resets every
  /// frame-local map / counter so leftovers from the
  /// previous function can't alias. Pairs with the
  /// per-function rebuild of `value_def_idx` over the
  /// function's SIR range (ValueId counters reset per
  /// function, so the flat map would alias vid=N).
  fn enter_function(&mut self, name: Symbol, idx: usize, all_insns: &[Insn]) {
    self.current_function = Some(name);
    self.current_fn_start = Some(idx);

    self.value_def_idx.clear();
    self.mutable_slots.clear();
    self.array_var_blocks.clear();
    self.next_mut_slot = 0;
    self.next_struct_slot = 0;
    self.io_shared_buf_offset = None;
    self.param_slots.clear();
    self.param_sym_slots.clear();
    self.local_enum_field_tys.clear();
    self.local_tuple_elem_tys.clear();

    let fn_end = all_insns[idx + 1..]
      .iter()
      .position(|ins| matches!(ins, Insn::FunDef { .. }))
      .map(|p| idx + 1 + p)
      .unwrap_or(all_insns.len());

    for (offset, ins) in all_insns[idx..fn_end].iter().enumerate() {
      let widx = InsnIdx((idx + offset) as u32);

      match ins {
        Insn::ConstInt { dst, .. }
        | Insn::ConstFloat { dst, .. }
        | Insn::ConstBool { dst, .. }
        | Insn::Load { dst, .. } => {
          self.value_def_idx.insert(*dst, widx);
        }
        _ => {}
      }
    }
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
      } => self.emit_str_sp(Register::new(*reg), *slot * STACK_SLOT_SIZE),
      SpillKind::Load {
        reg,
        slot,
        class: RegisterClass::GP,
      } => self.emit_ldr_sp(Register::new(*reg), *slot * STACK_SLOT_SIZE),
      SpillKind::Store {
        reg,
        slot,
        class: RegisterClass::FP,
      } => self.emit_str_fp_sp(FpRegister::new(*reg), *slot * STACK_SLOT_SIZE),
      SpillKind::Load {
        reg,
        slot,
        class: RegisterClass::FP,
      } => self.emit_ldr_fp_sp(FpRegister::new(*reg), *slot * STACK_SLOT_SIZE),
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
    self.reg_alloc = Some(RegAlloc::allocate(
      &sir.instructions,
      sir.next_value_id,
      self.interner,
      type_view,
    ));

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
    }

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
        pack_dylib.insert(*pack, self.interner.get(*sym).to_owned());
      }
    }

    // Single-walk pre-pass: collect FFI signatures + bind
    // each `pub ffi` to its declaring pack's `#link` dylib
    // path. Reads `owning_pack` from the FunDef itself
    // (set by the executor at emit time) instead of
    // tracking the most recent `PackDecl` positionally —
    // the positional model mis-attributed top-level user
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

        if let Some(pk) = owning_pack
          && let Some(path) = pack_dylib.get(pk)
        {
          let c_sym = c_sym_for(self.interner, *name, *link_name);

          self.extern_dylib_paths.insert(c_sym, path.clone());
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

    let mut sym_first_store_ty: HashMap<Symbol, TyId> = HashMap::default();

    for insn in insns.iter() {
      if let Insn::Store { name, ty_id, .. } = insn {
        sym_first_store_ty.entry(*name).or_insert(*ty_id);
      }
    }

    for insn in insns.iter() {
      let Insn::Template { id, bindings, .. } = insn else {
        continue;
      };

      let mut entries: Vec<(u32, u32, bool)> =
        Vec::with_capacity(bindings.text.len());

      for &(cmd_idx, sym) in &bindings.text {
        let slot = if let Some(&s) = self.reactive_slots.get(&sym) {
          s
        } else {
          let s = self.reactive_slots.len() as u32;

          self.reactive_slots.insert(sym, s);
          s
        };

        let is_str = sym_first_store_ty
          .get(&sym)
          .is_some_and(|t| t.0 == STR_TYPE_ID);

        entries.push((cmd_idx as u32, slot, is_str));
      }

      self.template_text_bindings.insert(*id, entries);
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
          _ => {}
        }
      }
    }

    // Whole-SIR pre-pass for top-level / pre-FunDef code
    // paths that consult `value_def_idx`. The per-FunDef
    // arm in `translate_insn` rebuilds the map scoped to
    // each function — required because ValueId counters
    // reset per function and the flat map would alias.
    self.value_def_idx.clear();

    for (i, insn) in insns.iter().enumerate() {
      let idx = InsnIdx(i as u32);

      match insn {
        Insn::ConstInt { dst, .. }
        | Insn::ConstFloat { dst, .. }
        | Insn::ConstBool { dst, .. }
        | Insn::Load { dst, .. } => {
          self.value_def_idx.insert(*dst, idx);
        }
        _ => {}
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

  /// Hand off codegen state to the linker phase.
  ///
  /// Consumes `self` and the freshly produced `artifact`,
  /// resolves the `main` and `_zo_ui_entry_point` offsets
  /// (so the linker doesn't need an interner handle), and
  /// bundles every fixup / symbol table the mach-o
  /// assembler needs into a `MachoLinkObject`. The
  /// resulting object is the only data that crosses the
  /// codegen → linker phase boundary.
  pub fn into_link_object(self, artifact: Artifact) -> MachoLinkObject {
    let main_offset = self
      .interner
      .symbol("main")
      .and_then(|s| self.functions.get(&s).copied());

    let ui_entry_offset = if self.has_templates {
      self.functions.get(&Symbol(UI_ENTRY_SYMBOL)).copied()
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
      Insn::ConstString { dst, ty_id, .. } => {
        self.value_types.insert(dst.0, *ty_id);
      }
      _ => {}
    }

    match insn {
      Insn::FunDef { name, params, .. } => {
        let offset = self.emitter.current_offset();

        self.functions.insert(*name, offset);
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
          let has_calls = self.promoted_has_calls(idx as u32, has_calls);

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
            + ARRAY_PUSH_SCRATCH_SIZE
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

          // Past every other area so realloc / heap-clone
          // saves can't alias struct slots.
          self.array_push_scratch_base =
            self.select_scratch_base + select_scratch_size;

          let param_base = spill_size + mut_size;

          for (i, (sym, ty_id)) in params.iter().enumerate() {
            let off = param_base + i as u32 * STACK_SLOT_SIZE;
            let is_fp =
              ty_id.0 >= FLOAT_TYPE_ID_MIN && ty_id.0 <= FLOAT_TYPE_ID_MAX;

            if is_fp {
              let src = FpRegister::new(i as u8);

              self.emit_str_fp_sp(src, off);
            } else {
              let src = Register::new(i as u8);

              self.emit_str_sp(src, off);
            }

            self.param_slots.insert(i as u32, off);
            // Also index by the parameter's symbol so
            // `LoadSource::Local(sym)` (emitted for immutable
            // param reads) can resolve the spill slot.
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
          // frames.
          if let Some(&state_slot) = self.reactive_slots.get(sym) {
            if let Some(dst_reg) = self.alloc_reg(*dst) {
              self.emit_state_load(dst_reg, state_slot, ty_id.0 == STR_TYPE_ID);
            }

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

      Insn::Call {
        name,
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
          // the std `pub ffi __zo_misato_*` declarations
          // name the C symbol directly, so no mapping is
          // needed.
          "exists" => self.emit_io_exists(args, idx),
          "read_file" => self.emit_io_read_file(args, idx),
          "write_file" => self.emit_io_write_file(args, idx),
          "append_file" => self.emit_io_append_file(args, idx),
          "readln" => self.emit_io_read_stdin(idx, "_zo_io_readln"),
          "read" => self.emit_io_read_stdin(idx, "_zo_io_read"),
          "args" => self.emit_io_args(idx),

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
          "Vec::remove" => self.emit_vec_remove(args, idx),

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
          "__zo_map_len_raw" => self.emit_map_len_raw(args, idx),
          "__zo_map_free_raw" => self.emit_map_free_raw(args, idx),
          "__zo_vec_len_raw" => self.emit_vec_len_raw(args, idx),
          "__zo_vec_free_raw" => self.emit_vec_free_raw(args, idx),
          "__zo_set_len_raw" => self.emit_set_len_raw(args, idx),
          "__zo_set_free_raw" => self.emit_set_free_raw(args, idx),

          // `str.replace(needle, with)` — `apply str` body
          // forwards `(self, needle, with)` to this raw FFI;
          // codegen forwards X0..X2 to `_zo_str_replace`.
          "__zo_str_replace" => self.emit_str_replace_raw(args, idx),

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

              self.emit_ffi_call(c_sym, &abi, args, idx);
              return;
            }

            // User-function call (zo's own positional
            // calling convention — not full AAPCS).
            // Move args to X0-X7 (GP) or D0-D7 (FP).
            // Collect moves first to detect clobbering:
            // if src of move B == dst of move A, moving A
            // first overwrites B's source. Save conflicting
            // sources to X16 before any moves happen.
            //
            // Stack-bounded — at most `MAX_REG_ARGS` GP
            // arg slots — so a fixed array + len cursor
            // replaces a per-call `Vec` allocation.
            let mut gp_moves: [(Register, Register); MAX_REG_ARGS] =
              [(X0, X0); MAX_REG_ARGS];
            let mut gp_moves_len: usize = 0;

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
                  gp_moves[gp_moves_len] = (dst_reg, src_reg);
                  gp_moves_len += 1;
                }
              }
            }

            let gp_moves = &gp_moves[..gp_moves_len];

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
            for (dst, src) in gp_moves {
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

              self.emit_str_sp(reg, off);
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

              self.emit_ldr_sp(reg, off);
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
                  // Bump cursor past the outer slots so
                  // nested copies can use the trailing
                  // slots within our reserved budget.
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
                  // Flat fallback — caller didn't supply
                  // a type view, so we can't tell which
                  // fields are structs. `deep_slots` then
                  // equals the flat field count (see
                  // `build_struct_return_map`'s `None`
                  // branch), so this still matches the
                  // budget.
                  for i in 0..deep_slots {
                    let src_off = (i * STACK_SLOT_SIZE) as i16;
                    let dst_off = dst_base + i * STACK_SLOT_SIZE;

                    self.emitter.emit_ldr(X16, X0, src_off);
                    self.emit_str_sp(X16, dst_off);
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
          // Mirror the prologue's promotion so the
          // epilogue restores FP/LR + tears down the
          // matching frame size.
          let has_calls = self
            .current_fn_start
            .map(|s| self.promoted_has_calls(s as u32, has_calls))
            .unwrap_or(has_calls);
          let param_reserve = self.param_slots.len() as u32 * STACK_SLOT_SIZE;
          let caller_save = if has_calls { CALLER_SAVE_RESERVE } else { 0 };
          let frame = (spill_size
            + mut_size
            + param_reserve
            + caller_save
            + struct_size
            + chan_scratch_size
            + select_scratch_size
            + ARRAY_PUSH_SCRATCH_SIZE
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
        // `refresh_bindings` reads.
        if let Some(&slot) = self.reactive_slots.get(name) {
          let value_reg = match self.alloc_reg(*value) {
            Some(r) => r,
            None => return,
          };

          self.emit_state_store(slot, value_reg, ty_id.0 == STR_TYPE_ID);

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

        self.emit_mov_imm_64(X0, alloc_size);
        self.emit_extern_call("_malloc");

        // X0 holds the heap pointer; pin it in a callee-saved
        // register (X19) across the per-element stores so the
        // X16/X17 scratches we use for length/cap don't lose
        // it. AAPCS guarantees `_malloc` preserved X19, so no
        // explicit save needed. Element values landed back in
        // their allocator-assigned regs because
        // `emit_extern_call` saves X1..X15 across the BL.
        let r_buf = Register::new(19);

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

          self.emit_array_element_store_sp(*field, all_insns, off);
        }

        // Set dst register to point at this struct's
        // base. Use ADD (not MOV) because ARM64 MOV
        // via ORR encodes register 31 as XZR, not SP.
        if let Some(dst) = self.reg_for_insn(idx) {
          self.emit_add_sp_offset(dst, base);
        }

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
      Insn::ChannelRecv { dst, channel, .. } => {
        // ABI: `_zo_chan_recv(chan, dst: *mut u8)`.
        // The runtime writes into the scratch slot;
        // we then load the written value into the
        // destination register the allocator reserved
        // for `dst`.
        let slot = self.chan_scratch_base;

        if let Some(ch_reg) = self.alloc_reg(*channel)
          && ch_reg != X0
        {
          self.emitter.emit_mov_reg(X0, ch_reg);
        }

        self.emit_add_sp_offset(X1, slot);
        self.emit_extern_call("_zo_chan_recv");

        if let Some(dst_reg) = self.alloc_reg(*dst) {
          self.emit_ldr_sp(dst_reg, slot);
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

  /// Clobber-safe int arg marshaling. Given (dst, src)
  /// pairs, emits a sequence of `mov` that always lands the
  /// right value in each `dst`, even if a later `dst` is
  /// some other move's `src`. One scratch slot (X16) is
  /// enough for any 3-5 arg call (raylib's whole surface).
  ///
  /// Without this, calling `init_window(w, h, c_str("…"))`
  /// segfaults: c_str's result lands in X0, then `mov X0, w`
  /// clobbers it before `mov X2, x0` can read it.
  fn emit_safe_int_arg_moves(&mut self, moves: &[(Register, Register)]) {
    let mut saved_reg: Option<Register> = None;

    for j in 0..moves.len() {
      let (_, src) = moves[j];

      let is_clobbered = moves
        .iter()
        .enumerate()
        .any(|(k, (dst, _))| k != j && *dst == src);

      if is_clobbered && saved_reg.is_none() {
        self.emitter.emit_mov_reg(X16, src);
        saved_reg = Some(src);
      }
    }

    for &(dst, src) in moves {
      let actual_src = if Some(src) == saved_reg { X16 } else { src };

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

    for (i, abi_arg) in abi.args.iter().enumerate() {
      let arg_value = args[i];

      match abi_arg {
        AbiArg::Gp(dst_reg) => {
          let src = self.alloc_reg(arg_value).unwrap_or(*dst_reg);
          gp_moves.push((*dst_reg, src));
        }

        AbiArg::Fp { reg: dst_reg, .. } => {
          let src = self.alloc_fp_reg(arg_value).unwrap_or(*dst_reg);
          fp_moves.push((*dst_reg, src));
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

      AbiRet::Gp(reg) => {
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
    if !self.string_data_seen.contains(&msg_sym) {
      let mut buf = Buffer::new();
      let len = msg.len() as u64;

      buf.bytes(&len.to_le_bytes());
      buf.bytes(msg);
      buf.bytes(b"\0");

      self.string_data.push((msg_sym, buf.finish()));
      self.string_data_seen.insert(msg_sym);
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

  /// `__zo_str_replace(src, needle, with) -> str` — direct
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
  ) {
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
    self.emit_extern_call(runtime_call);

    self.emitter.emit_mov_imm(X16, 1);
    self.emitter.emit_eor(X16, X16, X0);
    self.emit_str_sp(X16, opt_base);

    self.emit_ldr_sp(X16, v_out_off);
    self.emit_str_sp(X16, opt_base + STACK_SLOT_SIZE);

    if let Some(dst) = self.reg_for_insn(idx) {
      self.emit_add_sp_offset(dst, opt_base);
    }
  }

  /// `v.get(idx)` — read-only lookup, returns `Option<T>`.
  fn emit_vec_get(&mut self, args: &[ValueId], idx: usize) {
    self.emit_vec_option_idx_call(args, idx, "_zo_vec_get");
  }

  /// `v.remove(idx)` — same shape as `get` plus the
  /// runtime shifts the tail down by one and decrements
  /// `len`.
  fn emit_vec_remove(&mut self, args: &[ValueId], idx: usize) {
    self.emit_vec_option_idx_call(args, idx, "_zo_vec_remove");
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

  /// `__zo_set_len_raw` and `__zo_set_free_raw` route
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
  fn materialize_value_into_x16(
    &mut self,
    elem: ValueId,
    all_insns: &[Insn],
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

          if let Some(&offset) = self.mutable_slots.get(&slot) {
            self.emit_ldr_sp(X16, offset);

            return true;
          }

          if let Some(&(offset, _)) = self.param_sym_slots.get(&slot) {
            self.emit_ldr_sp(X16, offset);

            return true;
          }

          false
        }
        LoadSource::Param(pidx) => {
          if let Some(&offset) = self.param_slots.get(pidx) {
            self.emit_ldr_sp(X16, offset);

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
    if self.materialize_value_into_x16(elem, all_insns) {
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
    if self.materialize_value_into_x16(elem, all_insns) {
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
  /// eframe window. Builds a 32-byte `ZoRuntimeContext`
  /// on the stack (template ptr via PC-relative `adr`,
  /// template len as immediate, both callback fields
  /// zero), sets `x0` to its address, and `bl`s
  /// `_zo_run_native`. The call blocks until the user
  /// closes the window.
  ///
  /// We don't go through `emit_extern_call` here because
  /// the `sub sp` invalidates its `caller_save_base`
  /// offsets — programs with caller-save liveness across
  /// `#dom` are unsupported (the directive is positioned
  /// at the directive site; the call blocks anyway).
  ///
  /// Side effect: registers `_zo_run_native` in
  /// `extern_dylib_paths` so the linker emits an
  /// `LC_LOAD_DYLIB` for `libzo_runtime_native.dylib`.
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

    // Stack layout (mirrors `ZoRuntimeContext` in
    // `zo-runtime-native::ffi`):
    //   [sp +  0..8 ] template_ptr
    //   [sp +  8..16] template_len
    //   [sp + 16..24] handle_event
    //   [sp + 24..32] text_bindings_ptr
    //   [sp + 32..40] text_bindings_count
    //   [sp + 40..40 + 16*N] text_bindings array — one
    //         `#[repr(C)] struct TextBinding` per entry,
    //         16 bytes: cmd_idx u32 @0, slot_id u32 @4,
    //         is_str u32 @8, _pad u32 @12.
    const CTX_BYTES: i16 = 40;
    const BINDINGS_BASE: i16 = CTX_BYTES;
    const BINDING_STRIDE: i16 = 16;

    let bindings_bytes = (bindings_count as i16) * BINDING_STRIDE;
    let total = CTX_BYTES + bindings_bytes;
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
      self.function_addr_fixups.push((adr_pos, dispatcher_symbol));
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
        let entry_base = BINDINGS_BASE + (i as i16) * BINDING_STRIDE;
        let lo = (cmd_idx as u64) | ((slot_id as u64) << 32);
        let hi = is_str as u64;

        self.emit_mov_imm_64(X9, lo);
        self.emitter.emit_str(X9, SP, entry_base);
        self.emit_mov_imm_64(X9, hi);
        self.emitter.emit_str(X9, SP, entry_base + 8);
      }

      // text_bindings_ptr = SP + BINDINGS_BASE.
      self.emitter.emit_add_imm(X9, SP, BINDINGS_BASE as u16);
      self.emitter.emit_str(X9, SP, 24);

      // text_bindings_count.
      self.emit_mov_imm_64(X9, bindings_count as u64);
      self.emitter.emit_str(X9, SP, 32);
    } else {
      self.emitter.emit_str(XZR, SP, 24);
      self.emitter.emit_str(XZR, SP, 32);
    }

    // `MOV X0, SP` — AArch64 MOV (register) is `ORR Rd,
    // XZR, Rm`; SP and XZR share encoding 31 so ORR
    // would zero X0. Use `ADD X0, SP, #0` (the "MOV
    // from/to SP" idiom).
    self.emitter.emit_add_imm(X0, SP, 0);

    self.emit_extern_call_no_spill(SYM_RUN);
    self.emitter.emit_add_imm(SP, SP, stack_reserve);
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

  /// Idempotently insert every runtime-dylib symbol into
  /// `extern_dylib_paths` so the Mach-O writer emits an
  /// `LC_LOAD_DYLIB` for `libzo_runtime_native.dylib`. Path
  /// resolution mirrors the `<exe-dir>/libzo_*.dylib`
  /// pattern that cargo's `cdylib` target produces — for
  /// dev workflow `target/debug/libzo_runtime_native.dylib`
  /// is right next to `target/debug/zo`. Falls back to a
  /// bare basename so the linker still records SOMETHING
  /// rather than failing silently when `current_exe()`
  /// can't be resolved (sandboxed test runners, etc.); dyld
  /// will then surface a clean "image not found" error at
  /// runtime instead of a malformed Mach-O.
  ///
  /// The on-disk lookup (a syscall on Apple) runs at most
  /// once per `ARM64Gen`: subsequent calls reuse the
  /// `runtime_dylib_path` cache and only check / insert
  /// into the `extern_dylib_paths` map.
  fn ensure_runtime_dylib_registered(&mut self) {
    let path = match &self.runtime_dylib_path {
      Some(p) => p.clone(),
      None => {
        let resolved = resolve_runtime_dylib_path()
          .unwrap_or_else(|| RUNTIME_DYLIB_FILE.to_string());

        self.runtime_dylib_path = Some(resolved.clone());
        resolved
      }
    };

    for sym in [
      SYM_RUN,
      SYM_STATE_INIT,
      SYM_STATE_GET,
      SYM_STATE_SET,
      SYM_STATE_GET_STR,
      SYM_STATE_SET_STR,
    ] {
      self
        .extern_dylib_paths
        .entry(sym.to_string())
        .or_insert_with(|| path.clone());
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
