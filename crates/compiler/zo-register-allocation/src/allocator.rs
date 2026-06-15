use crate::{
  ALLOCATABLE_FP, ALLOCATABLE_GP, EmitTiming, FnKey, FunctionInfo, RegAlloc,
  RegisterClass, SpillKind, SpillOp, flat_struct_slots_of,
};

use zo_interner::{Interner, Symbol};
use zo_liveness::{LivenessInfo, liveness};
use zo_sir::{Insn, LoadSource};
use zo_ty::{Ty, TyId, TyTable, struct_leaf_words};
use zo_value::FunctionKind;
use zo_value::ValueId;

use rustc_hash::FxHashMap as HashMap;

/// A single register pool (GP or FP).
struct RegPool {
  free: Vec<u8>,
  val_to_reg: HashMap<u32, u8>,
  reg_to_val: HashMap<u8, u32>,
  /// The set of regs that participate in allocation. Call
  /// result regs (x0/d0) live OUTSIDE this set — freeing
  /// them must not re-seed the pool or the next alloc
  /// would return x0 and desync with the post-call reload
  /// path (which rewrites `assignments[vid]`).
  allocatable: &'static [u8],
}

impl RegPool {
  fn new(regs: &'static [u8]) -> Self {
    Self {
      free: regs.iter().rev().copied().collect(),
      val_to_reg: HashMap::default(),
      reg_to_val: HashMap::default(),
      allocatable: regs,
    }
  }

  fn get(&self, vid: ValueId) -> Option<u8> {
    self.val_to_reg.get(&vid.0).copied()
  }

  fn assign(&mut self, vid: ValueId, reg: u8) {
    self.val_to_reg.insert(vid.0, reg);
    self.reg_to_val.insert(reg, vid.0);
    self.free.retain(|&r| r != reg);
  }

  fn free_value(&mut self, vid: ValueId) {
    if let Some(reg) = self.val_to_reg.remove(&vid.0) {
      self.reg_to_val.remove(&reg);
      // Only re-seed the free pool if this register is in
      // the allocatable set. x0/d0 (call-result reg) are
      // assigned explicitly by the Call handler but never
      // enter the pool — pushing them back here would
      // leak them into regular allocation.
      if self.allocatable.contains(&reg) {
        self.free.push(reg);
      }
    }
  }

  fn alloc_free(&mut self) -> Option<u8> {
    self.free.pop()
  }

  fn clear(&mut self, regs: &[u8]) {
    self.val_to_reg.clear();
    self.reg_to_val.clear();
    self.free = regs.iter().rev().copied().collect();
  }
}

/// Mutable state for the forward allocation pass.
struct AllocState {
  gp: RegPool,
  fp: RegPool,
  /// Tracks which values are FP (for correct spill emission).
  is_fp_value: HashMap<u32, bool>,
  /// Spill slots: ValueId.0 → slot index.
  spill_slots: HashMap<u32, u32>,
  /// Next spill slot index.
  next_spill: u32,
}

impl AllocState {
  fn new() -> Self {
    Self {
      gp: RegPool::new(&ALLOCATABLE_GP),
      fp: RegPool::new(&ALLOCATABLE_FP),
      is_fp_value: HashMap::default(),
      spill_slots: HashMap::default(),
      next_spill: 0,
    }
  }

  /// Look up the register holding a value (GP or FP).
  fn get(&self, vid: ValueId) -> Option<u8> {
    self.gp.get(vid).or_else(|| self.fp.get(vid))
  }

  fn is_fp(&self, vid: ValueId) -> bool {
    self.is_fp_value.get(&vid.0).copied().unwrap_or(false)
  }

  fn pool_mut(&mut self, fp: bool) -> &mut RegPool {
    if fp { &mut self.fp } else { &mut self.gp }
  }

  fn assign(&mut self, vid: ValueId, reg: u8, fp: bool) {
    self.is_fp_value.insert(vid.0, fp);
    self.pool_mut(fp).assign(vid, reg);
  }

  fn free_value(&mut self, vid: ValueId) {
    if self.is_fp(vid) {
      self.fp.free_value(vid);
    } else {
      self.gp.free_value(vid);
    }
    self.is_fp_value.remove(&vid.0);
  }

  fn spill_slot(&mut self, vid: u32) -> u32 {
    if let Some(&slot) = self.spill_slots.get(&vid) {
      return slot;
    }
    let slot = self.next_spill;
    self.next_spill += 1;
    self.spill_slots.insert(vid, slot);
    slot
  }

  fn evict(
    &mut self,
    vid: u32,
    insn_idx: usize,
    liveness: &LivenessInfo,
    local_idx: usize,
    result: &mut RegAlloc,
  ) -> u8 {
    let fp = self.is_fp(ValueId(vid));
    let reg = self.pool_mut(fp).val_to_reg[&vid];

    if liveness.is_live_out_raw(local_idx, vid) {
      let slot = self.spill_slot(vid);

      result.spill_ops.push(SpillOp {
        insn_idx,
        timing: EmitTiming::Before,
        kind: SpillKind::Store {
          reg,
          slot,
          class: if fp {
            RegisterClass::FP
          } else {
            RegisterClass::GP
          },
        },
      });
    }

    let pool = self.pool_mut(fp);

    pool.val_to_reg.remove(&vid);
    pool.reg_to_val.remove(&reg);
    self.is_fp_value.remove(&vid);

    reg
  }

  fn alloc_or_spill(
    &mut self,
    insn_idx: usize,
    liveness: &LivenessInfo,
    local_idx: usize,
    result: &mut RegAlloc,
    fp: bool,
  ) -> u8 {
    let pool = self.pool_mut(fp);

    if let Some(reg) = pool.alloc_free() {
      return reg;
    }

    // Spill from the correct pool. Walk `allocatable` in
    // its declared order and pick the first reg that is
    // currently bound — picking from `val_to_reg.keys()`
    // was non-deterministic (HashMap iteration order is
    // not stable across runs), which made the emitted
    // codegen output flaky between builds. The heuristic
    // here ("oldest allocatable reg in the static list")
    // is not optimal; correctness — i.e. reproducible
    // binaries — is the goal of this fix.
    let victim = pool
      .allocatable
      .iter()
      .find_map(|r| pool.reg_to_val.get(r).copied())
      .expect("register pool exhausted");

    self.evict(victim, insn_idx, liveness, local_idx, result)
  }

  fn clear_all(&mut self) {
    self.gp.clear(&ALLOCATABLE_GP);
    self.fp.clear(&ALLOCATABLE_FP);
    self.is_fp_value.clear();
  }
}

/// Read-only inputs threaded through `allocate_function`.
/// Bundled into a context struct so the allocator entry
/// point stays at two arguments — input + output — rather
/// than tripping clippy's `too_many_arguments` lint with
/// six separate immutable borrows.
pub struct AllocCtx<'a> {
  /// Whole-program SIR.
  pub insns: &'a [Insn],
  /// First insn index of the function body (a `FunDef`).
  pub start: usize,
  /// One past the last insn index of the function body.
  pub end: usize,
  /// `ValueId.0` produced at each insn position, sparse
  /// for insns that don't define a value.
  pub value_ids: &'a [Option<ValueId>],
  /// Total `ValueId` count across the whole program —
  /// liveness uses it to size its bitsets.
  pub num_values: u32,
  /// Interner for resolving function and runtime symbol
  /// names referenced by `Call` / extern bookkeeping.
  pub interner: &'a Interner,
  /// `(name, owning_pack) → struct-return field count` map,
  /// built once per program by `build_struct_return_map`.
  /// Used by the `Call` arm to budget caller frame slots
  /// without re-scanning the whole SIR per call.
  pub struct_return_fns: &'a HashMap<FnKey, u32>,
  /// Struct element type of a `Vec` access, keyed by the
  /// `Call`'s `ValueId` (`Sir::vec_elem_tys`). The `Vec`
  /// budget arm reads it to size the struct scratch.
  pub vec_elem_tys: &'a std::collections::HashMap<u32, TyId>,
  /// Type tables, for sizing struct element scratch.
  pub type_view: Option<(&'a [Ty], &'a TyTable)>,
}

/// Run the forward allocation pass for a single function.
///
/// `ctx.insns[ctx.start..ctx.end]` is the function body
/// (FunDef to next FunDef / end). Results are merged into
/// `result`.
pub fn allocate_function(ctx: &AllocCtx<'_>, result: &mut RegAlloc) {
  let AllocCtx {
    insns,
    start,
    end,
    value_ids,
    num_values,
    interner,
    struct_return_fns,
    vec_elem_tys,
    type_view,
  } = *ctx;
  let n = end - start;

  if n == 0 {
    return;
  }

  // Extract params.
  let (params, fn_kind) = match &insns[start] {
    Insn::FunDef { params, kind, .. } => (params.clone(), *kind),
    _ => return,
  };

  if fn_kind == FunctionKind::Intrinsic {
    return;
  }

  // Liveness analysis.
  let liveness = liveness::analyze(insns, start, end, value_ids, num_values);

  let mut state = AllocState::new();
  let mut has_calls = false;

  // Reserve parameter registers in the correct pool.
  let n_params = params.len().min(8);

  for (i, (_name, ty_id)) in params.iter().enumerate().take(n_params) {
    let is_fp = ty_id.0 >= 15 && ty_id.0 <= 17;

    if is_fp {
      state.fp.free.retain(|&r| r != i as u8);
    } else {
      state.gp.free.retain(|&r| r != i as u8);
    }
  }

  for i in 0..n {
    let gi = start + i;
    let insn = &insns[gi];
    let fp = insn_is_fp(insn);

    // --- Handle Load (parameter or mutable) ---
    if let Insn::Load { dst, src, .. } = insn {
      // Snapshot BEFORE allocation — captures the register
      // state the codegen sees when emission begins for this
      // instruction. The result register is inserted into
      // this snapshot after allocation so `alloc_reg(dst)`
      // finds it without a flat-map fallback.
      snapshot_state(&state, gi, result);

      match src {
        LoadSource::Param(_) => {
          let reg = state.alloc_or_spill(gi, &liveness, i, result, fp);

          state.assign(*dst, reg, fp);
          insert_assignment(result, start as u32, *dst, reg, fp);
          snapshot_result(result, gi, *dst, reg, fp);
        }
        LoadSource::Local(_) => {
          let reg = state.alloc_or_spill(gi, &liveness, i, result, fp);

          state.assign(*dst, reg, fp);
          insert_assignment(result, start as u32, *dst, reg, fp);
          snapshot_result(result, gi, *dst, reg, fp);
        }
      }

      free_dead(&mut state, &liveness, i);

      continue;
    }

    // Every `ArrayLiteral` heap-allocates via `_malloc`
    // (both empty and non-empty paths — `[]T` is dynamic
    // by type, so stack-allocating non-empty literals
    // breaks `arr.push`'s `_realloc` on a stack pointer);
    // `ArrayPush` itself may also call `_realloc` when
    // capacity is exhausted. Either makes the function
    // non-leaf and forces FP/LR save in the prologue.
    if matches!(insn, Insn::ArrayLiteral { .. })
      || matches!(insn, Insn::ArrayPush { .. })
    {
      has_calls = true;
    }

    // Concurrency insns all lower to `BL` into the
    // runtime. The function needs a full non-leaf
    // prologue (FP/LR save + caller-save reserve)
    // or the emitted caller-save `STR`s land in
    // garbage memory and the return address gets
    // clobbered.
    if matches!(
      insn,
      Insn::ChannelCreate { .. }
        | Insn::ChannelSend { .. }
        | Insn::ChannelRecv { .. }
        | Insn::ChannelClose { .. }
        | Insn::TaskSpawn { .. }
        | Insn::TaskAwait { .. }
        | Insn::TaskCancelled { .. }
        | Insn::TaskCancel { .. }
        | Insn::NurseryEnd { .. }
        | Insn::SelectWait { .. }
        | Insn::StrSlice { .. }
        | Insn::ToStr { .. }
        | Insn::StringFormat { .. }
        // `CoerceToDyn` lowers to `BL zo_dyn_box`;
        // `DynDispatch` lowers to `BLR x16` through a
        // vtable slot. Both clobber X30 — the function
        // must save FP/LR in its prologue or the
        // surrounding `ret` returns to whatever happened
        // to land in X30 (often itself → infinite loop).
        | Insn::CoerceToDyn { .. }
        | Insn::DynDispatch { .. }
        | Insn::TestBegin { .. }
        | Insn::TestRun { .. }
        | Insn::TestSummary
    ) {
      has_calls = true;
    }

    // `Insn::BinOp` on `Str` operands lowers to runtime
    // calls — `_memcmp` for `Eq`/`Neq`, `zo_str_concat`
    // for `Concat` (see arm codegen). Without this gate,
    // the leaf-frame skips the caller-save reserve and
    // the emitted spills overwrite the function's own
    // stack — crashes any function whose body compares
    // or concatenates `str` values.
    if let Insn::BinOp { ty_id, op, .. } = insn
      && ty_id.0 == 4
      && matches!(
        op,
        zo_sir::BinOp::Eq | zo_sir::BinOp::Neq | zo_sir::BinOp::Concat
      )
    {
      has_calls = true;
    }

    // --- Handle Call (clobbers all caller-saved) ---
    // `CallIndirect` is a call through a pointer value — same
    // caller-save clobber discipline; its `callee` is reloaded
    // from a spill slot by codegen via the pre-call snapshot,
    // just like the direct call's args.
    if matches!(insn, Insn::Call { .. } | Insn::CallIndirect { .. }) {
      // Snapshot the pre-call register state. The codegen's
      // staging code for overflow args (>8) and the arg-move
      // loop both call `alloc_reg` at this instruction's
      // index. The physical registers still hold their
      // values at that point — spill Stores write to stack
      // but don't clear registers.
      snapshot_state(&state, gi, result);

      has_calls = true;

      // Collect values to save (both GP and FP). Both
      // `gp_save` and `fp_save` get sorted in place before
      // their reload loops so non-result-reg originals claim
      // their reg before result-reg originals (X0 / D0) run
      // `alloc_free` (which would otherwise pop the same
      // reg). Spill loop is order-independent.
      let mut gp_save = state
        .gp
        .val_to_reg
        .iter()
        .filter(|(vid, _)| liveness.is_live_out_raw(i, **vid))
        .map(|(vid, reg)| (*vid, *reg))
        .collect::<Vec<_>>();

      let mut fp_save = state
        .fp
        .val_to_reg
        .iter()
        .filter(|(vid, _)| liveness.is_live_out_raw(i, **vid))
        .map(|(vid, reg)| (*vid, *reg))
        .collect::<Vec<_>>();

      for &(vid, reg) in &gp_save {
        let slot = state.spill_slot(vid);

        result.spill_ops.push(SpillOp {
          insn_idx: gi,
          timing: EmitTiming::Before,
          kind: SpillKind::Store {
            reg,
            slot,
            class: RegisterClass::GP,
          },
        });
      }

      for &(vid, reg) in &fp_save {
        let slot = state.spill_slot(vid);

        result.spill_ops.push(SpillOp {
          insn_idx: gi,
          timing: EmitTiming::Before,
          kind: SpillKind::Store {
            reg,
            slot,
            class: RegisterClass::FP,
          },
        });
      }

      state.clear_all();

      // Call result goes to X0 (GP) or D0 (FP).
      let result_fp = insn_is_fp(insn);

      if let Some(vid) = value_ids[gi] {
        let reg = 0; // X0 or D0

        state.assign(vid, reg, result_fp);
        insert_assignment(result, start as u32, vid, reg, result_fp);
        snapshot_result(result, gi, vid, reg, result_fp);
      }

      // Reload saved values into the SAME register they
      // occupied before the call. Reloading into the
      // original register keeps the next instruction's
      // snapshot consistent — UNLESS the original is the
      // call-result register (X0 / D0), now holding THIS
      // call's result. Reloading into it would clobber that
      // result. A struct literal with two float-returning
      // call initializers (`S { a = f(), b = g() }`) is the
      // trigger: `f()`'s result spills out of D0, then the
      // D0-original reload must land in a fresh FP register
      // so `g()`'s result survives in D0 for its own field
      // store. The GP path already does this for X0; the FP
      // path must match or both fields read the same value.
      if gi + 1 < end {
        gp_save.sort_by_key(|(_, reg)| u8::from(*reg == 0));

        for &(vid, orig_reg) in &gp_save {
          let slot = state.spill_slots[&vid];

          let reload_reg = if orig_reg == 0 {
            state.gp.alloc_free().expect("out of GP regs for reload")
          } else {
            state.gp.free.retain(|&r| r != orig_reg);
            orig_reg
          };

          result.spill_ops.push(SpillOp {
            insn_idx: gi + 1,
            timing: EmitTiming::Before,
            kind: SpillKind::Load {
              reg: reload_reg,
              slot,
              class: RegisterClass::GP,
              vid,
            },
          });

          state.assign(ValueId(vid), reload_reg, false);
          insert_assignment(
            result,
            start as u32,
            ValueId(vid),
            reload_reg,
            false,
          );
        }

        fp_save.sort_by_key(|(_, reg)| u8::from(*reg == 0));

        for &(vid, orig_reg) in &fp_save {
          let slot = state.spill_slots[&vid];

          let reload_reg = if orig_reg == 0 {
            state.fp.alloc_free().expect("out of FP regs for reload")
          } else {
            state.fp.free.retain(|&r| r != orig_reg);
            orig_reg
          };

          result.spill_ops.push(SpillOp {
            insn_idx: gi + 1,
            timing: EmitTiming::Before,
            kind: SpillKind::Load {
              reg: reload_reg,
              slot,
              class: RegisterClass::FP,
              vid,
            },
          });

          state.assign(ValueId(vid), reload_reg, true);
          insert_assignment(
            result,
            start as u32,
            ValueId(vid),
            reload_reg,
            true,
          );
        }
      }

      free_dead(&mut state, &liveness, i);

      continue;
    }

    // --- General case ---

    // Pass 1: reload spilled uses into registers. The
    // two-pass split is load-of-bearing — the second
    // pass below must not free any value the first pass
    // is still about to reload.
    zo_liveness::visit_uses(insn, |use_vid| {
      if use_vid.0 == u32::MAX {
        return;
      }

      if state.get(use_vid).is_some() {
        return;
      }

      if let Some(&slot) = state.spill_slots.get(&use_vid.0) {
        let ufp = state.is_fp(use_vid);
        let reg = state.alloc_or_spill(gi, &liveness, i, result, ufp);

        result.spill_ops.push(SpillOp {
          insn_idx: gi,
          timing: EmitTiming::Before,
          kind: SpillKind::Load {
            reg,
            slot,
            class: if ufp {
              RegisterClass::FP
            } else {
              RegisterClass::GP
            },
            vid: use_vid.0,
          },
        });

        state.assign(use_vid, reg, ufp);
        insert_assignment(result, start as u32, use_vid, reg, ufp);
      }
    });

    // Snapshot after reloads, before freeing dead uses.
    // All uses are in registers, all live values visible.
    // Result is inserted below after allocation.
    snapshot_state(&state, gi, result);

    // Pass 2: free uses that are not live past this insn.
    zo_liveness::visit_uses(insn, |use_vid| {
      if use_vid.0 == u32::MAX {
        return;
      }

      if !liveness.is_live_out_raw(i, use_vid.0) {
        state.free_value(use_vid);
      }
    });

    if let Some(vid) = value_ids[gi] {
      let reg = state.alloc_or_spill(gi, &liveness, i, result, fp);

      state.assign(vid, reg, fp);
      insert_assignment(result, start as u32, vid, reg, fp);
      snapshot_result(result, gi, vid, reg, fp);
    }

    free_dead(&mut state, &liveness, i);
  }

  // Record per-function info.
  let spill_count = state.next_spill;
  let spill_size = (spill_count * 8 + 15) & !15;

  // Compute total struct/enum allocation space needed.
  let mut struct_slots: u32 = 0;

  for insn in &insns[start..end] {
    match insn {
      Insn::StructConstruct { fields, .. } => {
        struct_slots += fields.len() as u32;
      }
      Insn::EnumConstruct { fields, .. } => {
        // Every enum construction reserves `1 + fields.len()`
        // slots: `[tag, f0, f1, ...]`. Unit variants still get
        // a single slot for the tag so the codegen can return
        // a stable pointer (uniform pointer representation,
        // ZO-CL08). Missing this previously silently corrupted
        // the frame when a function had >0 unit enum
        // constructions — the prologue reserved too little
        // stack, and the unit-variant store walked past the
        // end into the caller's frame.
        struct_slots += 1 + fields.len() as u32;
      }
      Insn::ArrayLiteral { elements, .. } => {
        if elements.is_empty() {
          // Empty arrays are heap-allocated via malloc.
          // Only 1 stack slot for the pointer.
          struct_slots += 1;
        } else {
          // Non-empty literals: header + elements on stack.
          struct_slots += 2 + elements.len() as u32;
        }
      }
      Insn::TupleLiteral { elements, .. } => {
        struct_slots += elements.len() as u32;
      }
      // IO ext functions need extra stack slots for
      // syscall buffers and Result construction.
      //
      // `read_file` / `readln` / `read` produce a
      // `Result<str, int>` and read into a 4096-byte
      // buffer. Per-call allocation of buffer + Result
      // would be `~520 slots * call_count` — 5 calls
      // burn 20 KB. Codegen heap-copies the str payload
      // via `zo_str_alloc` and reuses one shared
      // 4104-byte buffer per function, so each call
      // only needs the small 3-slot Result frame
      // (`tag + ptr + scratch`). The shared buffer is
      // counted once below the per-insn loop.
      //
      // `write_file` / `append_file`: 5 slots for
      // Result. Other Call variants check for
      // struct-returning callees in the catch-all.
      Insn::Call {
        name,
        dst,
        callee_pack,
        ..
      } => {
        let fn_name = interner.get(*name);

        // A struct element needs more scratch than the scalar
        // budget below. These are upper bounds —
        // `flat_struct_slots_of` covers the live layout plus
        // the flatten save slots — so they never under-reserve
        // against codegen's `next_struct_slot` bumps in
        // `emit_vec_*`, which would corrupt the frame.
        let struct_elem_dims = vec_elem_tys.get(&dst.0).and_then(|elem| {
          let (tys, tt) = type_view?;

          Some((
            struct_leaf_words(*elem, tys, tt),
            flat_struct_slots_of(*elem, tys, tt).unwrap_or(1),
          ))
        });

        match fn_name {
          "Vec::push" | "Vec::set" if struct_elem_dims.is_some() => {
            let (leaf, live) = struct_elem_dims.unwrap();

            struct_slots += leaf + live;
          }
          "Vec::get" | "Vec::pop" | "Vec::remove"
            if struct_elem_dims.is_some() =>
          {
            let (leaf, live) = struct_elem_dims.unwrap();

            struct_slots += leaf + 2 + live;
          }
          "read_file" | "readln" | "read" => {
            struct_slots += crate::IO_RESULT_FRAME_SLOTS;
          }
          "write_file" | "append_file" => {
            struct_slots += 5;
          }

          // HashMap apply-method codegen handlers in
          // `zo-codegen-arm` allocate scratch slots for
          // key / value byte buffers and the
          // Option<V> aggregate. Budget must match
          // `next_struct_slot` bumps in `emit_map_*` —
          // mismatched slots overlap with the
          // caller-save area and corrupt restored
          // registers, hanging the program.
          "HashMap::new" => struct_slots += 1,
          "HashMap::insert" => struct_slots += 2,
          "HashMap::get" => struct_slots += 4,
          "HashMap::contains_key" => struct_slots += 1,
          "HashMap::remove" => struct_slots += 4,

          // Vec apply-method scratch budgets. Mirror the
          // bumps in `emit_vec_*` exactly — mismatched
          // slots overlap with caller-save area and corrupt
          // restored registers.
          "Vec::new" => struct_slots += 1,
          "Vec::push" => struct_slots += 1,
          "Vec::pop" => struct_slots += 3,
          "Vec::get" => struct_slots += 3,
          "Vec::set" => struct_slots += 1,
          "Vec::remove" => struct_slots += 3,

          // HashSet apply-method scratch budgets.
          "HashSet::new" => struct_slots += 1,
          "HashSet::insert" => struct_slots += 2,
          "HashSet::contains" => struct_slots += 1,
          "HashSet::remove" => struct_slots += 1,

          _ => {
            // Look up the callee in the pre-computed
            // struct-return map (one linear scan over
            // the SIR happens once in `RegAlloc::allocate`,
            // before any function is processed). The
            // previous per-call full-SIR scan was
            // O(calls × insns) and dominated codegen
            // time on programs with many small calls.
            if let Some(fields) = struct_return_fns.get(&(*name, *callee_pack))
            {
              struct_slots += *fields;
            }
          }
        }
      }
      _ => {}
    }
  }

  // Shared read buffer reserved once per function if any
  // `read_file` / `readln` / `read` call is present.
  // Codegen reuses the same offset across all such calls;
  // the str payload is heap-copied to make the buffer
  // safe to overwrite.
  let has_io_read = insns[start..end].iter().any(|insn| {
    if let Insn::Call { name, .. } = insn {
      let n = interner.get(*name);

      matches!(n, "read_file" | "readln" | "read")
    } else {
      false
    }
  });

  if has_io_read {
    struct_slots += crate::IO_SHARED_BUF_SLOTS;
  }

  let struct_size = (struct_slots * 8 + 15) & !15;

  // Count Store-target slots. Scalar variables take one
  // 8-byte slot; `[N]T` variables get an inline block of
  // `(2 + N) * 8` so codegen can memcpy on assignment
  // (otherwise `row = next` aliased the source's literal
  // block — both names walked the same memory).
  //
  // `array_sizes` is built from earlier `Insn::ArrayTyDef`
  // emissions; codegen does the same scan in its own
  // pre-pass.
  let mut array_sizes: HashMap<u32, u32> = HashMap::default();

  for i in 0..n {
    if let Insn::ArrayTyDef {
      array_ty,
      size: Some(sz),
      ..
    } = &insns[start + i]
    {
      array_sizes.insert(array_ty.0, *sz);
    }
  }

  let mut store_names: Vec<Symbol> = Vec::new();
  let mut mutable_slots: u32 = 0;

  for i in 0..n {
    if let Insn::Store { name, ty_id, .. } = &insns[start + i]
      && !store_names.contains(name)
    {
      store_names.push(*name);

      let slots = array_sizes.get(&ty_id.0).map(|sz| 2 + sz).unwrap_or(1);

      mutable_slots += slots;
    }
  }

  let mutable_size = (mutable_slots * 8 + 15) & !15;

  // `ChannelSend` stores the value on stack before the
  // FFI call reads it by pointer; `ChannelRecv` reads
  // the result the same way. A single 16-byte slot per
  // function covers both (one channel op is in flight
  // at a time per function), and 16 keeps the frame's
  // 16-byte alignment invariant. Zero when the function
  // contains no channel ops.
  let has_channel_op = (0..n).any(|i| {
    matches!(
      &insns[start + i],
      Insn::ChannelSend { .. } | Insn::ChannelRecv { .. }
    )
  });
  let chan_scratch_size = if has_channel_op { 16 } else { 0 };

  // `SelectWait` needs an on-stack `*mut ZoChan` array
  // (`nchans * 8` bytes) plus the runtime's output
  // buffer (`elem_sz` bytes). Each select in the
  // function contributes its own worst-case size; we
  // reserve the max so multiple selects reuse the same
  // frame region. 16-byte aligned to keep the frame's
  // alignment invariant. Zero when there are no selects.
  let mut select_scratch_size = 0u32;
  let mut string_format_scratch_size = 0u32;

  for i in 0..n {
    if let Insn::SelectWait { chans, .. } = &insns[start + i] {
      let nchans = chans.len() as u32;
      // Bound the element buffer at 8 bytes — the widest
      // scalar / pointer element this backend emits.
      // Wider-payload channels are a later scope.
      let want = nchans * 8 + 8;
      let aligned = (want + 15) & !15;

      if aligned > select_scratch_size {
        select_scratch_size = aligned;
      }
    }

    if let Insn::StringFormat { segments, .. } = &insns[start + i] {
      let want = segments.len() as u32 * 8;
      let aligned = (want + 15) & !15;

      if aligned > string_format_scratch_size {
        string_format_scratch_size = aligned;
      }
    }
  }

  result.function_info.insert(
    start,
    FunctionInfo {
      has_calls,
      spill_count,
      spill_size,
      struct_size,
      mutable_size,
      chan_scratch_size,
      select_scratch_size,
      string_format_scratch_size,
    },
  );
}

/// Free all active values that are NOT in live_out.
fn free_dead(
  state: &mut AllocState,
  liveness: &LivenessInfo,
  local_idx: usize,
) {
  let dead = state
    .gp
    .val_to_reg
    .keys()
    .chain(state.fp.val_to_reg.keys())
    .filter(|vid| !liveness.is_live_out_raw(local_idx, **vid))
    .copied()
    .collect::<Vec<_>>();

  for vid in dead {
    state.free_value(ValueId(vid));
  }
}

/// Determine if a SIR instruction produces a float value.
///
/// `ty_id` on `Insn::BinOp` is the OPERAND type — for
/// comparison operators (`<`, `<=`, `>`, `>=`, `==`, `!=`)
/// the operands may be float but the RESULT is always
/// bool (GP). Misclassifying `%r = lt %fa, %fb` as FP
/// puts the boolean result into an FP register, while
/// the codegen's CSEL writes it to a GP — consumers then
/// read a stale FP value. Explicitly carve comparisons
/// out so they allocate into the GP pool.
fn insn_is_fp(insn: &Insn) -> bool {
  match insn {
    Insn::ConstFloat { .. } => true,
    Insn::BinOp { ty_id, op, .. } => {
      let is_cmp = matches!(
        op,
        zo_sir::BinOp::Lt
          | zo_sir::BinOp::Lte
          | zo_sir::BinOp::Gt
          | zo_sir::BinOp::Gte
          | zo_sir::BinOp::Eq
          | zo_sir::BinOp::Neq
      );

      !is_cmp && ty_id.0 >= 15 && ty_id.0 <= 17
    }
    Insn::UnOp { ty_id, .. }
    | Insn::Load { ty_id, .. }
    | Insn::Call { ty_id, .. }
    | Insn::CallIndirect { ty_id, .. }
    | Insn::ArrayIndex { ty_id, .. }
    | Insn::TupleIndex { ty_id, .. } => ty_id.0 >= 15 && ty_id.0 <= 17,
    Insn::Cast { to_ty, .. } => to_ty.0 >= 15 && to_ty.0 <= 17,
    _ => false,
  }
}

/// Snapshot the current register state into the per-
/// instruction maps. Called after each instruction is
/// fully processed so the codegen sees the correct
/// register for every value at every instruction point.
fn snapshot_state(
  state: &AllocState,
  global_idx: usize,
  result: &mut RegAlloc,
) {
  if global_idx < result.insn_gp.len() {
    result.insn_gp[global_idx] = state.gp.val_to_reg.clone();
  }

  if global_idx < result.insn_fp.len() {
    result.insn_fp[global_idx] = state.fp.val_to_reg.clone();
  }
}

/// Update the flat map fallback (GP or FP).
fn insert_assignment(
  result: &mut RegAlloc,
  fn_start: u32,
  vid: ValueId,
  reg: u8,
  fp: bool,
) {
  if fp {
    result.fp_assignments.insert((fn_start, vid.0), reg);
  } else {
    result.assignments.insert((fn_start, vid.0), reg);
  }
}

/// Insert a result value into an already-taken snapshot.
/// Called after `snapshot_state` so the instruction's own
/// result register is visible to `get_at` alongside the
/// pre-existing uses and live values.
fn snapshot_result(
  result: &mut RegAlloc,
  global_idx: usize,
  vid: ValueId,
  reg: u8,
  fp: bool,
) {
  let map = if fp {
    &mut result.insn_fp
  } else {
    &mut result.insn_gp
  };

  if global_idx < map.len() {
    // Evict any stale vid that occupied this register
    // before it was freed and reallocated. Without this,
    // the snapshot contains two vids on the same register
    // — a dead use and the new result — which is a
    // logical conflict even if benign at runtime.
    map[global_idx].retain(|_, r| *r != reg);
    map[global_idx].insert(vid.0, reg);
  }
}
