use zo_interner::Symbol;
use zo_span::Span;
use zo_token::Token;
use zo_ty::Mutability;
use zo_ty::SelfKind;
use zo_ty::TyId;
use zo_ui_protocol::{Attr, StyleScope, UiCommand};
use zo_value::{FunctionKind, Pubness, ValueId};

/// Reactive bindings carried by `Insn::Template`. Split by
/// target kind so the runtime can dispatch patches without
/// introspecting variant shapes at every step.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TemplateBindings {
  /// Text-content bindings: each entry is `(cmd_idx, var)`
  /// where `cmd_idx` points at a `UiCommand::Text(_)` whose
  /// string is regenerated from the state cell for `var` on
  /// each reactive update.
  pub text: Vec<(usize, Symbol)>,
  /// Element-attribute bindings: each entry is `(cmd_idx,
  /// Attr::Dynamic { name, var, initial })` pointing at a
  /// `UiCommand::Element` whose attribute `name` is reactive
  /// on variable `var`. The runtime calls
  /// `UiCommand::set_attr` to apply the patch.
  pub attrs: Vec<(usize, Attr)>,
  /// Computed text bindings: each entry is `(cmd_idx,
  /// ComputedBinding)` pointing at a `UiCommand::Text(_)`
  /// whose value is recomputed by invoking
  /// `closure_name` over the captured locals on every
  /// reactive update. Used for compound `{expr}`
  /// interpolations (ternaries, function calls, ...) that
  /// can't be expressed as a single `Symbol` lookup.
  pub computed: Vec<(usize, ComputedBinding)>,
  /// List bindings: `(cmd_idx, ListBinding)`. Each entry
  /// points at a placeholder `UiCommand::Text(_)` slot in
  /// the parent commands buffer. The runtime, on every
  /// state-cell update for `items_var`, walks the array
  /// and splats `item_template` once per element into a
  /// fresh sub-command list — replacing the placeholder
  /// with the rendered batch. Used for
  /// `<X>{arr.map(fn(t) =:> <body>)}</X>`.
  pub list: Vec<(usize, ListBinding)>,
}

/// Side-channel for a compound `{expr}` template
/// interpolation. The executor synthesizes a closure named
/// `closure_name` that captures `captures` (in param-order)
/// and returns the expression's value. The runtime invokes
/// the closure on each state change and stamps the result
/// over the bound `UiCommand::Text` slot.
#[derive(Clone, Debug, PartialEq)]
pub struct ComputedBinding {
  pub closure_name: Symbol,
  pub captures: Vec<Symbol>,
}

/// Side-channel for a `<X>{arr.map(fn(t) =:> <body>)}</X>`
/// list rendering. The executor doesn't expand the closure
/// at compile time — instead it captures the per-item
/// "template recipe" (`item_template`) plus the array
/// variable's symbol. At runtime, on every event affecting
/// `items_var`, the driver re-runs the recipe once per
/// element and splices the resulting commands at the
/// placeholder slot.
///
/// `item_template` is a flat sequence of "render this
/// command, with the item value substituted at this
/// position" — kept small (open tag, text, close tag for
/// the wip's `<li>{t}</li>`) since the closure body is
/// constrained to a single-tag wrapper with one `{t}`
/// interp.
#[derive(Clone, Debug, PartialEq)]
pub struct ListBinding {
  /// The `[]T` variable being mapped. State-cell updates
  /// for this symbol trigger a list re-render.
  pub items_var: Symbol,
  /// Per-item template — applied N times for an N-element
  /// array.
  pub item_template: Vec<ListItemCmd>,
}

/// One step in a list-binding's per-item recipe. The
/// runtime walks this sequence once per element and emits
/// `UiCommand`s with the item value substituted in
/// `TextFromItem` slots.
#[derive(Clone, Debug, PartialEq)]
pub enum ListItemCmd {
  /// Emit a `UiCommand::Element` with this static
  /// configuration. Used for the wrapping tag (e.g. `<li>`).
  Element {
    tag: zo_ui_protocol::ElementTag,
    attrs: Vec<Attr>,
  },
  /// Emit a `UiCommand::EndElement`.
  EndElement,
  /// Emit a `UiCommand::Text` with this literal string.
  Text(String),
  /// Emit a `UiCommand::Text` whose content is the current
  /// item's stringified value. The wip's `<li>{t}</li>`
  /// uses one of these for the `{t}` interp.
  TextFromItem,
}

/// One path literal inside a `#link { ... }` directive
/// — pairs the interned string with the source span so
/// the resolution diagnostic can underline the exact
/// offending characters. Couples value + span at the
/// type level so a future parser change can't desync
/// them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinkPath {
  pub value: Symbol,
  pub span: zo_span::Span,
}

/// One platform slot inside a `#link { ... }` directive.
/// Codegen tries `system` first (homebrew / apt installs),
/// falls back to `vendor` (bundled prebuilt under
/// `<exe-dir>/../lib/vendor/`). Either may be absent.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LinkEntry {
  /// Absolute path or `@executable_path/...`. The latter
  /// bypasses on-disk checks since the dylib is staged
  /// per-binary, not present at codegen time.
  pub system: Option<LinkPath>,
  /// Bare filename resolved under
  /// `<exe-dir>/../lib/vendor/<name>` (where
  /// `tasks/zo-install.sh` extracts the
  /// `zo-vendor-VERSION-PLATFORM.tar.gz` artifact).
  pub vendor: Option<LinkPath>,
}

/// Outcome of the executor's `system → vendor` walk
/// over a `#link`'s host entry. Codegen reads this to
/// decide whether to emit an `LC_LOAD_DYLIB`. Compiler
/// staging reads the `Resolved` variant to know which
/// dylib basenames to copy next to the user binary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LinkResolution {
  /// Path codegen should emit as `LC_LOAD_DYLIB <sym>`.
  /// An absolute path (system install) or
  /// `@executable_path/<name>` (per-binary staging).
  Resolved(Symbol),
  /// Host entry absent — the pack didn't declare a
  /// `#link` for this OS. Codegen no-ops; no
  /// diagnostic.
  Skipped,
  /// Host entry declared but neither `system` nor
  /// `vendor` resolved. A `LinkResolutionFailed`
  /// diagnostic was already reported at the executor;
  /// codegen no-ops.
  Failed,
}

/// Per-platform dylib link metadata declared by a
/// `#link { ... }` directive at the top of a `pack`. Each
/// platform slot is a `LinkEntry` with `system` /
/// `vendor` fallback semantics — see [`LinkEntry`]. The
/// compiler picks one slot at codegen time per host OS.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LinkSpec {
  pub macos: Option<LinkEntry>,
  pub linux: Option<LinkEntry>,
  pub windows: Option<LinkEntry>,
}

impl LinkSpec {
  /// The platform entry for the host OS the compiler is
  /// running on. Eliminates the duplicated `if cfg!(...)`
  /// chain at every consumer (codegen, compiler staging,
  /// future linker plumbing).
  pub fn host_entry(&self) -> Option<&LinkEntry> {
    if cfg!(target_os = "macos") {
      self.macos.as_ref()
    } else if cfg!(target_os = "linux") {
      self.linux.as_ref()
    } else {
      self.windows.as_ref()
    }
  }
}

/// Source of a Load instruction — either a function parameter
/// or a local variable on the stack.
#[derive(Clone, Debug, PartialEq)]
pub enum LoadSource {
  /// Function parameter by index (X0-X7 or D0-D7).
  Param(u32),
  /// Local variable by symbol (stack-allocated).
  Local(Symbol),
}

/// How `load` brings imported items into the current file's
/// scope. The path itself (`load core::math::…`) is carried
/// separately on `Insn::ModuleLoad::path`.
#[derive(Clone, Debug, PartialEq)]
pub enum ImportKind {
  /// `load core::math` — items reachable only as `math::name`.
  Qualified,
  /// `load core::math::*` — every `pub` item of the target
  /// pack is in scope unqualified; qualified form still works.
  Glob,
  /// `load core::math::(sin, cos)` — only the listed identifiers
  /// are in scope unqualified.
  Selective(Vec<Symbol>),
}

/// Represents a semantic intermediate representation.
#[derive(Debug)]
pub struct Sir {
  /// The linear array of SIR instructions.
  pub instructions: Vec<Insn>,
  /// Source span per instruction, aligned 1:1 with
  /// `instructions`. Invariant: `spans.len() ==
  /// instructions.len()`. Empty during execution — `emit`
  /// records a node index in `node_idxs` instead; `resolve_
  /// spans` fills this in one linear pass at the end. Every
  /// later site that mutates `instructions` mutates `spans`
  /// identically. Drives SIR-level diagnostics.
  pub spans: Vec<Span>,
  /// Parse-node index per instruction, recorded by `emit`
  /// from `node_cursor`. Transient: resolved into `spans`
  /// and dropped by `resolve_spans` at the end of execution.
  /// Avoids reading the tree's span side-array on every node.
  pub node_idxs: Vec<u32>,
  /// The next value ID for SSA.
  pub next_value_id: u32,
  /// The next label ID for branch targets.
  pub next_label_id: u32,
  /// Current parse-node index, stamped onto each emitted
  /// instruction. The executor sets it per node — a register
  /// write, no memory read — and spans resolve in bulk later.
  pub node_cursor: u32,
}

impl Sir {
  /// Creates a new [`SirBuilder`] instance.
  pub fn new() -> Self {
    Self {
      instructions: Vec::with_capacity(1024),
      spans: Vec::new(),
      node_idxs: Vec::with_capacity(1024),
      next_value_id: 0,
      next_label_id: 0,
      node_cursor: 0,
    }
  }

  /// Sets the parse-node index stamped onto subsequent emits.
  #[inline]
  pub fn set_node(&mut self, node_idx: u32) {
    self.node_cursor = node_idx;
  }

  /// Resolves the buffered node indices into source spans in a
  /// single linear pass against the owning tree's span array,
  /// then drops the transient buffer. Called once at the end
  /// of execution, before merge — sequential access on both
  /// sides, prefetcher-friendly.
  pub fn resolve_spans(&mut self, tree_spans: &[Span]) {
    self.spans.clear();
    self.spans.reserve(self.node_idxs.len());

    for &node_idx in &self.node_idxs {
      let span = tree_spans
        .get(node_idx as usize)
        .copied()
        .unwrap_or(Span::ZERO);

      self.spans.push(span);
    }

    self.node_idxs = Vec::new();
  }

  /// Allocates a fresh label ID.
  pub fn next_label(&mut self) -> u32 {
    let id = self.next_label_id;

    self.next_label_id += 1;

    id
  }

  /// Allocates a fresh SSA value ID. Mirrors `next_label`'s
  /// shape so `dst` minting at every emit site is one line
  /// instead of the `let dst = ValueId(self.sir.next_value_id);
  /// self.sir.next_value_id += 1;` pair.
  pub fn next_value(&mut self) -> ValueId {
    let id = ValueId(self.next_value_id);

    self.next_value_id += 1;

    id
  }

  /// Emits an instruction and return its result [`ValueId`].
  ///
  /// Every value-producing instruction has an explicit `dst`
  /// field. Non-value instructions return a sentinel.
  pub fn emit(&mut self, insn: Insn) -> ValueId {
    let value_id = insn.value_id();

    self.instructions.push(insn);
    self.node_idxs.push(self.node_cursor);

    value_id
  }

  /// Parse-node index of the instruction that defines
  /// `value`, or `None` for a sentinel / undefined value.
  ///
  /// Scans the emitted instructions in reverse for the
  /// matching `dst`. Reserved for the diagnostic path — a
  /// type mismatch recovers the source span of each
  /// conflicting value here — so the linear scan never runs
  /// on the happy path.
  pub fn node_of_value(&self, value: ValueId) -> Option<u32> {
    if value.0 == u32::MAX {
      return None;
    }

    self
      .instructions
      .iter()
      .rposition(|insn| insn.value_id() == value)
      .and_then(|i| self.node_idxs.get(i).copied())
  }

  /// Offsets all `ValueId`s in instructions by `offset`.
  /// Used when prepending module SIR to avoid ID collisions.
  ///
  /// @note — the `u32::MAX` sentinel (produced by `push`
  /// for non-value-producing instructions) is preserved
  /// unchanged. Offsetting it would overflow AND lose
  /// the sentinel property, misclassifying the value
  /// slot downstream.
  pub fn offset_value_ids(instructions: &mut [Insn], offset: u32) {
    for insn in instructions.iter_mut() {
      insn.visit_value_ids_mut(&mut |v| {
        if v.0 != u32::MAX {
          v.0 += offset;
        }
      });
    }
  }

  /// Offsets every label id (`Insn::Label.id`, `Insn::Jump
  /// .target`, `Insn::BranchIfNot.target`) by `offset`.
  /// Each module's SIR starts its own label counter at 0,
  /// so after naive concatenation multiple `Label { id: 0 }`
  /// exist and any `Jump { target: 0 }` resolves to the
  /// earliest one — wrong branch destination, silent wrong
  /// code. Parallel to `offset_value_ids` but for the
  /// label namespace.
  pub fn offset_labels(instructions: &mut [Insn], offset: u32) {
    for insn in instructions.iter_mut() {
      match insn {
        Insn::Label { id } => *id += offset,
        Insn::Jump { target } => *target += offset,
        Insn::BranchIfNot { target, .. } => *target += offset,
        _ => {}
      }
    }
  }
}

impl Insn {
  /// The `ValueId` this instruction defines, or the
  /// `u32::MAX` sentinel for a non-value instruction.
  ///
  /// Single source of truth for the dst of each variant —
  /// `emit` stamps it onto the value stream and
  /// `Sir::node_of_value` reads it back to recover a value's
  /// source span on the diagnostic path.
  pub fn value_id(&self) -> ValueId {
    match self {
      Insn::ConstInt { dst, .. }
      | Insn::ConstFloat { dst, .. }
      | Insn::ConstBool { dst, .. }
      | Insn::ConstString { dst, .. }
      | Insn::Call { dst, .. }
      | Insn::Load { dst, .. }
      | Insn::BinOp { dst, .. }
      | Insn::UnOp { dst, .. }
      | Insn::ArrayLiteral { dst, .. }
      | Insn::ArrayIndex { dst, .. }
      | Insn::ArrayLen { dst, .. }
      | Insn::ArrayPop { dst, .. }
      | Insn::TupleLiteral { dst, .. }
      | Insn::TupleIndex { dst, .. }
      | Insn::EnumConstruct { dst, .. }
      | Insn::StructConstruct { dst, .. }
      | Insn::Cast { dst, .. }
      | Insn::ChannelCreate { dst, .. }
      | Insn::ChannelRecv { dst, .. }
      | Insn::TaskSpawn { dst, .. }
      | Insn::TaskAwait { dst, .. }
      | Insn::TaskCancelled { dst, .. }
      | Insn::StrSlice { dst, .. }
      | Insn::ToStr { dst, .. }
      | Insn::StringFormat { dst, .. }
      | Insn::CoerceToDyn { dst, .. }
      | Insn::DynDispatch { dst, .. } => *dst,
      Insn::Template { id, .. } => *id,
      _ => ValueId(u32::MAX),
    }
  }

  /// Walks every `ValueId` in this instruction, applying `f`.
  /// Used by SIR passes that need to rewrite value IDs
  /// (e.g., module merging, monomorphization).
  pub fn visit_value_ids_mut(&mut self, f: &mut impl FnMut(&mut ValueId)) {
    match self {
      Insn::ConstInt { dst, .. }
      | Insn::ConstFloat { dst, .. }
      | Insn::ConstBool { dst, .. }
      | Insn::ConstString { dst, .. }
      | Insn::Load { dst, .. } => f(dst),
      Insn::ModuleLoad { .. }
      | Insn::PackDecl { .. }
      | Insn::PackLink { .. }
      | Insn::EnumDef { .. }
      | Insn::StructDef { .. }
      | Insn::ArrayTyDef { .. }
      | Insn::MapTyDef { .. }
      | Insn::VecTyDef { .. }
      | Insn::SetTyDef { .. }
      | Insn::Label { .. }
      | Insn::Jump { .. }
      | Insn::FunDef { .. }
      | Insn::ConstDef { .. }
      | Insn::StyleSheet { .. }
      | Insn::TestBegin { .. }
      | Insn::TestRun { .. }
      | Insn::TestSummary
      | Insn::Nop => {}
      Insn::VarDef { init, .. } => {
        if let Some(v) = init {
          f(v);
        }
      }
      Insn::Store { value, .. } => f(value),
      // `Drop` references a local by name, not a `ValueId`.
      Insn::Drop { .. } => {}
      Insn::Return { value, .. } => {
        if let Some(v) = value {
          f(v);
        }
      }
      Insn::Call { dst, args, .. } => {
        f(dst);
        args.iter_mut().for_each(&mut *f);
      }
      Insn::BinOp { dst, lhs, rhs, .. } => {
        f(dst);
        f(lhs);
        f(rhs);
      }
      Insn::UnOp { dst, rhs, .. } => {
        f(dst);
        f(rhs);
      }
      Insn::BranchIfNot { cond, .. } => f(cond),
      Insn::Directive { value, .. } => f(value),
      Insn::Template { id, .. } => f(id),
      Insn::ArrayLiteral { dst, elements, .. } => {
        f(dst);
        elements.iter_mut().for_each(&mut *f);
      }
      Insn::ArrayIndex {
        dst, array, index, ..
      } => {
        f(dst);
        f(array);
        f(index);
      }
      Insn::ArrayStore {
        array,
        index,
        value,
        ..
      } => {
        f(array);
        f(index);
        f(value);
      }
      Insn::ArrayLen { dst, array, .. } => {
        f(dst);
        f(array);
      }
      Insn::ArrayPush { array, value, .. } => {
        f(array);
        f(value);
      }
      Insn::ArrayPop { dst, array, .. } => {
        f(dst);
        f(array);
      }
      Insn::TupleLiteral { dst, elements, .. } => {
        f(dst);
        elements.iter_mut().for_each(&mut *f);
      }
      Insn::TupleIndex { dst, tuple, .. } => {
        f(dst);
        f(tuple);
      }
      Insn::EnumConstruct { dst, fields, .. } => {
        f(dst);
        fields.iter_mut().for_each(&mut *f);
      }
      Insn::StructConstruct { dst, fields, .. } => {
        f(dst);
        fields.iter_mut().for_each(&mut *f);
      }
      Insn::FieldStore { base, value, .. } => {
        f(base);
        f(value);
      }
      Insn::Cast { dst, src, .. } => {
        f(dst);
        f(src);
      }
      Insn::ChannelCreate { dst, .. } => {
        f(dst);
      }
      Insn::ChannelSend { channel, value, .. } => {
        f(channel);
        f(value);
      }
      Insn::ChannelRecv { dst, channel, .. } => {
        f(dst);
        f(channel);
      }
      Insn::ChannelClose { channel } => {
        f(channel);
      }
      Insn::TaskSpawn { dst, args, .. } => {
        f(dst);
        args.iter_mut().for_each(&mut *f);
      }
      Insn::TaskAwait { dst, task, .. } => {
        f(dst);
        f(task);
      }
      Insn::SelectWait {
        out_which, chans, ..
      } => {
        f(out_which);
        chans.iter_mut().for_each(&mut *f);
      }
      Insn::SelectRecv { dst, which, .. } => {
        f(dst);
        f(which);
      }
      Insn::TaskCancelled { dst, task, .. } => {
        f(dst);
        f(task);
      }
      Insn::TaskCancel { task } => {
        f(task);
      }
      Insn::StrSlice {
        dst, src, lo, hi, ..
      } => {
        f(dst);
        f(src);
        f(lo);
        f(hi);
      }
      Insn::ToStr { dst, src, .. } => {
        f(dst);
        f(src);
      }
      Insn::StringFormat { dst, segments, .. } => {
        f(dst);

        for seg in segments {
          f(seg);
        }
      }
      Insn::NurseryBegin { .. } | Insn::NurseryEnd { .. } => {}
      Insn::CoerceToDyn { dst, src, .. } => {
        f(dst);
        f(src);
      }
      Insn::DynDispatch {
        dst, recv, args, ..
      } => {
        f(dst);
        f(recv);
        for a in args {
          f(a);
        }
      }
    }
  }

  /// Walks every `TyId` in this instruction, applying `f`.
  /// Used by the instantiation pass to substitute generic
  /// inference vars with concrete types in a monomorphized
  /// body. Covers every `ty_id` field; aggregate kinds
  /// (ArrayLiteral / TupleLiteral / struct / enum) expose
  /// their element/field types where applicable.
  pub fn visit_ty_ids_mut(&mut self, f: &mut impl FnMut(&mut TyId)) {
    match self {
      Insn::ConstInt { ty_id, .. }
      | Insn::ConstFloat { ty_id, .. }
      | Insn::ConstBool { ty_id, .. }
      | Insn::ConstString { ty_id, .. }
      | Insn::Load { ty_id, .. }
      | Insn::Store { ty_id, .. }
      | Insn::Drop { ty_id, .. }
      | Insn::Return { ty_id, .. }
      | Insn::Call { ty_id, .. }
      | Insn::BinOp { ty_id, .. }
      | Insn::UnOp { ty_id, .. }
      | Insn::ConstDef { ty_id, .. }
      | Insn::VarDef { ty_id, .. }
      | Insn::Directive { ty_id, .. }
      | Insn::Template { ty_id, .. }
      | Insn::ArrayLiteral { ty_id, .. }
      | Insn::ArrayIndex { ty_id, .. }
      | Insn::ArrayStore { ty_id, .. }
      | Insn::ArrayLen { ty_id, .. }
      | Insn::ArrayPush { ty_id, .. }
      | Insn::ArrayPop { ty_id, .. }
      | Insn::TupleLiteral { ty_id, .. }
      | Insn::TupleIndex { ty_id, .. }
      | Insn::EnumConstruct { ty_id, .. }
      | Insn::StructConstruct { ty_id, .. }
      | Insn::FieldStore { ty_id, .. } => f(ty_id),
      Insn::Cast { from_ty, to_ty, .. } => {
        f(from_ty);
        f(to_ty);
      }
      Insn::ArrayTyDef {
        array_ty, elem_ty, ..
      } => {
        f(array_ty);
        f(elem_ty);
      }
      Insn::MapTyDef { map_ty, .. } => {
        f(map_ty);
      }
      Insn::VecTyDef { vec_ty, .. } => {
        f(vec_ty);
      }
      Insn::SetTyDef { set_ty, .. } => {
        f(set_ty);
      }
      // Type-definition / signature-carrying insns. The
      // executor's post-pass resolve walker depends on
      // these being visited so generic param / field types
      // don't leak into SIR as unresolved inference vars.
      Insn::FunDef {
        params, return_ty, ..
      } => {
        for (_, ty) in params {
          f(ty);
        }
        f(return_ty);
      }
      Insn::StructDef { ty_id, fields, .. } => {
        f(ty_id);

        for (_, fty, _) in fields {
          f(fty);
        }
      }
      Insn::EnumDef {
        ty_id, variants, ..
      } => {
        f(ty_id);

        for (_, _, field_tys) in variants {
          for fty in field_tys {
            f(fty);
          }
        }
      }
      Insn::ChannelCreate { elem_ty, .. } => f(elem_ty),
      Insn::ChannelSend { ty_id, .. }
      | Insn::ChannelRecv { ty_id, .. }
      | Insn::TaskSpawn { ty_id, .. }
      | Insn::TaskAwait { ty_id, .. } => f(ty_id),
      Insn::SelectWait { elem_ty, .. } => f(elem_ty),
      Insn::SelectRecv { ty_id, .. } => f(ty_id),
      Insn::TaskCancelled { ty_id, .. } => f(ty_id),
      Insn::TaskCancel { .. } => {}
      Insn::StrSlice { ty_id, .. } => f(ty_id),
      Insn::ToStr { src_ty, .. } => f(src_ty),
      Insn::StringFormat { ty_id, .. } => f(ty_id),
      Insn::ChannelClose { .. }
      | Insn::ModuleLoad { .. }
      | Insn::PackDecl { .. }
      | Insn::PackLink { .. }
      | Insn::Label { .. }
      | Insn::Jump { .. }
      | Insn::BranchIfNot { .. }
      | Insn::StyleSheet { .. }
      | Insn::NurseryBegin { .. }
      | Insn::NurseryEnd { .. }
      | Insn::TestBegin { .. }
      | Insn::TestRun { .. }
      | Insn::TestSummary
      | Insn::Nop => {}
      Insn::CoerceToDyn { concrete_ty, .. } => f(concrete_ty),
      Insn::DynDispatch { ty_id, .. } => f(ty_id),
    }
  }
}

impl Default for Sir {
  fn default() -> Self {
    Self::new()
  }
}

/// SIR Instructions - minimal set for current executor
#[derive(Clone, Debug, PartialEq)]
pub enum Insn {
  /// Constant integer literal.
  ConstInt {
    dst: ValueId,
    value: u64,
    ty_id: TyId,
  },
  /// Constant float literal.
  ConstFloat {
    dst: ValueId,
    value: f64,
    ty_id: TyId,
  },
  /// Constant boolean value
  ConstBool {
    dst: ValueId,
    value: bool,
    ty_id: TyId,
  },
  /// Constant string value (interned as Symbol).
  ConstString {
    dst: ValueId,
    symbol: Symbol,
    ty_id: TyId,
  },
  /// Variable definition (compile-time binding).
  VarDef {
    name: Symbol,
    ty_id: TyId,
    init: Option<ValueId>,
    mutability: Mutability,
    pubness: Pubness,
  },
  /// Compile-time constant definition: `val X: int = 42;`
  /// No stack slot — value is inlined at every use site.
  ConstDef {
    name: Symbol,
    ty_id: TyId,
    value: ValueId,
    pubness: Pubness,
  },
  /// Store to variable/memory
  Store {
    name: Symbol,   // Variable to store to
    value: ValueId, // Value to store
    ty_id: TyId,    // Type of value
  },
  /// Scope-exit drop marker for an owned local. Emitted by
  /// the executor when a binding's scope closes; the
  /// ownership pass elides it when the value was moved/freed
  /// or its type has no destructor, and codegen lowers a
  /// survivor to a call to the type's consuming destructor.
  Drop {
    local: Symbol, // Binding being dropped
    ty_id: TyId,   // Its type (resolves the destructor)
  },
  /// Function definition
  FunDef {
    name: Symbol,
    params: Vec<(Symbol, TyId)>,
    return_ty: TyId,
    body_start: u32,
    kind: FunctionKind,
    pubness: Pubness,
    /// Receiver mode of the first parameter. `Write`
    /// (`mut self`) is consumed at every dot-call site to
    /// enforce that the receiver's binding is `mut`;
    /// `Consume` (`own self`) moves the receiver. Non-methods
    /// and `self`-only methods are `None` / `Read`.
    self_kind: SelfKind,
    /// C symbol override from a `%% link_name = "X".`
    /// attribute. `Some(sym)` directs codegen to use the
    /// interned string verbatim as the C symbol (after
    /// the platform leading underscore) instead of the
    /// `_<zo_name>` default. Only meaningful when `kind`
    /// is `FunctionKind::Intrinsic`.
    link_name: Option<Symbol>,
    /// The pack this function was declared inside (or
    /// `None` for top-level). Codegen reads this to
    /// route a `pub ffi`'s `extern_used` symbol to the
    /// pack's `#link` dylib. Without this field, codegen
    /// would have to infer the owning pack via a
    /// positional `PackDecl` scan — which misattributes
    /// user-level FFIs to whichever preload pack landed
    /// last in the merged SIR.
    owning_pack: Option<Symbol>,
    /// Source span of the function's introducer — the
    /// `fun` keyword or, when more readable, the identifier
    /// name. Lets diagnostics anchored at the FunDef
    /// (DCE rationale, unused-function warnings, link-name
    /// conflicts) point users back at the source.
    /// `Span::ZERO` for synthetic functions generated by
    /// the executor (closures, monomorphized methods).
    span: Span,
    /// `true` when declared with the `test` modifier.
    is_test: bool,
  },
  /// Return from function
  Return {
    value: Option<ValueId>, // None for void returns
    ty_id: TyId,
  },
  /// Function call.
  ///
  /// @note — `callee_pack` carries the resolved owning
  /// pack of the target `FunDef`. Codegen forms its asm
  /// label from `(callee_pack, name)` so two modules can
  /// emit `FunDef`s with the same bare `name` without
  /// link-time collision. `None` selects the global
  /// namespace (FFI extern symbols, `main`, preload-
  /// injected helpers).
  Call {
    dst: ValueId,
    name: Symbol,
    callee_pack: Option<Symbol>,
    args: Vec<ValueId>,
    ty_id: TyId, // Return type
  },
  /// Load a parameter or local into an SSA value.
  Load {
    dst: ValueId,
    src: LoadSource,
    ty_id: TyId,
  },
  /// Binary operation
  BinOp {
    dst: ValueId, // Destination SSA value
    op: BinOp,
    lhs: ValueId,
    rhs: ValueId,
    ty_id: TyId,
  },
  /// Unary operation
  UnOp {
    dst: ValueId,
    op: UnOp,
    rhs: ValueId,
    ty_id: TyId,
  },
  /// Directive execution (e.g., #dom, #run)
  Directive {
    name: Symbol,
    value: ValueId,
    ty_id: TyId,
  },
  /// Module import — resolved at compile time. `kind`
  /// distinguishes qualified vs glob vs selective forms;
  /// the executor classifies it from the load's child tree
  /// (`*` → Glob, `(…)` → Selective, otherwise Qualified).
  /// `pubness` follows Rust's `pub use`: `pub load X::*;`
  /// re-exports X through the current module's surface;
  /// plain `load X::*;` is private — X's symbols are
  /// visible only inside this module's body.
  ModuleLoad {
    path: Vec<Symbol>,
    kind: ImportKind,
    pubness: Pubness,
  },
  /// Pack declaration — defines a namespace.
  PackDecl { name: Symbol, pubness: Pubness },
  /// Pack-level dylib link metadata produced by a
  /// `#link { ... }` directive. The executor
  /// pre-resolves the host's `system → vendor` chain and
  /// stores the outcome in `resolution` so codegen stays
  /// a pure data transform.
  PackLink {
    pack: Symbol,
    spec: LinkSpec,
    resolution: LinkResolution,
  },
  /// The branch target label.
  Label { id: u32 },
  /// The unconditional jump to a label.
  Jump { target: u32 },
  /// The conditional branch — jump to target if false.
  BranchIfNot { cond: ValueId, target: u32 },
  /// Array literal: [e0, e1, ..., eN].
  ArrayLiteral {
    dst: ValueId,
    elements: Vec<ValueId>,
    ty_id: TyId,
  },
  /// Array index: arr[idx].
  ArrayIndex {
    dst: ValueId,
    array: ValueId,
    index: ValueId,
    ty_id: TyId,
  },
  /// Array length: arr.len.
  ArrayLen {
    dst: ValueId,
    array: ValueId,
    ty_id: TyId,
  },
  /// Array push: arr.push(value). Side effect — mutates len.
  /// `owner` is the receiver's local symbol (when the
  /// receiver is a bare ident). Codegen needs it to write
  /// the realloc'd pointer back to the local's stack slot
  /// — without it, the backend would have to scan SIR for
  /// the `Insn::Load` that produced `array`.
  ArrayPush {
    array: ValueId,
    value: ValueId,
    ty_id: TyId,
    owner: Option<Symbol>,
  },
  /// Array pop: val = arr.pop(). Decrements len, returns last.
  ArrayPop {
    dst: ValueId,
    array: ValueId,
    ty_id: TyId,
  },
  /// Tuple literal: (e0, e1, ..., eN).
  TupleLiteral {
    dst: ValueId,
    elements: Vec<ValueId>,
    ty_id: TyId,
  },
  /// Tuple/struct field read: tup.N (compile-time index).
  TupleIndex {
    dst: ValueId,
    tuple: ValueId,
    index: u32,
    ty_id: TyId,
  },
  /// Struct field write: struct.N = value.
  FieldStore {
    base: ValueId,
    index: u32,
    value: ValueId,
    ty_id: TyId,
  },
  /// Array element write: arr[i] = value.
  ArrayStore {
    array: ValueId,
    index: ValueId,
    value: ValueId,
    ty_id: TyId,
  },
  /// Enum type definition.
  EnumDef {
    name: Symbol,
    ty_id: TyId,
    /// (variant_name, discriminant, field_types).
    variants: Vec<(Symbol, u32, Vec<TyId>)>,
    pubness: Pubness,
  },
  /// Enum variant construction: `Foo::Ok(42)`.
  EnumConstruct {
    dst: ValueId,
    enum_name: Symbol,
    variant: u32,
    fields: Vec<ValueId>,
    ty_id: TyId,
  },
  /// Struct type definition.
  StructDef {
    name: Symbol,
    ty_id: TyId,
    /// (field_name, field_ty, has_default).
    fields: Vec<(Symbol, TyId, bool)>,
    pubness: Pubness,
  },
  /// Array type description — emitted once per unique
  /// `[]T` type the executor encounters. Codegen consumes
  /// this to populate its `array_metas` so `showln(arr)` can
  /// walk elements and format each one using the element
  /// type's writer. Mirrors `EnumDef`'s role for enum pretty-
  /// printing.
  /// `size = Some(n)` for statically-sized `[N]T` arrays
  /// (stack-allocatable, no growth); `None` for the dynamic
  /// `[]T` shape (heap-allocated, growable via `_realloc`).
  /// Codegen reads `size` to pick the `ArrayLiteral` lowering
  /// path.
  ArrayTyDef {
    array_ty: TyId,
    elem_ty: TyId,
    size: Option<u32>,
  },
  /// HashMap type description — emitted once per
  /// `HashMap<K, V>::new()` lowering so codegen can
  /// look up the per-element format kinds when
  /// `showln(m)` runs. `key_fmt` / `val_fmt` are the
  /// `MapFmt` discriminants the runtime expects (see
  /// `zo-runtime/src/map.rs`): `0=Int`, `1=Bool`,
  /// `2=Char`, `3=Str`, `4=Float`. Codegen routes the
  /// pretty-print call to `_zo_map_show` with these
  /// kinds so each entry formats as `key: value` for
  /// every supported scalar key/value type.
  MapTyDef {
    map_ty: TyId,
    key_fmt: u32,
    val_fmt: u32,
  },
  /// `Vec<$T>` type description — emitted once per
  /// instantiation. Same role as `MapTyDef` for the Vec
  /// pretty-printer: `elem_fmt` is the `MapFmt`
  /// discriminant for the element kind, consumed by
  /// codegen to route `showln(v)` to `_zo_vec_show`.
  VecTyDef { vec_ty: TyId, elem_fmt: u32 },
  /// `HashSet<$K>` type description — emitted once per
  /// instantiation. `key_fmt` carries the element's
  /// `MapFmt` discriminant; codegen routes
  /// `showln(s)` to `_zo_set_show`.
  SetTyDef { set_ty: TyId, key_fmt: u32 },
  /// Struct construction: `Span { lo: 0, hi: 10 }`.
  StructConstruct {
    dst: ValueId,
    struct_name: Symbol,
    fields: Vec<ValueId>,
    ty_id: TyId,
  },
  /// Template literal (fragment or HTML tag)
  Template {
    id: ValueId,
    name: Option<Symbol>,
    ty_id: TyId,
    commands: Vec<UiCommand>,
    /// Reactive bindings. When any bound variable changes,
    /// the listed command(s) must be re-patched with the new
    /// value. Text bindings target `UiCommand::Text(_)`
    /// content; attribute bindings target a named attribute
    /// on a `UiCommand::Element` — the runtime uses
    /// `UiCommand::set_attr` to apply the update.
    bindings: TemplateBindings,
  },
  /// Stylesheet declaration: `$: { ... }` or `pub $: { ... }`.
  StyleSheet {
    css: String,
    scope: StyleScope,
    scope_hash: Option<String>,
  },
  /// Type cast: `expr as Type`.
  Cast {
    dst: ValueId,
    src: ValueId,
    from_ty: TyId,
    to_ty: TyId,
  },
  /// Dead instruction — replaces folded operands in-place
  /// so instruction indices stay stable.
  Nop,

  /// Coerce a concrete value to its `any <Abstract>`
  /// fat-pointer. Heap-boxed (16 bytes:
  /// `[data_ptr, vtable_ptr]`) so returning a coerced
  /// value across a function boundary stays sound
  /// without a borrow checker.
  CoerceToDyn {
    dst: ValueId,
    src: ValueId,
    abstract_name: Symbol,
    concrete_ty: TyId,
  },

  /// Dynamic dispatch through a fat-pointer's vtable.
  /// `recv` is the fat-pointer ValueId, `method_index`
  /// is the slot inside `AbstractDef.methods`.
  DynDispatch {
    dst: ValueId,
    recv: ValueId,
    method_index: u32,
    abstract_name: Symbol,
    /// Carried for pp / diagnostics; dispatch resolves
    /// through `method_index`.
    method_name: Symbol,
    args: Vec<ValueId>,
    ty_id: TyId,
  },

  // ===== STRUCTURED CONCURRENCY =====
  //
  // Typed SIR carriers for the surface-level
  // `nursery { }` / `spawn` / `await` / `channel()` /
  // `tx.send` / `rx.recv`. ARM codegen lowers these to
  // BL calls into `zo-runtime`.
  /// Create a channel pair. Emits one runtime call that
  /// returns a single channel handle; `tx` and `rx` bind
  /// that handle under two separate ValueIds to match the
  /// surface tuple-destructure `imu (tx, rx) := channel()`.
  /// `elem_ty` is the element type `T` shared by send and
  /// recv. `capacity == 0` is unbuffered (rendezvous).
  /// `capacity` must be an integer literal — enforced by
  /// the executor at the built-in `channel()` call site.
  ChannelCreate {
    dst: ValueId,
    elem_ty: TyId,
    capacity: u32,
  },
  /// Push `value` onto `channel`. Blocks if the channel is
  /// bounded and its buffer is full. `ty_id` is the element
  /// type, stored on the insn so the validator can assert
  /// `value`'s ty matches without consulting the channel's
  /// definition insn.
  ChannelSend {
    channel: ValueId,
    value: ValueId,
    ty_id: TyId,
  },
  /// Pop a value off `channel` into `dst`. Blocks until a
  /// value is available. `ty_id` is the element type and
  /// also the type of `dst`.
  ChannelRecv {
    dst: ValueId,
    channel: ValueId,
    ty_id: TyId,
  },
  /// Close the channel — wakes every parked sender and
  /// receiver so they observe the closed state. After
  /// close, further `ChannelSend` panics and
  /// `ChannelRecv` drains remaining buffered values
  /// then returns zero-filled. Idempotent — repeated
  /// close is a no-op.
  ChannelClose { channel: ValueId },
  /// Spawn a task running `callee(args)`. `dst` is the task
  /// handle whose type is `Ty::Task(callee_return_ty)`.
  /// Must appear inside a `NurseryBegin` / `NurseryEnd`
  /// span — enforced by the executor, not the validator.
  ///
  /// `kind` distinguishes the two-tier spawn model:
  /// `Green` multiplexes on the current scheduler
  /// (cheap, cooperative), `Thread` spawns a dedicated
  /// OS thread (expensive, preemptive, real multi-core
  /// parallelism).
  TaskSpawn {
    dst: ValueId,
    callee: Symbol,
    /// Pack the spawned callee belongs to. Codegen pairs
    /// it with `callee` to form the `(name, owning_pack)`
    /// key for the address fixup so cross-module same-
    /// bare-name targets stay disambiguated.
    callee_pack: Option<Symbol>,
    args: Vec<ValueId>,
    ty_id: TyId,
    kind: SpawnKind,
  },
  /// Suspend until `task` completes, then bind the task's
  /// result value to `dst`. `ty_id` is the unwrapped result
  /// type — the `T` inside `Ty::Task(T)`, not the handle
  /// type itself.
  TaskAwait {
    dst: ValueId,
    task: ValueId,
    ty_id: TyId,
  },
  /// Selective receive — atomic wait on N channels.
  /// `out_which` receives the 0-based arm index of
  /// the channel that fired. Paired with a following
  /// `SelectRecv` that reads the received value out of
  /// the scratch buffer into a register. The split
  /// keeps the insn-to-dst mapping single-valued so
  /// the liveness / register allocator stays simple.
  SelectWait {
    out_which: ValueId,
    chans: Vec<ValueId>,
    elem_ty: TyId,
  },
  /// Companion to `SelectWait` — produces `dst` by
  /// loading the runtime-written value from the select
  /// scratch buffer. `which` reads the arm index from
  /// the preceding `SelectWait` purely to anchor
  /// liveness; codegen doesn't consume it. `chans_len`
  /// lets the backend compute the scratch buf offset
  /// (`nchans * 8` bytes of pointers precede the
  /// output buffer in the frame).
  SelectRecv {
    dst: ValueId,
    which: ValueId,
    ty_id: TyId,
    chans_len: u32,
  },
  /// Read a task handle's cancellation flag. Surface
  /// form is `t.cancelled()` where `t: Task<T>`. Lowers
  /// to `BL _zo_task_is_cancelled(task)` — runtime does
  /// a relaxed atomic load of the shared flag. `dst`
  /// receives the resulting `bool`.
  TaskCancelled {
    dst: ValueId,
    task: ValueId,
    ty_id: TyId,
  },
  /// Signal a task to cancel. Surface form is
  /// `t.cancel()` where `t: Task<T>`. Lowers to
  /// `BL _zo_task_cancel(task)` — runtime latches the
  /// shared cancel flag. Cooperative: the task itself
  /// must poll `.cancelled()` (or the runtime must
  /// cascade the flag at a yield point) for the
  /// cancellation to have any observable effect.
  TaskCancel { task: ValueId },
  /// Runtime string slice `src[lo..hi]`. Lowered to
  /// `BL _zo_str_slice(src, lo, hi)`; returns a fresh
  /// heap-backed `str`. Emitted by the executor when
  /// the bounds are not compile-time constants
  /// (compile-time bounds still fold to `ConstString`).
  StrSlice {
    dst: ValueId,
    src: ValueId,
    lo: ValueId,
    hi: ValueId,
    ty_id: TyId,
  },
  /// Convert a typed value to its `str` representation.
  /// Identity for `str`; codegen dispatches on `src_ty`
  /// to the right runtime helper (`_zo_int_to_str`, etc.).
  ToStr {
    dst: ValueId,
    src: ValueId,
    src_ty: TyId,
  },
  /// Concatenate N already-str-typed segments into one
  /// `str` via a single heap allocation. Codegen builds
  /// a pointer array on the stack and calls
  /// `_zo_str_multi_concat`.
  StringFormat {
    dst: ValueId,
    segments: Vec<ValueId>,
    ty_id: TyId,
  },
  /// Runtime byte-wise `str` equality. Lowered to
  /// Open a nursery scope. Every `TaskSpawn` between this
  /// insn and its matching `NurseryEnd` is scoped to this
  /// nursery: on scope exit, all such tasks are joined
  /// (or cancelled if any sibling panicked). `kind`
  /// distinguishes a plain `nursery { }` from a
  /// `supervise { }` scope — the latter additionally
  /// propagates panics upward through the enclosing
  /// task's cascade chain.
  NurseryBegin { label: u32, kind: NurseryKind },
  /// Close the nursery opened by a matching `NurseryBegin`
  /// with the same `label`. Emits the implicit join of
  /// every scoped task.
  NurseryEnd { label: u32 },
  /// Print "running N tests ..." header.
  TestBegin { count: u32 },
  /// Execute one `test fun` via the runtime's test shim.
  /// Codegen loads the callee's address (same ADR+fixup
  /// as TaskSpawn) and the name as a string literal.
  TestRun {
    callee: Symbol,
    callee_pack: Option<Symbol>,
  },
  /// Print the test summary and exit non-zero on failure.
  TestSummary,
}

/// Discriminator for `Insn::NurseryBegin` — plain
/// scope vs supervised scope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NurseryKind {
  /// Plain `nursery { }` — siblings cancel on sibling
  /// panic; error re-raises at scope exit.
  Scoped,
  /// `supervise { }` — in addition to `Scoped`
  /// semantics, the panic propagates through the
  /// enclosing task's cascade chain rather than
  /// stopping at this scope.
  Supervised,
}

/// Discriminator for `Insn::TaskSpawn` — green task on
/// the current scheduler vs fresh OS thread.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpawnKind {
  /// Multiplex on the current scheduler. Cheap
  /// (~KB stack), cooperative, yields at channel /
  /// await boundaries.
  Green,
  /// Fresh OS thread via `pthread_create`. Expensive
  /// (~MB stack), kernel-preemptive, real multi-core
  /// parallelism. Used via the `spawn thread fn()`
  /// surface form.
  Thread,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BinOp {
  /// `+`
  Add,
  /// `-`
  Sub,
  /// `*`
  Mul,
  /// `/`
  Div,
  /// `%`
  Rem,
  /// `==`
  Eq,
  /// `!=`
  Neq,
  /// `<`
  Lt,
  /// `<=`
  Lte,
  /// `>`
  Gt,
  /// `>=`
  Gte,
  /// `&&`
  And,
  /// `||`
  Or,
  /// `&`
  BitAnd,
  /// `|`
  BitOr,
  /// `^`
  BitXor,
  /// `<<`
  Shl,
  /// `>>`
  Shr,
  /// `++` (string concatenation)
  Concat,
}

impl BinOp {
  /// Gets the related [`BinOp`] from a [`Token`].
  pub const fn from(&self, kind: Token) -> Option<BinOp> {
    const BINOPS: [Option<BinOp>; 256] = {
      let mut table = [None; 256];
      table[Token::Plus as usize] = Some(BinOp::Add);
      table[Token::Minus as usize] = Some(BinOp::Sub);
      table[Token::Star as usize] = Some(BinOp::Mul);
      table[Token::Slash as usize] = Some(BinOp::Div);
      table[Token::Percent as usize] = Some(BinOp::Rem);
      table[Token::Eq as usize] = Some(BinOp::Eq);
      table[Token::BangEq as usize] = Some(BinOp::Neq);
      table[Token::Lt as usize] = Some(BinOp::Lt);
      table[Token::LtEq as usize] = Some(BinOp::Lte);
      table[Token::Gt as usize] = Some(BinOp::Gt);
      table[Token::GtEq as usize] = Some(BinOp::Gte);
      table[Token::AmpAmp as usize] = Some(BinOp::And);
      table[Token::PipePipe as usize] = Some(BinOp::Or);
      table[Token::Amp as usize] = Some(BinOp::BitAnd);
      table[Token::Pipe as usize] = Some(BinOp::BitOr);
      table[Token::Caret as usize] = Some(BinOp::BitXor);
      table[Token::LShift as usize] = Some(BinOp::Shl);
      table[Token::RShift as usize] = Some(BinOp::Shr);
      table[Token::PlusPlus as usize] = Some(BinOp::Concat);
      table
    };

    BINOPS[kind as usize]
  }
}

/// Represents unary operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
  /// Arithmetic negation — `-x`.
  Neg,
  /// Logical not — `!x`.
  Not,
  /// Bitwise not — currently no surface token; reachable
  /// only through type-checker tests and codegen-clif.
  /// Keep until either the grammar grows `~` or the rule
  /// is dropped along with its tests.
  BitNot,
}

impl UnOp {
  /// Gets the related [`UnOp`] from a [`Token`].
  pub const fn from(&self, kind: Token) -> Option<UnOp> {
    const UNOPS: [Option<UnOp>; 256] = {
      let mut table = [None; 256];
      table[Token::Bang as usize] = Some(UnOp::Not);
      table[Token::Minus as usize] = Some(UnOp::Neg);
      table
    };

    UNOPS[kind as usize]
  }
}
