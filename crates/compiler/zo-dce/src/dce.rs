use zo_error::{Error, ErrorKind};
use zo_interner::Symbol;
use zo_liveness::compute_value_ids;
use zo_reporter::report_error;
use zo_sir::{Insn, Sir};
use zo_span::Span;
use zo_value::Pubness;

use rustc_hash::FxHashSet as HashSet;

/// Function range in the SIR instruction stream.
struct FunRange {
  name: Symbol,
  /// Index of the `FunDef` instruction.
  start: usize,
  /// Index of the terminating `Return` instruction.
  end: usize,
  /// Whether the function is `pub` (exported).
  pubness: Pubness,
}

/// Dead code elimination pipeline.
///
/// Runs three passes in order:
///   1. Dead function elimination (interprocedural).
///   2. Unreachable code after `return` (intraprocedural).
///   3. Dead instruction elimination (liveness-based, fixpoint).
pub struct Dce<'a> {
  sir: &'a mut Sir,
  main_sym: Symbol,
  interner: &'a zo_interner::Interner,
}

impl<'a> Dce<'a> {
  /// Creates a new DCE pipeline for the given SIR.
  pub fn new(
    sir: &'a mut Sir,
    main_sym: Symbol,
    interner: &'a zo_interner::Interner,
  ) -> Self {
    Self {
      sir,
      main_sym,
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
      self.main_sym,
      &event_handlers,
    );

    let dead = functions
      .iter()
      .filter(|f| !reachable.contains(&f.name))
      .collect::<Vec<_>>();

    // TODO: report UnusedFunction warnings once zo-error has
    // Severity::Warning so warnings don't block compilation.
    // for _f in &dead {
    //   report_error(Error::new(
    //     ErrorKind::UnusedFunction,
    //     Span::ZERO,
    //   ));
    // }

    let mut dead_ranges =
      dead.iter().map(|f| (f.start, f.end)).collect::<Vec<_>>();

    dead_ranges.sort_by_key(|r| std::cmp::Reverse(r.0));

    for (start, end) in dead_ranges {
      if end < self.sir.instructions.len() {
        self.sir.instructions.drain(start..=end);
      }
    }
  }

  // ==============================================================
  // Pass 2: Unreachable code after return.
  // ==============================================================

  /// Removes instructions between a `Return` and the next
  /// `Label` or `FunDef`. Single linear scan, O(N).
  fn eliminate_unreachable_after_return(&mut self) {
    let mut i = 0;
    let mut in_dead_zone = false;

    while i < self.sir.instructions.len() {
      if in_dead_zone {
        match &self.sir.instructions[i] {
          Insn::Label { .. }
          | Insn::FunDef { .. }
          | Insn::StructDef { .. }
          | Insn::EnumDef { .. }
          | Insn::ConstDef { .. } => {
            in_dead_zone = false;
            i += 1;
          }
          _ => {
            self.sir.instructions.remove(i);
          }
        }
      } else {
        if matches!(&self.sir.instructions[i], Insn::Return { .. }) {
          in_dead_zone = true;
        }

        i += 1;
      }
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
      let mut any_removed = false;

      for &(start, end) in functions.iter().rev() {
        if start >= self.sir.instructions.len()
          || end > self.sir.instructions.len()
        {
          continue;
        }

        let liveness = zo_liveness::analyze(
          &self.sir.instructions,
          start,
          end,
          &value_ids,
          num_values,
        );

        let mut dead = Vec::new();

        for i in 0..(end - start) {
          let gi = start + i;

          if let Insn::Store { name, .. } = &self.sir.instructions[gi]
            && !liveness.is_var_live_out(i, *name)
          {
            report_error(Error::new(ErrorKind::UnusedVariable, Span::ZERO));

            dead.push(gi);
          }
        }

        dead.sort_unstable_by(|a, b| b.cmp(a));

        for idx in dead {
          self.sir.instructions.remove(idx);
          any_removed = true;
        }
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
  /// An instruction is dead if:
  ///   1. It defines a `ValueId` (has a `dst`).
  ///   2. That `ValueId` is not live-out at the instruction.
  ///   3. The instruction is pure (no side effects).
  ///
  /// Iterates to fixpoint — removing one instruction may make
  /// others dead.
  fn eliminate_dead_instructions(&mut self) {
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
      let mut any_removed = false;

      for &(start, end) in functions.iter().rev() {
        if start >= self.sir.instructions.len()
          || end > self.sir.instructions.len()
        {
          continue;
        }

        let liveness = zo_liveness::analyze(
          &self.sir.instructions,
          start,
          end,
          &value_ids,
          num_values,
        );

        let mut dead = Vec::new();

        for i in 0..(end - start) {
          let gi = start + i;
          let insn = &self.sir.instructions[gi];

          if is_impure(insn) {
            continue;
          }

          if matches!(
            insn,
            Insn::Label { .. }
              | Insn::Jump { .. }
              | Insn::BranchIfNot { .. }
              | Insn::FunDef { .. }
          ) {
            continue;
          }

          if let Some(vid) = value_ids[gi]
            && !liveness.live_out[i].test(vid.0 as usize)
          {
            dead.push(gi);
          }
        }

        dead.sort_unstable_by(|a, b| b.cmp(a));

        for idx in dead {
          self.sir.instructions.remove(idx);
          any_removed = true;
        }
      }

      if !any_removed {
        break;
      }
    }
  }
}

// ================================================================
// Helpers (module-level, stateless).
// ================================================================

/// Returns true if an instruction has side effects.
fn is_impure(insn: &Insn) -> bool {
  matches!(
    insn,
    Insn::Call { .. }
      | Insn::Store { .. }
      | Insn::FieldStore { .. }
      | Insn::ArrayStore { .. }
      | Insn::ArrayPush { .. }
      | Insn::Directive { .. }
      | Insn::Return { .. }
  )
}

/// Scans the instruction stream and pairs each `FunDef` with
/// its terminating `Return` to build function ranges.
fn build_function_map(instructions: &[Insn]) -> Vec<FunRange> {
  let mut functions = Vec::new();
  let mut i = 0;

  while i < instructions.len() {
    if let Insn::FunDef { name, pubness, .. } = &instructions[i] {
      let start = i;
      let mut end = i + 1;

      while end < instructions.len() {
        match &instructions[end] {
          Insn::Return { .. } => break,
          Insn::FunDef { .. }
          | Insn::StructDef { .. }
          | Insn::EnumDef { .. }
          | Insn::ConstDef { .. } => {
            end -= 1;
            break;
          }
          _ => end += 1,
        }
      }

      if end >= instructions.len() {
        end = instructions.len() - 1;
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
    if let Insn::Call { name, .. } = insn {
      calls.push(*name);
    }
  }

  calls
}

/// Marks functions reachable from roots via transitive call
/// graph walk (worklist algorithm).
fn mark_reachable(
  functions: &[FunRange],
  instructions: &[Insn],
  main_sym: Symbol,
  event_handlers: &HashSet<Symbol>,
) -> HashSet<Symbol> {
  let mut reachable = HashSet::default();
  let mut worklist = Vec::new();

  for func in functions {
    if func.name == main_sym
      || func.pubness == Pubness::Yes
      || event_handlers.contains(&func.name)
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
    if let Insn::Template { commands, .. } = insn {
      for cmd in commands {
        if let zo_ui_protocol::UiCommand::Event { handler, .. } = cmd
          && let Some(sym) = interner.symbol(handler)
        {
          handlers.insert(sym);
        }
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
