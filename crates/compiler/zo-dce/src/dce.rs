use zo_error::{Error, ErrorKind};
use zo_interner::Symbol;
use zo_liveness::{compute_value_ids, insn_def, visit_uses};
use zo_reporter::rationale::report_rationale;
use zo_reporter::report_error;
use zo_sir::{Insn, Sir};
use zo_span::Span;
use zo_value::{Pubness, ValueId};

use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use smallvec::SmallVec;

/// Function range in the SIR instruction stream.
struct FunRange {
  name: Symbol,
  /// Index of the `FunDef` instruction.
  start: usize,
  /// Index of the terminating `Return` instruction.
  end: usize,
  /// Whether the function is `pub` (exported).
  pubness: Pubness,
  /// Pack the function belongs to. `None` for items at the
  /// crate root (the file being compiled). DCE uses this to
  /// distinguish "user's `pub` API" (rooted — might be
  /// reached via FFI / dyn-dispatch the call graph can't
  /// see) from "loaded pack's `pub` items" (only kept when
  /// transitively reachable from `main` or another root).
  owning_pack: Option<Symbol>,
}

/// Dead code elimination pipeline.
///
/// Runs three passes in order:
///   1. Dead function elimination (interprocedural).
///   2. Unreachable code after `return` (intraprocedural).
///   3. Dead instruction elimination (liveness-based, fixpoint).
pub struct Dce<'a> {
  sir: &'a mut Sir,
  roots: Vec<Symbol>,
  interner: &'a zo_interner::Interner,
}

impl<'a> Dce<'a> {
  /// Creates a new DCE pipeline. `roots` contains every
  /// symbol that must survive elimination — `main`,
  /// abstract-impl methods, test functions, etc.
  pub fn new(
    sir: &'a mut Sir,
    roots: Vec<Symbol>,
    interner: &'a zo_interner::Interner,
  ) -> Self {
    Self {
      sir,
      roots,
      interner,
    }
  }

  /// Runs the full DCE pipeline.
  pub fn eliminate(&mut self) {
    self.eliminate_dead_functions();
    self.eliminate_unreachable_after_return();
    // TODO: dead variable elimination disabled.
    // self.eliminate_dead_variables();
    self.eliminate_dead_instructions();
  }

  // ==============================================================
  // Pass 1: Dead function elimination (interprocedural).
  // ==============================================================

  /// Builds a call graph from function bodies, then walks from
  /// roots (`main` + pub) via a worklist to find all
  /// transitively reachable functions. Removes the rest.
  fn eliminate_dead_functions(&mut self) {
    if self.sir.instructions.is_empty() {
      return;
    }

    let functions = build_function_map(&self.sir.instructions);

    if functions.is_empty() {
      return;
    }

    let event_handlers =
      collect_event_handler_syms(&self.sir.instructions, self.interner);

    let reachable = mark_reachable(
      &functions,
      &self.sir.instructions,
      &self.roots,
      &event_handlers,
    );

    let dead = functions
      .iter()
      .filter(|f| !reachable.contains(&f.name))
      .collect::<Vec<_>>();

    // Rationale: emit one `severity: "note"` entry per
    // eliminated function pointing at its `fun` introducer.
    // No-op when `--explain-decisions` is off — the gate in
    // `report_rationale` short-circuits before any work.
    //
    // Filter to the user's entry file: skip pack-owned
    // functions (preload internals like `core::io::*` that
    // legitimately fall out under DCE but aren't the
    // user's dead code) and skip synthetic functions (the
    // `Type::default()` ctors emitted by `apply` blocks
    // with `Span::ZERO`). Without this filter, a trivial
    // entry program produces dozens of notes about preload
    // pack internals the user never typed.
    for f in &dead {
      if f.owning_pack.is_some() {
        continue;
      }
      let span = match self.sir.instructions.get(f.start) {
        Some(Insn::FunDef { span, .. }) if *span != Span::ZERO => *span,
        _ => continue,
      };
      report_rationale(Error::new(ErrorKind::DeadCodeEliminated, span));
    }

    let mut dead_ranges =
      dead.iter().map(|f| (f.start, f.end)).collect::<Vec<_>>();

    dead_ranges.sort_by_key(|r| std::cmp::Reverse(r.0));

    for (start, end) in dead_ranges {
      if end < self.sir.instructions.len() {
        self.sir.instructions.drain(start..=end);
        self.sir.spans.drain(start..=end);
      }
    }
  }

  // ==============================================================
  // Pass 2: Unreachable code after return.
  // ==============================================================

  /// Removes instructions between a `Return` and the next
  /// `Label` or `FunDef`. Single linear scan, O(N).
  fn eliminate_unreachable_after_return(&mut self) {
    // Mark unreachable instructions in one pass, then compact
    // with a single `retain`. Removing inline with
    // `Vec::remove` memmoves the tail per dead instruction —
    // O(dead × len) — which is catastrophic on large programs.
    let mut keep = Vec::with_capacity(self.sir.instructions.len());
    let mut in_dead_zone = false;
    let mut any_dead = false;

    for insn in &self.sir.instructions {
      if in_dead_zone {
        match insn {
          Insn::Label { .. }
          | Insn::FunDef { .. }
          | Insn::StructDef { .. }
          | Insn::EnumDef { .. }
          | Insn::ConstDef { .. }
          | Insn::ArrayTyDef { .. }
          | Insn::MapTyDef { .. }
          // Pack-level metadata (`pack X;`, `#link {...}`)
          // are top-level declarations, never dead. After
          // module-merge they sit between a preload
          // function's Return and the user's first
          // FunDef — exiting the dead-zone here keeps
          // them in the SIR so the codegen FFI pre-pass
          // can associate every `pub ffi` with its
          // declaring pack's `#link` metadata.
          | Insn::PackDecl { .. }
          | Insn::PackLink { .. } => {
            in_dead_zone = false;
            keep.push(true);
          }
          _ => {
            keep.push(false);
            any_dead = true;
          }
        }
      } else {
        if matches!(insn, Insn::Return { .. }) {
          in_dead_zone = true;
        }

        keep.push(true);
      }
    }

    if any_dead {
      let mut i = 0;
      self.sir.instructions.retain(|_| {
        let k = keep[i];
        i += 1;
        k
      });

      // Keep `spans` aligned 1:1 with `instructions`.
      let mut j = 0;
      self.sir.spans.retain(|_| {
        let k = keep[j];
        j += 1;
        k
      });
    }
  }

  // ==============================================================
  // Pass 3: Dead variable (Store) elimination (liveness-based).
  // ==============================================================

  /// Eliminates dead `Store` instructions.
  ///
  /// A `Store { name }` is dead if the named variable is not
  /// live-out at that instruction — meaning no path from that
  /// point ever reads the stored value before it's overwritten
  /// or the function exits.
  #[allow(dead_code)]
  fn eliminate_dead_variables(&mut self) {
    if self.sir.instructions.is_empty() {
      return;
    }

    let num_values = self.sir.next_value_id;

    loop {
      let functions = find_functions(&self.sir.instructions);

      if functions.is_empty() {
        return;
      }

      let value_ids = compute_value_ids(&self.sir.instructions);

      // Mark dead stores across every function, then compact
      // in a single `retain` pass — same O(dead × len) memmove
      // hazard as `eliminate_dead_instructions`.
      let mut dead_mask = vec![false; self.sir.instructions.len()];
      let mut any_removed = false;

      for &(start, end) in functions.iter().rev() {
        if start >= self.sir.instructions.len()
          || end > self.sir.instructions.len()
        {
          continue;
        }

        let fn_span = match &self.sir.instructions[start] {
          Insn::FunDef { span, .. } => *span,
          _ => Span::ZERO,
        };

        let liveness = zo_liveness::analyze(
          &self.sir.instructions,
          start,
          end,
          &value_ids,
          num_values,
        );

        for i in 0..(end - start) {
          let gi = start + i;

          if let Insn::Store { name, .. } = &self.sir.instructions[gi]
            && !liveness.is_var_live_out(i, *name)
          {
            report_error(Error::new(ErrorKind::UnusedVariable, fn_span));

            dead_mask[gi] = true;
            any_removed = true;
          }
        }
      }

      if any_removed {
        let mut gi = 0;
        self.sir.instructions.retain(|_| {
          let keep = !dead_mask[gi];
          gi += 1;
          keep
        });

        // Keep `spans` aligned 1:1 with `instructions`.
        let mut gj = 0;
        self.sir.spans.retain(|_| {
          let keep = !dead_mask[gj];
          gj += 1;
          keep
        });
      }

      if !any_removed {
        break;
      }
    }
  }

  // ==============================================================
  // Pass 4: Dead instruction elimination (liveness-based).
  // ==============================================================

  /// Eliminates dead instructions within each function body.
  ///
  /// A value-producing, pure instruction is dead when its
  /// result is never observed — not consumed (transitively)
  /// by any instruction that must execute.
  ///
  /// Optimistic mark-sweep (Wegman–Zadeck, §5.1): assume every
  /// value is dead, seed the instructions that must execute
  /// (`insn_is_critical`), then propagate liveness backward
  /// through operands exactly once via a worklist. A single
  /// `retain` sweep removes the unmarked. Strict O(N) — a
  /// dead-dependency chain of depth D collapses in one pass,
  /// where the prior fixed-point loop took D passes.
  fn eliminate_dead_instructions(&mut self) {
    if self.sir.instructions.is_empty() {
      return;
    }

    let functions = find_functions(&self.sir.instructions);

    if functions.is_empty() {
      return;
    }

    // One global mask, one allocation; the compaction is a
    // single linear `retain`. Marking is scoped per function
    // with a function-local def map — merged-module SIR reuses
    // ValueIds across functions (each module numbers its
    // values independently), so a whole-program def index would
    // misroute lookups to the wrong function.
    let mut dead = vec![false; self.sir.instructions.len()];

    for &(start, end) in &functions {
      if start >= self.sir.instructions.len()
        || end > self.sir.instructions.len()
      {
        continue;
      }

      mark_dead_in_function(&self.sir.instructions, start, end, &mut dead);
    }

    let mut gi = 0;

    self.sir.instructions.retain(|_| {
      let keep = !dead[gi];
      gi += 1;
      keep
    });

    // Keep `spans` aligned 1:1 with `instructions`.
    let mut gj = 0;

    self.sir.spans.retain(|_| {
      let keep = !dead[gj];
      gj += 1;
      keep
    });
  }
}

// ================================================================
// Helpers (module-level, stateless).
// ================================================================

/// An instruction that must execute regardless of whether its
/// result is read — the seed set for the optimistic mark-sweep.
///
/// Side-effecting instructions ([`is_impure`], which already
/// includes `Return` and `Directive`) plus the control-flow and
/// binding instructions that anchor liveness:
///   - `BranchIfNot` keeps its `cond` operand live.
///   - `VarDef` keeps its `init` operand live (the binding is
///     observable through later `Load`s of the same name).
///   - `Label` / `Jump` / `FunDef` are structural; seeding them
///     is harmless (they carry no value operands).
fn insn_is_critical(insn: &Insn) -> bool {
  is_impure(insn)
    || matches!(
      insn,
      Insn::Label { .. }
        | Insn::Jump { .. }
        | Insn::BranchIfNot { .. }
        | Insn::FunDef { .. }
        | Insn::VarDef { .. }
    )
}

/// Mark dead, value-producing, pure instructions in the
/// function body `[start, end)` into `dead`.
///
/// Optimistic worklist: seed critical instructions live, then
/// propagate liveness backward through operands.
///
/// Position-aware: a `find_functions` range can span a value
/// space boundary — trailing module-level `val` const-defs sit
/// between a function's `Return` and the next `FunDef`, and
/// merged-module SIR reuses ValueIds, so the same `ValueId` can
/// be defined twice in one range. Each use therefore resolves
/// to the nearest *preceding* definition (the dominating def in
/// linear order). Def positions live in a `SmallVec<[u32; 1]>`,
/// so the single-definition norm — the overwhelming majority —
/// stays inline and allocates nothing; only a genuine collision
/// spills to the heap. The full range is retained on purpose:
/// the dead trailing const-value insns must be swept here, or
/// codegen later sees the reused ValueId and aliases the body's
/// definition to the trailing constant's slot.
fn mark_dead_in_function(
  insns: &[Insn],
  start: usize,
  end: usize,
  dead: &mut [bool],
) {
  let len = end - start;
  let mut alive = vec![false; len];

  // `ValueId → ascending def positions (global indices)`. Built
  // by scanning in order, so each is already sorted. Single-def
  // values stay inline in the `SmallVec`, so the common case
  // allocates nothing.
  let mut defs: HashMap<ValueId, SmallVec<[u32; 1]>> = HashMap::default();

  for (offset, insn) in insns[start..end].iter().enumerate() {
    if let Some(dst) = insn_def(insn) {
      defs.entry(dst).or_default().push((start + offset) as u32);
    }
  }

  // Worklist of `(value, use position)`. The position picks the
  // dominating def when a ValueId is defined more than once.
  let mut work: Vec<(ValueId, usize)> = Vec::new();

  let push_uses =
    |insn: &Insn, pos: usize, work: &mut Vec<(ValueId, usize)>| {
      visit_uses(insn, |u| {
        if u.0 != u32::MAX {
          work.push((u, pos));
        }
      });
    };

  // Seed: every critical insn is live; enqueue its operands.
  for (offset, insn) in insns[start..end].iter().enumerate() {
    if insn_is_critical(insn) {
      alive[offset] = true;
      push_uses(insn, start + offset, &mut work);
    }
  }

  // Propagate: resolve each use to its dominating def, mark that
  // def live, enqueue its operands. Each insn flips false→true
  // at most once, so total work stays linear.
  while let Some((vid, use_pos)) = work.pop() {
    let Some(positions) = defs.get(&vid) else {
      continue;
    };

    let def_gi = resolve_def(positions, use_pos);
    let li = def_gi as usize - start;

    if !alive[li] {
      alive[li] = true;
      push_uses(&insns[def_gi as usize], def_gi as usize, &mut work);
    }
  }

  // Collect: value-producing, not critical, never marked.
  for (offset, insn) in insns[start..end].iter().enumerate() {
    if insn_def(insn).is_some() && !insn_is_critical(insn) && !alive[offset] {
      dead[start + offset] = true;
    }
  }
}

/// Pick the definition of a value that dominates a use at
/// `use_pos`: the largest def position `<= use_pos`. Falls back
/// to the earliest definition for the rare forward reference
/// (no preceding def — e.g. a value threaded around a loop).
/// `positions` is ascending.
#[inline]
fn resolve_def(positions: &[u32], use_pos: usize) -> u32 {
  let up = use_pos as u32;
  let idx = positions.partition_point(|&p| p <= up);

  if idx > 0 {
    positions[idx - 1]
  } else {
    positions[0]
  }
}

/// Returns true if an instruction has side effects.
///
/// `Template` and `StyleSheet` carry observable side effects —
/// they describe a UI command stream that the runtime renders
/// to the screen. DCE must not drop them even when their `id`
/// is transitively unused through liveness (e.g. a `VarDef`
/// init referencing a `Template`'s id can get pruned in its
/// own pass, leaving the Template looking dead; it is not).
fn is_impure(insn: &Insn) -> bool {
  matches!(
    insn,
    Insn::Call { .. }
      | Insn::Store { .. }
      | Insn::FieldStore { .. }
      | Insn::ArrayStore { .. }
      | Insn::ArrayPush { .. }
      | Insn::ArrayPop { .. }
      | Insn::Directive { .. }
      | Insn::Return { .. }
      | Insn::Template { .. }
      | Insn::StyleSheet { .. }
      | Insn::ArrayTyDef { .. }
      | Insn::MapTyDef { .. }
      // Concurrency insns have observable side effects:
      // channel enqueue/dequeue, task enqueue, scheduler
      // drain, selective wait on N channels. DCE must
      // keep them so their operands stay live.
      | Insn::ChannelCreate { .. }
      | Insn::ChannelSend { .. }
      | Insn::ChannelRecv { .. }
      | Insn::ChannelClose { .. }
      | Insn::TaskSpawn { .. }
      | Insn::TaskAwait { .. }
      | Insn::NurseryBegin { .. }
      | Insn::NurseryEnd { .. }
      | Insn::SelectWait { .. }
      | Insn::SelectRecv { .. }
      | Insn::TaskCancelled { .. }
      | Insn::TaskCancel { .. }
      | Insn::StrSlice { .. }
      | Insn::ToStr { .. }
      | Insn::StringFormat { .. }
      // `CoerceToDyn` heap-allocates via `_zo_dyn_box`; the
      // call is the side effect that must outlive liveness
      // even if the resulting fat-pointer isn't immediately
      // read in the same scope. `DynDispatch` is a real
      // call too — same pruning hazard.
      | Insn::CoerceToDyn { .. }
      | Insn::DynDispatch { .. }
      | Insn::TestBegin { .. }
      | Insn::TestRun { .. }
      | Insn::TestSummary
  )
}

/// Scans the instruction stream and pairs each `FunDef` with
/// its terminating `Return` to build function ranges.
fn build_function_map(instructions: &[Insn]) -> Vec<FunRange> {
  let mut functions = Vec::new();
  let mut i = 0;

  while i < instructions.len() {
    if let Insn::FunDef {
      name,
      pubness,
      owning_pack,
      ..
    } = &instructions[i]
    {
      let start = i;
      let mut end = i + 1;

      // Function bodies can hold multiple `Return` insns
      // (one per `match` / `if` arm + the implicit tail
      // return). They can also hold lazily-emitted type
      // metadata (`EnumDef`, `StructDef`, `ConstDef`,
      // `ArrayTyDef`, `MapTyDef`) that the executor flushes
      // at first-use of the corresponding type — e.g.
      // `match read_file(...)` materializes the
      // `Result<str,int>` `EnumDef` mid-body. Only `FunDef`
      // opens a NEW function; everything else stays in the
      // current body so its `Call` insns contribute to
      // reachability. Truncating earlier silently drops
      // callees and DCE then strips them from the SIR.
      //
      // We track `last_return` so the final range stops at
      // the function's last `Return` rather than spilling
      // into trailing module-scope metadata. Without this,
      // a dead intrinsic FFI (unused `pub ffi` after preload
      // expansion) drains every subsequent `StructDef` /
      // `EnumDef` until the next `FunDef` — including
      // user-file structs whose codegen then loses
      // `struct_metas[id]` and prints values as raw ints.
      let mut last_return = start;

      while end < instructions.len() {
        // Module-scope boundaries that aren't part of any
        // function body: another `FunDef`, or the
        // `PackDecl` / `PackLink` of the NEXT pack. Without
        // stopping at the latter, draining a dead function
        // also drains the next pack's metadata — codegen
        // then can't resolve the next pack's FFI dylib path
        // (`pack_dylib` ends up empty for that pack) and
        // every FFI call into it ends up as an unbound
        // symbol at link time.
        if matches!(
          &instructions[end],
          Insn::FunDef { .. } | Insn::PackDecl { .. } | Insn::PackLink { .. }
        ) {
          end -= 1;
          break;
        }

        if matches!(&instructions[end], Insn::Return { .. }) {
          last_return = end;
        }

        end += 1;
      }

      if end >= instructions.len() {
        end = instructions.len() - 1;
      }

      // Two trim cases:
      //   - body-less intrinsic FFI: no `Return` ever
      //     emitted (kind: Intrinsic, body_start: 0). The
      //     range walk above happily extends past the
      //     FunDef until the next FunDef/PackDecl, scooping
      //     up every trailing `StructDef`/`EnumDef`/etc.
      //     Clamp to `start` so a dead FFI only drains
      //     itself.
      //   - normal function with a real body: trim
      //     trailing module-scope metadata that landed
      //     between this function's last `Return` and the
      //     next FunDef — those decls belong to the NEXT
      //     scope, not this one.
      let trailing_is_metadata = |from: usize, to: usize| {
        (from..=to).all(|i| {
          matches!(
            &instructions[i],
            Insn::StructDef { .. }
              | Insn::EnumDef { .. }
              | Insn::ConstDef { .. }
              | Insn::ArrayTyDef { .. }
              | Insn::MapTyDef { .. }
          )
        })
      };

      if last_return == start
        && end > start
        && trailing_is_metadata(start + 1, end)
      {
        end = start;
      } else if last_return > start
        && end > last_return
        && trailing_is_metadata(last_return + 1, end)
      {
        end = last_return;
      }

      // Skip zero-body functions (intrinsic stubs).
      // Removing them would shift indices and break
      // codegen function offsets.
      if start < end {
        functions.push(FunRange {
          name: *name,
          start,
          end,
          pubness: *pubness,
          owning_pack: *owning_pack,
        });
      }

      i = end + 1;
    } else {
      i += 1;
    }
  }

  functions
}

/// Collects call targets from a slice of instructions.
fn collect_calls_in_range(
  instructions: &[Insn],
  start: usize,
  end: usize,
) -> Vec<Symbol> {
  let mut calls = Vec::new();

  for insn in &instructions[start..=end.min(instructions.len() - 1)] {
    match insn {
      Insn::Call { name, .. } => calls.push(*name),
      // `spawn fn()` captures `fn` by address so the
      // runtime can call it inside a green / OS task.
      // DCE must treat the callee as reachable or the
      // emitted binary would be missing its code.
      Insn::TaskSpawn { callee, .. } => calls.push(*callee),
      _ => {}
    }
  }

  calls
}

/// Marks functions reachable from roots via transitive call
/// graph walk (worklist algorithm).
fn mark_reachable(
  functions: &[FunRange],
  instructions: &[Insn],
  roots: &[Symbol],
  event_handlers: &HashSet<Symbol>,
) -> HashSet<Symbol> {
  let mut reachable = HashSet::default();
  let mut worklist = Vec::new();

  // Explicit roots — main, abstract-impl methods,
  // test functions, and anything else the driver pins.
  for &root in roots {
    worklist.push(root);
  }

  // Crate-root `pub` items and template event handlers
  // are implicit roots.
  for func in functions {
    if event_handlers.contains(&func.name)
      || (func.pubness == Pubness::Yes && func.owning_pack.is_none())
    {
      worklist.push(func.name);
    }
  }

  while let Some(name) = worklist.pop() {
    if !reachable.insert(name) {
      continue;
    }

    // Scan ALL entries with this name (there may be
    // duplicates: intrinsic stub + user-defined body).
    for func in functions.iter().filter(|f| f.name == name) {
      for callee in collect_calls_in_range(instructions, func.start, func.end) {
        if !reachable.contains(&callee) {
          worklist.push(callee);
        }
      }
    }
  }

  reachable
}

/// Collects handler function Symbols referenced by template
/// Event commands. These closures are called by the runtime
/// at event time, not by static code — they must survive DCE.
fn collect_event_handler_syms(
  instructions: &[Insn],
  interner: &zo_interner::Interner,
) -> HashSet<Symbol> {
  let mut handlers = HashSet::default();

  for insn in instructions {
    if let Insn::Template {
      commands, bindings, ..
    } = insn
    {
      for cmd in commands {
        if let zo_ui_protocol::UiCommand::Event { handler, .. } = cmd
          && let Some(sym) = interner.symbol(handler)
        {
          handlers.insert(sym);
        }
      }

      // Computed text bindings reference their closure
      // by symbol via a side-channel (not as `UiCommand::
      // Event`), so DCE wouldn't see them otherwise and
      // would drop the closure as unreachable.
      for (_, cb) in &bindings.computed {
        handlers.insert(cb.closure_name);
      }
    }
  }

  handlers
}

/// Helper: find non-intrinsic function ranges.
fn find_functions(insns: &[Insn]) -> Vec<(usize, usize)> {
  let mut positions = Vec::new();

  for (i, insn) in insns.iter().enumerate() {
    if let Insn::FunDef { kind, .. } = insn {
      positions.push((i, *kind));
    }
  }

  let mut result = Vec::new();

  for (j, &(start, kind)) in positions.iter().enumerate() {
    if kind == zo_value::FunctionKind::Intrinsic {
      continue;
    }

    let end = if j + 1 < positions.len() {
      positions[j + 1].0
    } else {
      insns.len()
    };

    result.push((start, end));
  }

  result
}
