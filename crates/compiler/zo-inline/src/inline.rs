//! SIR→SIR function inlining for `--release` builds.
//!
//! Replaces a call to a small pure function with its body.

use zo_interner::{Interner, Symbol};
use zo_sir::{Insn, LoadSource, Sir};
use zo_span::Span;
use zo_ty::{Mutability, TyId};
use zo_value::{FunctionKind, Pubness, ValueId};

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
const INLINE_MAX_INSNS: usize = 16;

/// How a callee's return value reaches the call's `dst`.
enum ReturnMode {
  /// A single straight-line return.
  Direct(Option<ValueId>),
  /// Branches or multiple returns, carrying the return type.
  Slot(TyId),
}

/// A callee cleared for inlining.
struct Candidate {
  /// Body instructions, without the `FunDef`.
  body: Vec<Insn>,
  /// Declared parameter count.
  param_count: usize,
  /// How the return value reaches the call site.
  ret: ReturnMode,
  /// ValueId stride to reserve per inlined copy.
  value_span: u32,
  /// Label stride to reserve per inlined copy.
  label_span: u32,
}

/// The SIR→SIR function inliner.
pub struct Inline<'a> {
  sir: &'a mut Sir,
  interner: &'a mut Interner,
  release: Release,
}

impl<'a> Inline<'a> {
  /// Creates an inliner over `sir`.
  pub fn new(
    sir: &'a mut Sir,
    interner: &'a mut Interner,
    release: Release,
  ) -> Self {
    Self {
      sir,
      interner,
      release,
    }
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

    Sir::offset_value_ids(&mut body, value_base);
    Sir::offset_labels(&mut body, self.sir.next_label_id);

    self.sir.next_value_id += candidate.value_span;
    self.sir.next_label_id += candidate.label_span;

    let param_map = bind_params(&mut body, args);

    match candidate.ret {
      ReturnMode::Direct(ret) => {
        if let Some(ret) = ret {
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
      ReturnMode::Slot(ret_ty) => {
        // A fresh local merges the return values from each branch;
        // mem2reg promotes it back to a register during codegen.
        let slot = self.interner.intern(&format!("__inline_ret_{}__", dst.0));
        let merge = self.sir.next_label_id;

        self.sir.next_label_id += 1;

        new_insns.push(Insn::VarDef {
          name: slot,
          ty_id: ret_ty,
          init: None,
          mutability: Mutability::Yes,
          pubness: Pubness::No,
        });
        new_spans.push(span);

        for insn in body {
          match insn {
            Insn::Return {
              value: Some(value),
              ty_id,
            } => {
              new_insns.push(Insn::Store {
                name: slot,
                value,
                ty_id,
              });
              new_spans.push(span);
              new_insns.push(Insn::Jump { target: merge });
              new_spans.push(span);
            }
            Insn::Return { value: None, .. } => {
              new_insns.push(Insn::Jump { target: merge });
              new_spans.push(span);
            }
            other => {
              new_insns.push(other);
              new_spans.push(span);
            }
          }
        }

        new_insns.push(Insn::Label { id: merge });
        new_spans.push(span);
        new_insns.push(Insn::Load {
          dst,
          src: LoadSource::Local(slot),
          ty_id: ret_ty,
        });
        new_spans.push(span);
      }
    }
  }
}

/// Drops each `Load param[i]` and maps its result to `args[i]`.
fn bind_params(body: &mut [Insn], args: &[ValueId]) -> HashMap<u32, ValueId> {
  let mut param_map: HashMap<u32, ValueId> = HashMap::default();

  for insn in body.iter_mut() {
    if let Insn::Load {
      src: LoadSource::Param(i),
      dst,
      ..
    } = insn
      && let Some(&arg) = args.get(*i as usize)
    {
      param_map.insert(dst.0, arg);
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

  param_map
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

  let mut returns = 0;
  let mut has_branch = false;
  let mut ret_ty = None;

  for insn in body {
    match insn {
      Insn::Return { ty_id, .. } => {
        returns += 1;
        ret_ty = Some(*ty_id);
      }
      Insn::Label { .. } | Insn::Jump { .. } | Insn::BranchIfNot { .. } => {
        has_branch = true;
      }
      other if is_inlinable_insn(other) => {}
      _ => return None,
    }
  }

  let ret_ty = ret_ty?;
  let (value_span, label_span) = strides(body);

  let single_return = returns == 1
    && !has_branch
    && matches!(body.last(), Some(Insn::Return { .. }));

  if single_return {
    let Some(Insn::Return { value, .. }) = body.last() else {
      return None;
    };

    Some(Candidate {
      body: body[..body.len() - 1].to_vec(),
      param_count,
      ret: ReturnMode::Direct(*value),
      value_span,
      label_span,
    })
  } else {
    Some(Candidate {
      body: body.to_vec(),
      param_count,
      ret: ReturnMode::Slot(ret_ty),
      value_span,
      label_span,
    })
  }
}

/// Whether `insn` is side-effect-free and safe to duplicate.
fn is_inlinable_insn(insn: &Insn) -> bool {
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
      | Insn::Label { .. }
      | Insn::Jump { .. }
      | Insn::BranchIfNot { .. }
      | Insn::Nop
  )
}

/// One past the largest ValueId and label `body` mentions.
fn strides(body: &[Insn]) -> (u32, u32) {
  let mut value_span = 0u32;
  let mut label_span = 0u32;

  for insn in body {
    let mut probe = insn.clone();

    probe.visit_value_ids_mut(&mut |value| {
      value_span = value_span.max(value.0 + 1);
    });

    match insn {
      Insn::Label { id } => label_span = label_span.max(id + 1),
      Insn::Jump { target } => label_span = label_span.max(target + 1),
      Insn::BranchIfNot { target, .. } => {
        label_span = label_span.max(target + 1)
      }
      _ => {}
    }
  }

  (value_span, label_span)
}

/// Resets each `FunDef`'s `body_start` to its new index.
fn fixup_fundef_offsets(insns: &mut [Insn]) {
  for (idx, insn) in insns.iter_mut().enumerate() {
    if let Insn::FunDef { body_start, .. } = insn {
      *body_start = (idx + 1) as u32;
    }
  }
}
