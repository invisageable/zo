use zo_interner::Symbol;
use zo_sir::{Insn, Sir};

use rustc_hash::FxHashSet as HashSet;

/// Function range in the SIR instruction stream.
struct FunRange {
  name: Symbol,
  /// Index of the `FunDef` instruction.
  start: usize,
  /// Index of the terminating `Return` instruction.
  end: usize,
}

/// Eliminates unreachable functions from the SIR.
///
/// Builds a call graph, marks functions reachable from `main`
/// transitively, and removes unreachable function bodies.
/// Top-level instructions (outside any function) are always kept.
pub fn eliminate_dead_functions(sir: &mut Sir) {
  if sir.instructions.is_empty() {
    return;
  }

  let functions = build_function_map(&sir.instructions);

  if functions.is_empty() {
    return;
  }

  let called = collect_called_names(&sir.instructions);
  let reachable = mark_reachable(&functions, &called);

  // Collect dead ranges in reverse order for safe removal.
  let mut dead_ranges = functions
    .iter()
    .filter(|f| !reachable.contains(&f.name))
    .map(|f| (f.start, f.end))
    .collect::<Vec<_>>();

  dead_ranges.sort_by_key(|r| std::cmp::Reverse(r.0));

  for (start, end) in dead_ranges {
    if end < sir.instructions.len() {
      sir.instructions.drain(start..=end);
    }
  }
}

/// Scans the instruction stream and pairs each `FunDef` with
/// its terminating `Return` to build function ranges.
fn build_function_map(instructions: &[Insn]) -> Vec<FunRange> {
  let mut functions = Vec::new();
  let mut i = 0;

  while i < instructions.len() {
    if let Insn::FunDef { name, .. } = &instructions[i] {
      let start = i;

      // Scan forward for Return. Stop at the next
      // FunDef or top-level declaration (StructDef,
      // EnumDef) to avoid swallowing adjacent bodies
      // or type definitions. Intrinsic functions have
      // no Return so this also handles them.
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

      // Clamp to last valid index.
      if end >= instructions.len() {
        end = instructions.len() - 1;
      }

      functions.push(FunRange {
        name: *name,
        start,
        end,
      });

      i = end + 1;
    } else {
      i += 1;
    }
  }

  functions
}

/// Collects all function names that appear in `Insn::Call`.
fn collect_called_names(instructions: &[Insn]) -> HashSet<Symbol> {
  let mut called = HashSet::default();

  for insn in instructions {
    if let Insn::Call { name, .. } = insn {
      called.insert(*name);
    }
  }

  called
}

/// Marks functions reachable from `main` via transitive calls.
///
/// Roots: `main` + any function name found in a `Call` instruction
/// that itself appears inside a reachable function body.
/// Marks functions reachable from the entry point (last
/// function) and any function referenced by a `Call`.
///
/// A function is dead if no `Call` references it and it's
/// not the entry point.
fn mark_reachable(
  functions: &[FunRange],
  called: &HashSet<Symbol>,
) -> HashSet<Symbol> {
  let mut reachable = HashSet::default();

  // The last function is the entry point (main by convention).
  if let Some(last) = functions.last() {
    reachable.insert(last.name);
  }

  // A function is reachable if any Call references it.
  for func in functions {
    if called.contains(&func.name) {
      reachable.insert(func.name);
    }
  }

  reachable
}
