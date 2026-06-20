//! SIR→SIR function inlining for `--release` builds.
//!
//! Replaces a call to a small pure-leaf function with its body.

use zo_interner::Symbol;
use zo_sir::{Insn, LoadSource, Sir};
use zo_span::Span;
use zo_value::{FunctionKind, ValueId};

use rustc_hash::FxHashMap as HashMap;

/// Whether `--release` optimization passes run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Release {
  /// Dev build — no optimization passes.
  #[default]
  No,
  /// Optimized build.
  Yes,
}

/// Largest callee body, in SIR instructions, eligible to inline.
const INLINE_MAX_INSNS: usize = 8;

/// A callee cleared for inlining.
struct Candidate {
  /// Body instructions, without the `FunDef` or `Return`.
  body: Vec<Insn>,
  /// Declared parameter count.
  param_count: usize,
  /// The returned value, `None` for a void function.
  ret: Option<ValueId>,
  /// ValueId stride to reserve per inlined copy.
  value_span: u32,
  /// Label stride to reserve per inlined copy.
  label_span: u32,
}

/// The SIR→SIR function inliner.
pub struct Inline<'sir> {
  sir: &'sir mut Sir,
  release: Release,
}

impl<'sir> Inline<'sir> {
  /// Creates an inliner over `sir`.
  pub fn new(sir: &'sir mut Sir, release: Release) -> Self {
    Self { sir, release }
  }

  /// Inlines every eligible call site.
  pub fn inline(&mut self) {
    if self.release == Release::No {
      return;
    }

    let candidates = self.collect_candidates();

    if candidates.is_empty() {
      return;
    }

    self.rewrite_calls(&candidates);
  }

  /// Collects the functions eligible to inline.
  fn collect_candidates(&self) -> HashMap<(Symbol, Option<Symbol>), Candidate> {
    let insns = &self.sir.instructions;
    let mut candidates = HashMap::default();
    let mut i = 0;

    while i < insns.len() {
      let Insn::FunDef {
        name,
        kind,
        owning_pack,
        is_test,
        params,
        ..
      } = &insns[i]
      else {
        i += 1;

        continue;
      };

      let start = i + 1;
      let mut end = start;

      while end < insns.len() && !is_boundary(&insns[end]) {
        end += 1;
      }

      if *kind == FunctionKind::UserDefined
        && !is_test
        && let Some(candidate) =
          build_candidate(&insns[start..end], params.len())
      {
        candidates.insert((*name, *owning_pack), candidate);
      }

      i = end;
    }

    candidates
  }

  /// Replaces eligible calls with their callee bodies.
  fn rewrite_calls(
    &mut self,
    candidates: &HashMap<(Symbol, Option<Symbol>), Candidate>,
  ) {
    let old = std::mem::take(&mut self.sir.instructions);
    let old_spans = std::mem::take(&mut self.sir.spans);

    let mut new_insns: Vec<Insn> = Vec::with_capacity(old.len());
    let mut new_spans: Vec<Span> = Vec::with_capacity(old.len());
    let mut subst: HashMap<u32, ValueId> = HashMap::default();

    for (idx, insn) in old.into_iter().enumerate() {
      let span = old_spans.get(idx).copied().unwrap_or(Span::ZERO);

      if let Insn::Call {
        dst,
        name,
        callee_pack,
        args,
        ..
      } = &insn
        && let Some(candidate) = candidates.get(&(*name, *callee_pack))
        && args.len() == candidate.param_count
      {
        self.splice(
          candidate,
          *dst,
          args,
          span,
          &mut new_insns,
          &mut new_spans,
          &mut subst,
        );

        continue;
      }

      new_insns.push(insn);
      new_spans.push(span);
    }

    for insn in new_insns.iter_mut() {
      insn.visit_value_ids_mut(&mut |value| {
        if let Some(&to) = subst.get(&value.0) {
          *value = to;
        }
      });
    }

    fixup_fundef_offsets(&mut new_insns);

    self.sir.instructions = new_insns;
    self.sir.spans = new_spans;
  }

  /// Inlines one call site into the rebuilt stream.
  #[allow(clippy::too_many_arguments)]
  fn splice(
    &mut self,
    candidate: &Candidate,
    dst: ValueId,
    args: &[ValueId],
    span: Span,
    new_insns: &mut Vec<Insn>,
    new_spans: &mut Vec<Span>,
    subst: &mut HashMap<u32, ValueId>,
  ) {
    let mut body = candidate.body.clone();
    let value_base = self.sir.next_value_id;
    let label_base = self.sir.next_label_id;

    Sir::offset_value_ids(&mut body, value_base);
    Sir::offset_labels(&mut body, label_base);

    self.sir.next_value_id += candidate.value_span;
    self.sir.next_label_id += candidate.label_span;

    let mut param_map: HashMap<u32, ValueId> = HashMap::default();

    for insn in body.iter_mut() {
      if let Insn::Load {
        src: LoadSource::Param(i),
        dst: param_dst,
        ..
      } = insn
        && let Some(&arg) = args.get(*i as usize)
      {
        param_map.insert(param_dst.0, arg);
        *insn = Insn::Nop;
      }
    }

    for insn in body.iter_mut() {
      insn.visit_value_ids_mut(&mut |value| {
        if let Some(&arg) = param_map.get(&value.0) {
          *value = arg;
        }
      });
    }

    if let Some(ret) = candidate.ret {
      let renumbered = ret.0 + value_base;
      let final_ret = param_map
        .get(&renumbered)
        .copied()
        .unwrap_or(ValueId(renumbered));

      subst.insert(dst.0, final_ret);
    }

    for insn in body {
      new_insns.push(insn);
      new_spans.push(span);
    }
  }
}

/// Whether `insn` ends a function body.
fn is_boundary(insn: &Insn) -> bool {
  matches!(
    insn,
    Insn::FunDef { .. } | Insn::PackDecl { .. } | Insn::PackLink { .. }
  )
}

/// Returns a `Candidate` when `body` is eligible to inline.
fn build_candidate(body: &[Insn], param_count: usize) -> Option<Candidate> {
  if body.is_empty() || body.len() > INLINE_MAX_INSNS {
    return None;
  }

  let (last, rest) = body.split_last()?;

  let Insn::Return { value, .. } = last else {
    return None;
  };

  if !rest.iter().all(is_pure_value_insn) {
    return None;
  }

  let mut value_span = 0u32;

  for insn in rest {
    let mut probe = insn.clone();

    probe.visit_value_ids_mut(&mut |value| {
      value_span = value_span.max(value.0 + 1);
    });
  }

  if let Some(returned) = value {
    value_span = value_span.max(returned.0 + 1);
  }

  Some(Candidate {
    body: rest.to_vec(),
    param_count,
    ret: *value,
    value_span,
    label_span: 0,
  })
}

/// Whether `insn` is side-effect-free value computation.
fn is_pure_value_insn(insn: &Insn) -> bool {
  matches!(
    insn,
    Insn::ConstInt { .. }
      | Insn::ConstFloat { .. }
      | Insn::ConstBool { .. }
      | Insn::ConstString { .. }
      | Insn::Load { .. }
      | Insn::BinOp { .. }
      | Insn::UnOp { .. }
      | Insn::Cast { .. }
      | Insn::TupleIndex { .. }
      | Insn::Nop
  )
}

/// Resets each `FunDef`'s `body_start` to its new index.
fn fixup_fundef_offsets(insns: &mut [Insn]) {
  for (idx, insn) in insns.iter_mut().enumerate() {
    if let Insn::FunDef { body_start, .. } = insn {
      *body_start = (idx + 1) as u32;
    }
  }
}
