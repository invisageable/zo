//! SIR→SIR function inlining for `--release` builds.
//!
//! Replaces a call to a small pure function with its body.

use zo_interner::{Interner, Symbol};
use zo_sir::{Insn, LoadSource, Sir};
use zo_span::Span;
use zo_ty::{Mutability, Ty, TyId};
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
/// Raised once the inliner learned to splice body-local variables —
/// real numeric kernels (a `1.0 / ((i+j)*(i+j+1)/2 + i + 1)` helper)
/// run past the straight-line-no-locals limit.
const INLINE_MAX_INSNS: usize = 32;

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
  /// Parameter names, in declaration order. A shorthand
  /// struct field (`Self { x }`) loads a parameter through
  /// `LoadSource::Local(name)` rather than `Param(i)`, so
  /// binding needs the names to rebind those loads too.
  param_syms: Vec<Symbol>,
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
  tys: &'a [Ty],
  release: Release,
}

impl<'a> Inline<'a> {
  /// Creates an inliner over `sir`. `tys` resolves a `TyId` so a
  /// body carrying an unresolved type is left alone.
  pub fn new(
    sir: &'a mut Sir,
    interner: &'a mut Interner,
    tys: &'a [Ty],
    release: Release,
  ) -> Self {
    Self {
      sir,
      interner,
      tys,
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

  /// Whether every type the body mentions is concrete. An
  /// unresolved inference variable means a monomorphization left
  /// a generic type open in the body; inlining would expose it to
  /// codegen's type-driven dispatch (such as `showln`), which then
  /// misclassifies the value. Such a body stays a call.
  fn types_resolved(&self, body: &[Insn]) -> bool {
    body.iter().all(|insn| {
      let mut resolved = true;
      let mut probe = insn.clone();

      probe.visit_ty_ids_mut(&mut |ty_id| {
        if matches!(self.tys.get(ty_id.0 as usize), Some(Ty::Infer(_))) {
          resolved = false;
        }
      });

      resolved
    })
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
        && !is_runtime_dispatched(self.interner.get(*name))
        && let Some(candidate) = build_candidate(&insns[start..end], params)
        && self.types_resolved(&candidate.body)
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
        && args.len() == candidate.param_syms.len()
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
        // Nested inlining chains substitutions: an inner call's
        // result feeds an outer call that also inlines, so
        // `outer_dst -> inner_dst -> arg`. Follow the chain to its
        // final value. Substitutions only point at values defined
        // earlier, so the chain terminates; the map size bounds the
        // hop count as a backstop.
        let mut id = value.0;
        let mut hops = 0;

        while let Some(&to) = subst.get(&id) {
          id = to.0;
          hops += 1;

          if hops > subst.len() {
            break;
          }
        }

        *value = ValueId(id);
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

    let param_map = bind_params(&mut body, args, &candidate.param_syms);

    rename_body_locals(&mut body, self.interner, dst);

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

/// Drops each parameter load and maps its result to the matching
/// argument. A parameter reaches the body either by index
/// (`LoadSource::Param`) or, through a shorthand struct field, by
/// name (`LoadSource::Local`). A named local load whose name is not
/// a parameter is one of the body's own locals — left for
/// `rename_body_locals` — so only parameter names rebind here.
fn bind_params(
  body: &mut [Insn],
  args: &[ValueId],
  param_syms: &[Symbol],
) -> HashMap<u32, ValueId> {
  let mut param_map: HashMap<u32, ValueId> = HashMap::default();

  for insn in body.iter_mut() {
    let Insn::Load { src, dst, .. } = insn else {
      continue;
    };

    let arg = match src {
      LoadSource::Param(i) => args.get(*i as usize).copied(),
      LoadSource::Local(name) => param_syms
        .iter()
        .position(|sym| sym == name)
        .and_then(|i| args.get(i).copied()),
    };

    if let Some(arg) = arg {
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

/// Whether the body contains a loop — a branch or jump back to an
/// earlier label. A loop carries mutable state through a body local
/// across iterations, which needs a loop-header phi when the slot is
/// promoted; that promotion isn't sound here, so a looping body
/// stays a call. Loops are also low value to inline — the call
/// overhead amortizes over the iterations.
fn has_loop(body: &[Insn]) -> bool {
  let mut label_pos: HashMap<u32, usize> = HashMap::default();

  for (idx, insn) in body.iter().enumerate() {
    if let Insn::Label { id } = insn {
      label_pos.insert(*id, idx);
    }
  }

  body.iter().enumerate().any(|(idx, insn)| {
    let target = match insn {
      Insn::Jump { target } => Some(*target),
      Insn::BranchIfNot { target, .. } => Some(*target),
      _ => None,
    };

    target
      .and_then(|t| label_pos.get(&t))
      .is_some_and(|&pos| pos < idx)
  })
}

/// Whether the body's local variables are safe to splice: every
/// store, drop, and named local load reaches either a parameter or a
/// local the body itself declares with a `VarDef`, and no local
/// shadows a parameter. A store to a parameter (a mutated `mut`
/// param) or to an undeclared name can't be renamed soundly, so such
/// a body is rejected.
fn body_locals_sound(body: &[Insn], param_syms: &[Symbol]) -> bool {
  let locals = body
    .iter()
    .filter_map(|insn| match insn {
      Insn::VarDef { name, .. } => Some(*name),
      _ => None,
    })
    .collect::<Vec<_>>();

  if locals.iter().any(|local| param_syms.contains(local)) {
    return false;
  }

  body.iter().all(|insn| match insn {
    Insn::Store { name, .. } => locals.contains(name),
    Insn::Drop { local, .. } => locals.contains(local),
    Insn::Load {
      src: LoadSource::Local(name),
      ..
    } => param_syms.contains(name) || locals.contains(name),
    _ => true,
  })
}

/// Renames the body's own locals to call-site-unique names so a
/// caller local of the same name — or a second inline of the same
/// callee — never clashes. Only `VarDef`-declared names are renamed;
/// parameter loads were already bound to arguments.
fn rename_body_locals(
  body: &mut [Insn],
  interner: &mut Interner,
  dst: ValueId,
) {
  let mut rename: HashMap<u32, Symbol> = HashMap::default();

  for insn in body.iter() {
    if let Insn::VarDef { name, .. } = insn
      && !rename.contains_key(&name.as_u32())
    {
      let original = interner.get(*name).to_owned();
      let fresh = interner.intern(&format!("__inl{}_{original}__", dst.0));

      rename.insert(name.as_u32(), fresh);
    }
  }

  if rename.is_empty() {
    return;
  }

  for insn in body.iter_mut() {
    match insn {
      Insn::VarDef { name, .. } | Insn::Store { name, .. } => {
        if let Some(&fresh) = rename.get(&name.as_u32()) {
          *name = fresh;
        }
      }
      Insn::Drop { local, .. } => {
        if let Some(&fresh) = rename.get(&local.as_u32()) {
          *local = fresh;
        }
      }
      Insn::Load {
        src: LoadSource::Local(name),
        ..
      } => {
        if let Some(&fresh) = rename.get(&name.as_u32()) {
          *name = fresh;
        }
      }
      _ => {}
    }
  }
}

/// Whether codegen lowers a call to `name` as a runtime builtin,
/// overriding the function's SIR body. The std container methods
/// carry placeholder bodies (`Vec::push` is `ret void`, `Vec::new`
/// a stub `struct`) and codegen emits the real operation by name.
/// Inlining the placeholder would drop or corrupt that operation,
/// so the call must reach codegen intact. Mirrors the `::`-mangled
/// dispatch in the AArch64 backend.
fn is_runtime_dispatched(name: &str) -> bool {
  name.starts_with("Vec::")
    || name.starts_with("HashMap::")
    || name.starts_with("HashSet::")
    || name.starts_with("arr_int::")
}

/// Whether `insn` ends a function body.
fn is_boundary(insn: &Insn) -> bool {
  matches!(
    insn,
    Insn::FunDef { .. } | Insn::PackDecl { .. } | Insn::PackLink { .. }
  )
}

/// Returns a `Candidate` when `body` is eligible to inline.
fn build_candidate(
  body: &[Insn],
  params: &[(Symbol, TyId)],
) -> Option<Candidate> {
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
  let param_syms = params.iter().map(|(name, _)| *name).collect::<Vec<_>>();

  if !body_locals_sound(body, &param_syms) || has_loop(body) {
    return None;
  }

  let single_return = returns == 1
    && !has_branch
    && matches!(body.last(), Some(Insn::Return { .. }));

  let body_without_ret = &body[..body.len() - 1];

  if single_return
    && let Some(Insn::Return { value, .. }) = body.last()
    && returned_value_keeps_type(body_without_ret, *value, ret_ty)
  {
    return Some(Candidate {
      body: body_without_ret.to_vec(),
      param_syms,
      ret: ReturnMode::Direct(*value),
      value_span,
      label_span,
    });
  }

  Some(Candidate {
    body: body.to_vec(),
    param_syms,
    ret: ReturnMode::Slot(ret_ty),
    value_span,
    label_span,
  })
}

/// Whether substituting `value` for the call result preserves the
/// declared return type. A value's defining insn carries the type
/// codegen assigns it, and that can differ from the return type: a
/// comparison `BinOp` carries its operand type, not the `bool` it
/// yields. Such a return routes through a slot load that restores
/// `ret_ty`; matching types take the cheaper direct substitution.
fn returned_value_keeps_type(
  body: &[Insn],
  value: Option<ValueId>,
  ret_ty: TyId,
) -> bool {
  match value {
    None => true,
    Some(value) => value_def_type(body, value) == Some(ret_ty),
  }
}

/// The result type of the insn that defines `target`, if any.
fn value_def_type(body: &[Insn], target: ValueId) -> Option<TyId> {
  body.iter().find_map(|insn| match insn {
    Insn::ConstInt { dst, ty_id, .. }
    | Insn::ConstFloat { dst, ty_id, .. }
    | Insn::ConstBool { dst, ty_id, .. }
    | Insn::ConstString { dst, ty_id, .. }
    | Insn::Load { dst, ty_id, .. }
    | Insn::BinOp { dst, ty_id, .. }
    | Insn::UnOp { dst, ty_id, .. }
    | Insn::TupleIndex { dst, ty_id, .. }
    | Insn::StructConstruct { dst, ty_id, .. }
    | Insn::EnumConstruct { dst, ty_id, .. }
      if *dst == target =>
    {
      Some(*ty_id)
    }
    Insn::Cast { dst, to_ty, .. } if *dst == target => Some(*to_ty),
    _ => None,
  })
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
      | Insn::StructConstruct { .. }
      | Insn::EnumConstruct { .. }
      | Insn::VarDef { .. }
      | Insn::Store { .. }
      | Insn::Drop { .. }
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
