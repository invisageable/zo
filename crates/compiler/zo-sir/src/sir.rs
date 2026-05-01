use zo_interner::Symbol;
use zo_token::Token;
use zo_ty::Mutability;
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

/// Source of a Load instruction — either a function parameter
/// or a local variable on the stack.
#[derive(Clone, Debug, PartialEq)]
pub enum LoadSource {
  /// Function parameter by index (X0-X7 or D0-D7).
  Param(u32),
  /// Local variable by symbol (stack-allocated).
  Local(Symbol),
}

/// Represents a semantic intermediate representation.
#[derive(Debug)]
pub struct Sir {
  /// The linear array of SIR instructions.
  pub instructions: Vec<Insn>,
  /// The next value ID for SSA.
  pub next_value_id: u32,
  /// The next label ID for branch targets.
  pub next_label_id: u32,
}

impl Sir {
  /// Creates a new [`SirBuilder`] instance.
  pub fn new() -> Self {
    Self {
      instructions: Vec::with_capacity(1024),
      next_value_id: 0,
      next_label_id: 0,
    }
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
    let value_id = match &insn {
      // All value-producing instructions have explicit dst.
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
      // Concurrency value-producing insns.
      | Insn::ChannelCreate { dst, .. }
      | Insn::ChannelRecv { dst, .. }
      | Insn::TaskSpawn { dst, .. }
      | Insn::TaskAwait { dst, .. }
      | Insn::TaskCancelled { dst, .. }
      | Insn::StrSlice { dst, .. } => *dst,
      // Template uses `id` as its value.
      Insn::Template { id, .. } => *id,
      // Non-value instructions.
      _ => ValueId(u32::MAX),
    };

    self.instructions.push(insn);

    value_id
  }

  /// Offsets all `ValueId`s in instructions by `offset`.
  /// Used when prepending module SIR to avoid ID collisions.
  pub fn offset_value_ids(instructions: &mut [Insn], offset: u32) {
    for insn in instructions.iter_mut() {
      insn.visit_value_ids_mut(&mut |v| v.0 += offset);
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
      | Insn::Nop => {}
      Insn::VarDef { init, .. } => {
        if let Some(v) = init {
          f(v);
        }
      }
      Insn::Store { value, .. } => f(value),
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
      Insn::NurseryBegin { .. } | Insn::NurseryEnd { .. } => {}
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
      Insn::ChannelClose { .. }
      | Insn::ModuleLoad { .. }
      | Insn::PackDecl { .. }
      | Insn::Label { .. }
      | Insn::Jump { .. }
      | Insn::BranchIfNot { .. }
      | Insn::StyleSheet { .. }
      | Insn::NurseryBegin { .. }
      | Insn::NurseryEnd { .. }
      | Insn::Nop => {}
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
  /// Function definition
  FunDef {
    name: Symbol,
    params: Vec<(Symbol, TyId)>,
    return_ty: TyId,
    body_start: u32,
    kind: FunctionKind,
    pubness: Pubness,
    /// `true` when the first parameter was declared as
    /// `mut self`. Set only on apply-context methods —
    /// non-method functions and `self`-only methods are
    /// `false`. Consumed at every dot-call site to enforce
    /// that the receiver's binding is `mut`.
    mut_self: bool,
  },
  /// Return from function
  Return {
    value: Option<ValueId>, // None for void returns
    ty_id: TyId,
  },
  /// Function call
  Call {
    dst: ValueId,
    name: Symbol,
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
  /// Module import — resolved at compile time.
  ModuleLoad {
    path: Vec<Symbol>,
    imported_symbols: Vec<Symbol>,
  },
  /// Pack declaration — defines a namespace.
  PackDecl { name: Symbol, pubness: Pubness },
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
