use zo_constant_folding::{ConstFold, FoldResult, Operand};
use zo_error::{Error, ErrorKind};
use zo_interner::{Interner, Symbol};
use zo_reporter::report_error;
use zo_sir::{BinOp, Insn, LoadSource, Sir, TemplateBindings, UnOp};
use zo_span::Span;
use zo_template_optimizer::TemplateOptimizer;
use zo_token::{InterpSegment, LiteralStore, Token};
use zo_tree::{NodeHeader, NodeValue, Tree};
use zo_ty::{Annotation, Mutability, Ty, TyId};
use zo_ty_checker::TyChecker;
use zo_ui_protocol::{
  Attr, ElementTag, EventKind, PropValue, StyleScope, UiCommand,
};
use zo_value::{
  CaptureInfo, ClosureValue, FunDef, FunctionKind, Local, LocalKind, Pubness,
  Value, ValueId, ValueStorage,
};

use std::cell::Cell;
use std::collections::{HashMap, HashSet};

/// Scope frame for variable tracking
pub struct ScopeFrame {
  // Start index in locals array
  start: u32,
  // Number of locals in this scope
  count: u32,
}

/// A single instantiation request recorded at call site
/// and consumed by the re-execution pass.
///
/// `(mangled, base_name, concrete_tys, closure_subs)`:
///
/// - `concrete_tys` — resolved `TyId` per `$T` in the
///   generic's declaration order (empty for non-generic
///   bases). Indexed by position.
/// - `closure_subs` — `(param_sym, closure_fn_sym)` pairs
///   for Fn-typed params specialized to concrete closures
///   (empty when no closure was passed). The re-execution
///   pass rewrites the param's local from a runtime value
///   into a `Value::Closure` binding so `param(x)` in the
///   body dispatches directly to the concrete closure.
///
/// Generic-type and closure-param instantiations share the
/// same pipeline — `concrete_tys` drives type substitution,
/// `closure_subs` drives parameter-to-closure binding, and
/// either (or both) can be empty.
type Instantiation = (Symbol, Symbol, Vec<TyId>, Vec<(Symbol, Symbol)>);

/// Executor implements compile-time execution of HIR to produce SIR
///
/// Following the manifesto (line 176): "type checking is evaluation"
/// This means we execute the parse tree and produce typed SIR as output
pub struct Executor<'a> {
  /// Parse tree to execute
  tree: &'a Tree,
  /// String interner — mutable so the executor can intern
  /// new symbols during compile-time execution (e.g.
  /// interpolation desugaring).
  interner: &'a mut Interner,
  /// Literal values from tokenization
  literals: &'a LiteralStore,
  /// Operand stack (4 bytes per value - just indices!)
  value_stack: Vec<ValueId>,
  /// Type stack (4 bytes per type)
  ty_stack: Vec<TyId>,
  /// All values stored in side arrays
  values: ValueStorage,
  /// Block boundaries
  scope_stack: Vec<ScopeFrame>,
  /// All local variables (dense array)
  locals: Vec<Local>,
  /// Builds SIR as we execute (placeholder for now)
  sir: Sir,
  /// The type checker instance (borrowed from caller).
  ty_checker: &'a mut TyChecker,
  /// Type annotations for HIR nodes
  annotations: Vec<Annotation>,
  /// Maps value_stack indices to SIR ValueIds for operands
  sir_values: Vec<ValueId>,
  /// Function definitions
  funs: Vec<FunDef>,
  /// Current function context (if we're inside a function)
  current_function: Option<FunCtx>,
  /// Save-stack for nested `fun` inside a function body.
  /// Mirrors the `execute_closure` pattern: when the LBrace
  /// of a nested fun takes over `current_function`, we push
  /// the outer state here; the matching RBrace pops and
  /// restores it so the outer fun can resume. Per the zo
  /// grammar, any `item` (fun, struct, enum, type, val, …)
  /// is allowed at statement level — this is the first
  /// piece that lifts the "items only at top level"
  /// limitation for `fun_decl`.
  saved_outer_funs: Vec<SavedOuterFun>,
  /// Pending function definition (waiting for LBrace)
  pending_function: Option<FunDef>,
  /// True when the pending function has `-> Type` annotation.
  pending_fn_has_return_annotation: bool,
  /// Counter for generating unique template IDs
  template_counter: u32,
  /// Pending variable name from imu/mut for template assignment
  pending_var_name: Option<Symbol>,
  /// Counter for unique widget IDs (buttons, inputs)
  widget_counter: Cell<u32>,
  /// The pending branch contexts for control flow.
  branch_stack: Vec<BranchCtx>,
  /// Monotonically increasing counter used to mint unique
  /// names for synthetic per-branch result locals
  /// (`__branch_result_N__`). see `PLAN_BRANCH_EXPR_PHI.md`.
  branch_result_counter: u32,
  /// Skip main-loop processing until this index.
  skip_until: usize,
  /// Pending variable declaration (deferred to Semicolon).
  pending_decl: Option<PendingDecl>,
  /// Pending assignment target name (deferred to Semicolon).
  pending_assign: Option<(Symbol, Span)>,
  /// Pending compound assignment (deferred to Semicolon).
  pending_compound: Option<(Symbol, BinOp, Span)>,
  /// Receiver of a field compound assign (e.g., `self`
  /// in `self.x += 1`). Set when the target is a field,
  /// consumed by `finalize_pending_compound`.
  pending_compound_receiver: Option<Symbol>,
  /// Array context stack: (is_indexing, stack_depth, array_name).
  array_ctx: Vec<(bool, usize, Option<Symbol>)>,
  /// Pending array element assignment (deferred to Semicolon).
  /// (array_sir, index_sir, array_name, span)
  pending_array_assign: Option<(ValueId, ValueId, Symbol, Span)>,
  /// Tuple context stack: stack_depth_at_open.
  tuple_ctx: Vec<usize>,
  /// Deferred binary operators waiting for RHS group to close.
  /// (op, lhs_value, lhs_ty, lhs_sir, node_idx)
  deferred_binops: Vec<(BinOp, ValueId, TyId, ValueId, usize)>,
  /// Deferred short-circuit logical ops waiting for RHS to
  /// be pushed onto the stacks. Each entry records the
  /// synthetic sink local and the end label emitted at the
  /// `&&`/`||` site — the RHS-finalization path stores into
  /// the sink, emits the end label, and loads the merged
  /// result. Parallel to `deferred_binops` but finalized via
  /// the φ-sink machinery instead of a plain `BinOp`.
  deferred_short_circuits: Vec<DeferredShortCircuit>,
  /// Counter for generating unique closure names.
  closure_counter: u32,
  /// Known enum types by name → (EnumTyId, TyId).
  enum_defs: Vec<(Symbol, zo_ty::EnumTyId, TyId)>,
  /// Imported enum defs awaiting lazy interning. Populated
  /// by `with_imports`, consumed by `execute_enum_access`
  /// on first reference.
  pending_imported_enums: Vec<zo_module_resolver::ExportedEnum>,
  /// Concrete type args from the last ext function call
  /// with a parameterized return type. Consumed by the
  /// match handler to type bindings correctly.
  /// Per-variable return type args from ext function calls.
  /// Keyed by the variable name (Symbol) that stores the
  /// call result. Used by the match handler to type bindings.
  var_return_type_args: HashMap<u32, Vec<zo_ty::Ty>>,
  /// Pending enum construction: (enum_name, variant_disc,
  /// variant_field_count, ty_id).
  pending_enum_construct: Option<(Symbol, u32, u32, TyId)>,
  /// Current `apply Type` context — the type name being
  /// applied. Methods get mangled as `Type::method`.
  apply_context: Option<Symbol>,
  /// Nested `pack` context — each entry is
  /// `(pack_name, rbrace_idx)` where `rbrace_idx` is the
  /// tree index of the pack's closing `}`. Functions
  /// declared inside get their name prefixed with the
  /// joined pack chain (`inner::inner2::hello`). The
  /// closing RBrace at a matching `rbrace_idx` pops the
  /// entry. Mirrors `apply_context` at nested-stack arity.
  pack_context: Vec<(Symbol, usize)>,
  /// Every simple pack name that has been declared
  /// anywhere in the program (e.g. `inner`, `inner2`).
  /// Needed so bare idents like `inner` and `inner2`
  /// inside `inner.inner2.hello()` are not rejected as
  /// undefined variables — they are namespace prefixes.
  /// Resolution into the mangled callee happens in
  /// `execute_potential_call`.
  pack_names: HashSet<Symbol>,
  /// Global compile-time constants (`val` at module level).
  /// Visible from all functions.
  global_constants: Vec<Local>,
  /// Active type parameters: `$T → TyId`. Set during
  /// generic function definition, cleared after.
  type_params: Vec<(Symbol, TyId)>,
  /// Generic constraints: `$T: Eq` maps the type param
  /// name to the abstract name. Verified at call site.
  type_constraints: HashMap<Symbol, Symbol>,
  /// Tree range `(start, end_exclusive)` of each generic
  /// function's whole declaration (from `Fun` through the
  /// closing `}`). Populated at `execute_fun` time for any
  /// function that has `<$T>` parameters. Consumed by the
  /// instantiation pass to re-execute the body per
  /// concrete-type substitution — producing fresh SIR
  /// directly instead of cloning-and-rewriting the generic
  /// form's SIR.
  generic_tree_ranges: HashMap<Symbol, (u32, u32)>,
  /// Name override applied to the next `execute_fun` call.
  /// Used by the instantiation pass: re-executing a generic
  /// parses the same `Fun` node, which would emit the
  /// generic name again; setting this overrides that to the
  /// mangled name (e.g. `are_equal__Point`).
  mono_name_override: Option<Symbol>,
  /// Generic instantiation requests captured during call
  /// resolution. Each entry is `(mangled, generic_name,
  /// subs)` where `subs` are the type substitutions
  /// (old inference var → concrete TyId) for the
  /// instantiation. Drained by the instantiation pass,
  /// which re-executes the generic's tree range with
  /// those substitutions bound in the ty_checker.
  pending_instantiations: Vec<Instantiation>,
  /// Mangled names whose body SIR has already been emitted
  /// via re-execution — dedup guard against repeated call
  /// sites and recursive generics.
  reexecuted_instantiations: std::collections::HashSet<Symbol>,
  /// Buffered closure SIR instructions. Closures emit
  /// their FunDef + body here during execution. Flushed
  /// to `self.sir` after the enclosing function's Return
  /// so DCE sees them as separate, non-nested functions.
  deferred_closures: Vec<Insn>,
  /// RParen index of a pending call detected via operator-
  /// skipping at LParen (`Ident Op LParen`). The main loop
  /// suppresses deferred binops until this RParen is reached,
  /// preventing call args from being consumed by the operator.
  pending_call_rparen: Option<usize>,
  /// Open direct-call depth. Incremented at every `is_call`
  /// LParen (when LParen is preceded by a function ident),
  /// decremented at the matching RParen. Used by
  /// `apply_deferred_short_circuit` to distinguish
  ///   `a || f(x)` — SC deferred *outside* the call; finalize
  ///   must wait until the call result is on the stack; and
  ///   `f(a || b)` — SC deferred *inside* the call; finalize
  ///   must happen before the call collects its args.
  /// Without this, both cases look identical by stack depth
  /// alone and the SC captures the wrong value.
  direct_call_depth: u32,
  /// Pending stylesheet commands collected from `$:` blocks.
  /// Injected into the next `Insn::Template`'s commands.
  pending_styles: Vec<UiCommand>,
  /// Reactive bindings collected during template execution,
  /// consumed when emitting `Insn::Template`. Split into text
  /// and attribute bindings so the runtime can dispatch each
  /// kind to the right patch path.
  template_bindings: zo_sir::TemplateBindings,
  /// Abstract definitions: `abstract Show { fun show(self); }`
  abstract_defs: HashMap<Symbol, AbstractDef>,
  /// Abstract implementations: `apply Show for Rect { ... }`
  /// Maps (abstract_name, target_type_name) → mangled method
  /// names.
  abstract_impls: HashMap<(Symbol, Symbol), Vec<Symbol>>,
  /// Signature-only pre-scan flag. When true, `execute_fun`
  /// registers the `FunDef` in `self.funs` and returns before
  /// any body-level state is touched (no `pending_function`,
  /// no scope push, no param-locals). Enables forward
  /// references between mutually recursive functions — call
  /// resolution in the main pass can look up callee
  /// signatures regardless of source order.
  prescan_only: bool,
}

/// An `abstract` definition (method signatures, no bodies).
pub struct AbstractDef {
  pub methods: Vec<AbstractMethod>,
}

/// A single method signature in an abstract definition.
pub struct AbstractMethod {
  pub name: Symbol,
  pub params: Vec<(Symbol, TyId)>,
  pub return_ty: TyId,
}

/// Deferred short-circuit operator, finalized when the RHS
/// expression has fully materialized on the stacks. See
/// `execute_logical_binop` and `apply_deferred_short_circuit`
/// for the control-flow shape.
///
/// Finalization requires *three* depth markers to avoid
/// capturing transient stack values that land mid-RHS
/// (e.g. a function-call arg being pushed before the call
/// itself emits) — the RHS is "done" only when we're back
/// at the pre-op stack/tuple/call depth and one new value
/// sits on top of the stacks.
struct DeferredShortCircuit {
  /// Synthetic `__branch_result_N__` local receiving both
  /// the LHS (already stored at the op site) and the RHS
  /// (stored on finalization) — mirrors the ternary φ sink.
  sink: Symbol,
  /// Label emitted after the RHS store, reached directly
  /// from the LHS short-circuit path.
  end_label: u32,
  /// `bool` type id, cached to avoid re-looking it up.
  bool_ty: TyId,
  /// `sir_values.len()` right after the LHS was popped, at
  /// the op site. The RHS is "on top" only when the stack
  /// reaches `pre_rhs_depth + 1`.
  pre_rhs_depth: usize,
  /// `tuple_ctx.len()` at defer time. Only finalize when the
  /// tuple context has returned to at most this depth —
  /// otherwise an inner tuple/group element push would be
  /// stolen as the RHS.
  pre_tuple_ctx_len: usize,
  /// `direct_call_depth` at defer time. Mirrors
  /// `pre_tuple_ctx_len` but for function-call scopes,
  /// which don't push `tuple_ctx`. Prevents premature
  /// finalization when the RHS *is* a call: args get pushed
  /// while inside the call, and firing on the first arg
  /// would capture it instead of the call result.
  pre_direct_call_depth: u32,
}

/// Deferred variable declaration, finalized at Semicolon.
struct PendingDecl {
  name: Symbol,
  is_mutable: bool,
  is_constant: bool,
  pubness: Pubness,
  /// Explicit type annotation, if provided.
  annotated_ty: Option<TyId>,
  /// Source span of the declaration (for error reporting).
  span: Span,
}
impl<'a> Executor<'a> {
  /// Creates a new [`Executor`] instance.
  pub fn new(
    tree: &'a Tree,
    interner: &'a mut Interner,
    literals: &'a LiteralStore,
    ty_checker: &'a mut TyChecker,
  ) -> Self {
    let capacity = tree.nodes.len();

    Self {
      tree,
      interner,
      literals,
      value_stack: Vec::with_capacity(capacity / 4),
      ty_stack: Vec::with_capacity(capacity / 4),
      values: ValueStorage::new(capacity),
      scope_stack: Vec::with_capacity(32),
      locals: Vec::with_capacity(capacity / 10),
      sir: Sir::new(),
      ty_checker,
      annotations: Vec::with_capacity(capacity),
      sir_values: Vec::with_capacity(capacity / 4),
      funs: Vec::with_capacity(capacity / 100), // Estimate function count
      current_function: None,
      saved_outer_funs: Vec::new(),
      pending_function: None,
      pending_fn_has_return_annotation: false,
      template_counter: 0,
      pending_var_name: None,
      widget_counter: Cell::new(0),
      branch_stack: Vec::with_capacity(8),
      branch_result_counter: 0,
      skip_until: 0,
      pending_decl: None,
      pending_assign: None,
      pending_compound: None,
      pending_compound_receiver: None,
      array_ctx: Vec::new(),
      pending_array_assign: None,
      tuple_ctx: Vec::new(),
      deferred_binops: Vec::new(),
      deferred_short_circuits: Vec::new(),
      closure_counter: 0,
      enum_defs: Vec::new(),
      pending_imported_enums: Vec::new(),
      var_return_type_args: HashMap::default(),
      generic_tree_ranges: HashMap::default(),
      mono_name_override: None,
      pending_instantiations: Vec::new(),
      reexecuted_instantiations: std::collections::HashSet::new(),
      pending_enum_construct: None,
      apply_context: None,
      pack_context: Vec::new(),
      pack_names: HashSet::new(),
      global_constants: Vec::new(),
      type_params: Vec::new(),
      type_constraints: HashMap::new(),
      deferred_closures: Vec::new(),
      pending_call_rparen: None,
      direct_call_depth: 0,
      pending_styles: Vec::new(),
      template_bindings: TemplateBindings::default(),
      abstract_defs: HashMap::new(),
      abstract_impls: HashMap::new(),
      prescan_only: false,
    }
  }

  /// Upsert a function definition into `self.funs`. Replaces
  /// any existing entry with the same name in place so pre-
  /// scan-registered signatures get updated with real
  /// `body_start` values during the main pass (and re-
  /// executed generic instantiations overwrite their stub).
  fn push_or_replace_fun(&mut self, def: FunDef) {
    if let Some(slot) = self.funs.iter_mut().find(|f| f.name == def.name) {
      *slot = def;
    } else {
      self.funs.push(def);
    }
  }

  /// Checks if the node immediately before `idx` is `Token::Pub`.
  fn is_pub(&self, idx: usize) -> bool {
    idx > 0
      && self
        .tree
        .nodes
        .get(idx - 1)
        .is_some_and(|n| n.token == Token::Pub)
  }

  /// Gets the value associated with a node (if any).
  fn node_value(&self, node_idx: usize) -> Option<NodeValue> {
    self.tree.value(node_idx as u32)
  }

  /// Extracts a symbol's string value from a node, owned.
  fn symbol_str(&self, idx: usize) -> String {
    self
      .node_value(idx)
      .and_then(|v| match v {
        NodeValue::Symbol(s) => Some(self.interner.get(s).to_owned()),
        _ => None,
      })
      .unwrap_or_default()
  }

  /// Gets the variable name from an imu/mut declaration
  fn get_var_name(&self, start_idx: usize, end_idx: usize) -> Option<Symbol> {
    // Look for the Ident token after imu/mut
    for idx in (start_idx + 1)..end_idx {
      if let Some(node) = self.tree.nodes.get(idx)
        && node.token == Token::Ident
        && let Some(NodeValue::Symbol(sym)) = self.node_value(idx)
      {
        return Some(sym);
      }
    }

    None
  }

  /// Gets type from a type value (if any).
  fn ty_value(&self, value_id: ValueId) -> Option<TyId> {
    let idx = value_id.0 as usize;

    if idx < self.values.kinds.len() {
      match self.values.kinds[idx] {
        Value::Type => {
          let type_idx = self.values.indices[idx] as usize;

          self.values.types.get(type_idx).copied()
        }
        _ => None,
      }
    } else {
      None
    }
  }

  /// Look up a local variable (if any).
  fn lookup_local(&self, name: Symbol) -> Option<&Local> {
    self.locals.iter().rev().find(|local| local.name == name)
  }

  /// Push a new scope.
  fn push_scope(&mut self) {
    self.scope_stack.push(ScopeFrame {
      start: self.locals.len() as u32,
      count: 0,
    });
  }

  /// Pops a scope and remove its locals.
  fn pop_scope(&mut self) {
    if let Some(frame) = self.scope_stack.pop() {
      self.locals.truncate(frame.start as usize);
    }
  }

  /// Pre-populates the executor with imported function
  /// definitions and constants so they're available during
  /// execution.
  pub fn with_imports(
    mut self,
    funs: Vec<FunDef>,
    vars: Vec<Local>,
    enums: Vec<zo_module_resolver::ExportedEnum>,
    abstract_defs: HashMap<Symbol, AbstractDef>,
  ) -> Self {
    self.funs = funs;
    self.locals.extend(vars);
    self.abstract_defs.extend(abstract_defs);

    // Defer enum interning to first use to avoid TyId counter
    // shifts that would pollute HM unification. Raw export
    // data is stored here and resolved lazily in
    // `execute_enum_access` / the Ident handler.
    self.pending_imported_enums = enums;

    self
  }

  /// Walks the Tree in signature-only mode and registers
  /// every top-level `fun` in `self.funs`. Enables forward
  /// references: when the main pass later encounters a call
  /// to a function defined lexically below, the callee's
  /// signature is already in `funs` and resolution succeeds
  /// without reordering source.
  ///
  /// Scope: top-level free functions only. Methods inside
  /// `apply Type { ... }` and closure bodies are not
  /// pre-scanned — mutual recursion across apply blocks or
  /// between closures needs a follow-up (declaration-
  /// collection pass that honors apply context).
  fn prescan_fun_signatures(&mut self) {
    self.prescan_only = true;

    let n = self.tree.nodes.len();
    let mut idx = 0;
    // Track brace depth so we only register funs at the
    // top level — not funs inside `apply`, `struct`,
    // `abstract`, or a closure body.
    let mut block_depth = 0i32;

    while idx < n {
      let tok = self.tree.nodes[idx].token;

      if block_depth == 0 && tok == Token::Fun {
        let header = self.tree.nodes[idx];
        let children_end = (header.child_start + header.child_count) as usize;

        self.execute_fun(idx, children_end);

        // Advance past the function's matching `}` so the
        // walker resumes at the next top-level item.
        idx = self.fun_body_end(idx, n);

        continue;
      }

      match tok {
        Token::LBrace => block_depth += 1,
        Token::RBrace => block_depth -= 1,
        _ => {}
      }

      idx += 1;
    }

    // Reset state that prescan mutated so the main pass
    // starts from a clean slate.
    self.prescan_only = false;
    self.skip_until = 0;
    self.pending_fn_has_return_annotation = false;
    self.type_params.clear();
    self.type_constraints.clear();
    self.apply_context = None;
  }

  /// Finds the tree index one past the matching `}` of the
  /// function whose header lives at `start_idx`. Used by the
  /// prescan walker to skip over bodies it doesn't execute.
  fn fun_body_end(&self, start_idx: usize, n: usize) -> usize {
    let mut i = start_idx + 1;

    while i < n && self.tree.nodes[i].token != Token::LBrace {
      i += 1;
    }

    if i >= n {
      return n;
    }

    let mut depth = 1i32;
    let mut j = i + 1;

    while j < n && depth > 0 {
      match self.tree.nodes[j].token {
        Token::LBrace => depth += 1,
        Token::RBrace => depth -= 1,
        _ => {}
      }

      if depth == 0 {
        break;
      }

      j += 1;
    }

    j + 1
  }

  /// Executes a parse tree in one pass to build semantic IR.
  pub fn execute(
    mut self,
  ) -> (
    Sir,
    Vec<Annotation>,
    Vec<FunDef>,
    HashMap<Symbol, AbstractDef>,
  ) {
    // Pre-scan top-level function signatures so mutual
    // recursion and out-of-order calls resolve during the
    // main pass.
    self.prescan_fun_signatures();

    for idx in 0..self.tree.nodes.len() {
      if idx < self.skip_until {
        continue;
      }

      let header = self.tree.nodes[idx];

      self.execute_node(&header, idx);

      // Apply deferred binary operators only when:
      // 1. We're not inside a tuple/grouping context.
      // 2. The RHS value has been pushed to the stack.
      // 3. The next node is NOT an RParen for a call —
      //    that would mean the current value is a call
      //    arg, not the deferred binop's RHS.
      // Clear pending call marker when RParen is reached.
      if self.pending_call_rparen == Some(idx) {
        self.pending_call_rparen = None;
      }

      if self.tuple_ctx.is_empty() && self.pending_call_rparen.is_none() {
        self.apply_deferred_binop();
      }
    }

    // Safety net: flush any remaining deferred closures.
    if !self.deferred_closures.is_empty() {
      let closures = std::mem::take(&mut self.deferred_closures);

      self.sir.instructions.extend(closures);
    }

    // Re-execute the Tree subtree of each queued
    // instantiation — generic-type, closure-param, or both.
    // Produces fresh SIR directly under the mangled name,
    // no cloning, no post-hoc rewrites. Carbon-aligned:
    // semantic analysis IS execution (manifesto §execution-
    // based compilation), so specialization is "run the
    // body again with a different environment", not "copy
    // SIR and substitute".
    self.reexecute_generic_instantiations();

    // Surface every array type reached by this SIR stream as
    // an `ArrayTyDef` so codegen can populate `array_metas`
    // and print arrays elementwise in `showln`. Codegen does
    // its own pre-scan for these, so they can live at the
    // tail of the instruction list without re-shuffling.
    self.emit_array_ty_defs();

    (self.sir, self.annotations, self.funs, self.abstract_defs)
  }

  /// Walk every `ty_id` in the emitted SIR, find each unique
  /// array type (`Ty::Array(..)`), and append one
  /// `Insn::ArrayTyDef { array_ty, elem_ty }` per unique
  /// array type. Idempotent via a HashSet dedup on the
  /// array's `TyId.0`.
  fn emit_array_ty_defs(&mut self) {
    let mut seen: std::collections::HashSet<u32> =
      std::collections::HashSet::new();
    let mut to_emit: Vec<(TyId, TyId)> = Vec::new();

    for insn in &self.sir.instructions {
      let mut ty_ids: Vec<TyId> = Vec::new();

      match insn {
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
        | Insn::FieldStore { ty_id, .. } => ty_ids.push(*ty_id),
        Insn::Cast { to_ty, .. } => ty_ids.push(*to_ty),
        Insn::FunDef {
          return_ty, params, ..
        } => {
          ty_ids.push(*return_ty);

          for (_, ty) in params {
            ty_ids.push(*ty);
          }
        }
        _ => {}
      }

      for ty_id in ty_ids {
        if seen.contains(&ty_id.0) {
          continue;
        }

        // Follow inference chain — ArrayLiteral etc. may
        // carry an Infer TyId that resolves to Array only
        // after unification.
        let canonical = self.ty_checker.resolve_id(ty_id);

        if let Ty::Array(aid) = self.ty_checker.resolve_ty(canonical)
          && let Some(arr_ty) = self.ty_checker.ty_table.array(aid)
        {
          // Record under the ORIGINAL `ty_id` — codegen's
          // `value_types` will carry that same TyId as its
          // key, so matching must happen there.
          seen.insert(ty_id.0);
          to_emit.push((ty_id, arr_ty.elem_ty));
        }
      }
    }

    for (array_ty, elem_ty) in to_emit {
      self.sir.emit(Insn::ArrayTyDef { array_ty, elem_ty });
    }
  }

  /// Returns true if the token introduces a statement —
  /// a construct only valid inside a function body per
  /// the grammar (`fun_body = "{", { stmt }, "}"`)
  fn is_stmt_introducer(token: Token) -> bool {
    matches!(
      token,
      Token::Imu
        | Token::Mut
        | Token::If
        | Token::While
        | Token::For
        | Token::Loop
        | Token::Return
        | Token::Break
        | Token::Continue
    )
  }

  /// Executes a single node from the parse tree.
  /// This is the core of the execution-based compilation model
  fn execute_node(&mut self, header: &NodeHeader, idx: usize) {
    // Enforce grammar: `program = { item }`.
    // Statement introducers are only valid inside function
    // bodies. Reject them at top level.
    if self.current_function.is_none()
      && self.apply_context.is_none()
      && self.pending_function.is_none()
      && self.pack_context.is_empty()
      && Self::is_stmt_introducer(header.token)
    {
      let span = self.tree.spans[idx];

      report_error(Error::new(ErrorKind::InvalidTopLevelItem, span));

      return;
    }

    match header.token {
      Token::Fun => {
        let children_end = (header.child_start + header.child_count) as usize;

        self.execute_fun(idx, children_end);
      }

      Token::Fn => {
        let children_end = (header.child_start + header.child_count) as usize;

        self.execute_closure(idx, children_end);
      }

      Token::Ffi => {
        let children_end = (header.child_start + header.child_count) as usize;

        self.execute_ffi(idx, children_end);
      }

      Token::Enum => {
        let children_end = (header.child_start + header.child_count) as usize;

        self.execute_enum(idx, children_end);
      }

      Token::Struct => {
        let children_end = (header.child_start + header.child_count) as usize;

        self.execute_struct(idx, children_end);
      }

      Token::Apply => {
        let children_end = (header.child_start + header.child_count) as usize;

        self.execute_apply(idx, children_end);
      }

      Token::Abstract => {
        let children_end = (header.child_start + header.child_count) as usize;

        self.execute_abstract(idx, children_end);
      }

      // === TYPE ALIAS ===
      Token::Type => {
        let children_end = (header.child_start + header.child_count) as usize;

        self.execute_type_alias(idx, children_end);

        self.skip_until = children_end;
      }

      Token::Group => {
        let children_end = (header.child_start + header.child_count) as usize;

        self.execute_group_type(idx, children_end);

        self.skip_until = children_end;
      }

      // === MODULE STATEMENTS ===
      Token::Load => {
        let children_end = (header.child_start + header.child_count) as usize;

        self.execute_load(idx, children_end);

        self.skip_until = children_end;
      }

      Token::Pack => {
        let children_end = (header.child_start + header.child_count) as usize;

        // `execute_pack` owns its body traversal (mirrors
        // `execute_apply`): emits `PackDecl`, pushes the
        // pack name onto `pack_context`, iterates the body
        // through `execute_node` so nested `fun`s get name-
        // mangled as `pack::fun` (and nested packs as
        // `outer::inner::fun`), then pops and sets
        // `skip_until = end_idx` so the main loop does not
        // re-walk the pack's own `Ident`, `LBrace`, body,
        // or `RBrace` — any of which would otherwise be
        // misread (e.g. bare pack name → Undefined
        // variable).
        self.execute_pack(idx, children_end);
      }

      // === DECLARATIONS ===
      // Deferred: children are processed first by the main
      // loop, then finalized at the Semicolon.
      Token::Imu => {
        self.begin_decl(idx, header, false, false);
      }

      Token::Val => {
        self.begin_decl(idx, header, false, true);
      }

      Token::Mut => {
        self.begin_decl(idx, header, true, false);
      }

      // === CONTROL FLOW ===
      Token::If => {
        let children_end = (header.child_start + header.child_count) as usize;

        self.execute_if(idx, children_end);
      }

      // === TERNARY EXPRESSION: when cond ? true : false ===
      Token::When => {
        let end_label = self.sir.next_label();
        let else_label = self.sir.next_label();

        // Branch-expr φ sink: allocated when the ternary
        // sits in expression position (e.g. `imu y =
        // when ... ? a : b`). each arm stores its result
        // into the sink; the merge-point Load reads it
        // back as the expression's value.
        let (value_sink, _existing_outer) =
          self.mint_branch_sink_if_expr_position();

        self.branch_stack.push(BranchCtx {
          kind: BranchKind::Ternary,
          end_label,
          else_label: Some(else_label),
          // Store stack depth at When for deferred
          // branch detection.
          loop_label: Some(self.sir_values.len() as u32),
          branch_emitted: false,
          for_var: None,
          scope_depth: self.scope_stack.len(),
          value_sink,
          value_sink_ty: None,
          stack_depth_at_entry: self.sir_values.len() as u32,
        });
      }

      Token::Question => {
        // Ternary: condition is on the stack — emit branch.
        if let Some(ctx) = self.branch_stack.last_mut()
          && ctx.kind == BranchKind::Ternary
          && !ctx.branch_emitted
        {
          if let Some(cond_sir) = self.sir_values.last().copied() {
            let target = ctx.else_label.unwrap();

            self.sir.emit(Insn::BranchIfNot {
              cond: cond_sir,
              target,
            });
          }

          self.value_stack.pop();
          self.ty_stack.pop();
          self.sir_values.pop();

          ctx.branch_emitted = true;
        } else {
          // Error propagation: expr? desugars to
          // match expr { Ok(v) => v, Err(e) => return Err(e) }
          self.execute_try_operator(idx);
        }
      }

      Token::While => {
        let children_end = (header.child_start + header.child_count) as usize;

        self.execute_while(idx, children_end);
      }

      Token::For => {
        let children_end = (header.child_start + header.child_count) as usize;

        self.execute_for(idx, children_end);
      }

      Token::Match => {
        let children_end = (header.child_start + header.child_count) as usize;

        self.execute_match(idx, children_end);
      }

      // === CONTROL FLOW ELSE ===
      Token::Else => {
        let is_if = self
          .branch_stack
          .last()
          .is_some_and(|c| c.kind == BranchKind::If);

        if is_if {
          // φ sink (if-as-expression): capture the then-arm
          // value into the branch's sink local BEFORE the
          // `Jump`. Mirrors the Ternary path at
          // `Token::Colon` — both arms must `Store` to the
          // sink before reconverging so the merge `Load`
          // after `end_label` reads a defined slot.
          let ctx_idx = self.branch_stack.len() - 1;

          self.emit_branch_sink_store(ctx_idx);

          // Re-borrow `ctx` for the Jump + else-label emit.
          if let Some(ctx) = self.branch_stack.last_mut() {
            self.sir.emit(Insn::Jump {
              target: ctx.end_label,
            });

            if let Some(else_label) = ctx.else_label.take() {
              self.sir.emit(Insn::Label { id: else_label });
            }
          }
        }
      }

      // === STYLE BLOCKS ===
      Token::Dollar
        if header.child_count > 0
          && (header.child_start as usize) < self.tree.nodes.len()
          && self.tree.nodes[header.child_start as usize].token
            == Token::Colon =>
      {
        let children_end =
          (header.child_start as usize) + (header.child_count as usize);

        self.execute_style_block(idx, children_end);

        self.skip_until = children_end;
      }

      // === DIRECTIVES ===
      Token::Hash => {
        let children_end = (header.child_start + header.child_count) as usize;

        self.execute_directive(idx, children_end);

        self.skip_until = children_end;
      }

      // === TUPLES / GROUPING / TUPLE TYPE ===
      Token::LParen => {
        // Function call: Ident before LParen (direct or
        // with an operator in between). Uses semantic
        // validation to distinguish `f(x)` from `a*(b)`.
        // Also catches pack-dotted calls (`pack.fn()`,
        // `outer.inner.fn()`) which have `Dot` before the
        // LParen rather than an Ident.
        let is_call = self.resolve_call_target(idx).is_some()
          || self.resolve_pack_dotted_call(idx).is_some();

        if is_call {
          // Track direct-call depth — consumed by
          // `apply_deferred_short_circuit` to know whether
          // a pending SC's RHS is `f(...)` (finalize *after*
          // the call returns) vs a value inside `f(...)`
          // (finalize when the call's args stabilize).
          self.direct_call_depth += 1;

          // Skip — RParen handles call.
          // For operator-separated calls (Ident Op LParen),
          // find the matching RParen and suppress deferred
          // binops until the call args are fully evaluated.
          if idx > 1
            && !matches!(self.tree.nodes[idx - 1].token, Token::Ident)
            && !self.deferred_binops.is_empty()
          {
            // Find matching RParen by depth counting.
            let mut depth = 1;
            let mut rp = idx + 1;

            while rp < self.tree.nodes.len() && depth > 0 {
              match self.tree.nodes[rp].token {
                Token::LParen => depth += 1,
                Token::RParen => depth -= 1,
                _ => {}
              }

              if depth > 0 {
                rp += 1;
              }
            }

            self.pending_call_rparen = Some(rp);
          }
        } else if idx + 1 < self.tree.nodes.len()
          && self.tree.nodes[idx + 1].token.is_ty()
        {
          // Tuple type annotation: (int, float, str).
          let (ty_id, skip_to) = self.resolve_tuple_type(idx);
          let value_id = self.values.store_type(ty_id);

          self.value_stack.push(value_id);
          self.ty_stack.push(self.ty_checker.type_type());
          self.skip_until = skip_to;
        } else if idx > 0 && self.tree.nodes[idx - 1].token == Token::Dot {
          // Method call: receiver.method() — don't
          // enter tuple context. execute_potential_call
          // will handle it at RParen.
        } else {
          // Tuple literal or grouping.
          let depth = self.sir_values.len();

          self.tuple_ctx.push(depth);
        }
      }

      // === FUNCTION CALLS / TUPLE CLOSE ===
      Token::RParen => {
        // If this RParen closes something LParen counted as
        // a direct call, decrement here so the counter stays
        // balanced across *every* RParen exit path (enum
        // constructor, direct call, method, etc.).
        let closed_a_direct_call = self.rparen_closes_call(idx);

        // Check if this closes an enum variant constructor.
        if let Some((enum_name, disc, field_count, ty_id)) =
          self.pending_enum_construct.take()
        {
          if closed_a_direct_call {
            self.direct_call_depth = self.direct_call_depth.saturating_sub(1);
          }
          let mut fields = Vec::with_capacity(field_count as usize);

          for _ in 0..field_count {
            if let Some(sv) = self.sir_values.pop() {
              fields.push(sv);
            }
            self.value_stack.pop();
            self.ty_stack.pop();
          }

          fields.reverse();

          let dst = ValueId(self.sir.next_value_id);

          self.sir.next_value_id += 1;

          let sv = self.sir.emit(Insn::EnumConstruct {
            dst,
            enum_name,
            variant: disc,
            fields,
            ty_id,
          });

          let rid = self.values.store_runtime(0);

          self.value_stack.push(rid);
          self.ty_stack.push(ty_id);
          self.sir_values.push(sv);
        }
        // Closes a call? `(a, b)` as the outer tuple pushed
        // tuple_ctx; a call like `f(x)` inside it did NOT.
        // Without this check, the call's RParen would pop
        // the outer tuple's depth and drop its result.
        else if closed_a_direct_call {
          // Ordering matters for short-circuit finalization:
          //   1. Finalize SCs deferred *inside* this call
          //      (pre_direct_call_depth == current) — their
          //      RHS is the final arg sitting on the stack.
          //      Runs before decrement so the guard passes.
          //   2. Decrement direct_call_depth.
          //   3. Emit the call.
          //   4. Finalize SCs deferred *outside* this call
          //      (pre_direct_call_depth < post-decrement) —
          //      their RHS is the call's return value now on
          //      the stack.
          self.apply_deferred_short_circuit();
          self.direct_call_depth = self.direct_call_depth.saturating_sub(1);
          self.execute_potential_call(idx);
          self.apply_deferred_short_circuit();
        }
        // Check if this closes a tuple/grouping context.
        else if let Some(depth) = self.tuple_ctx.pop() {
          let count = self.sir_values.len().saturating_sub(depth);

          if count > 1 {
            // Tuple literal: collect elements.
            let mut elements = Vec::with_capacity(count);
            let mut elem_tys = Vec::with_capacity(count);

            for _ in 0..count {
              if let Some(sv) = self.sir_values.pop() {
                elements.push(sv);
              }

              self.value_stack.pop();

              if let Some(ty) = self.ty_stack.pop() {
                elem_tys.push(ty);
              }
            }

            elements.reverse();
            elem_tys.reverse();

            // Build tuple type.
            let tuple_ty_id = self.ty_checker.ty_table.intern_tuple(elem_tys);
            let ty_id = self.ty_checker.intern_ty(Ty::Tuple(tuple_ty_id));
            let dst = ValueId(self.sir.next_value_id);

            self.sir.next_value_id += 1;

            let sv = self.sir.emit(Insn::TupleLiteral {
              dst,
              elements,
              ty_id,
            });
            let rid = self.values.store_runtime(0);

            self.value_stack.push(rid);
            self.ty_stack.push(ty_id);
            self.sir_values.push(sv);
          }
          // count <= 1: grouping — leave value on stack as-is.
          // Drain SCs whose tuple/grouping RHS just closed.
          self.apply_deferred_short_circuit();
          self.apply_deferred_binop();
        } else {
          // No tuple context → function call (via method or
          // other non-direct path). `direct_call_depth` is
          // not decremented here because LParen's `is_call`
          // branch owns that counter — if we reach this arm
          // the LParen didn't increment.
          self.apply_deferred_short_circuit();
          self.execute_potential_call(idx);
          self.apply_deferred_short_circuit();
        }
      }

      // === SCOPE BOUNDARIES ===
      Token::LBrace => {
        // Check for struct construction: Ident { field: val }
        if self.try_struct_construct(idx) {
          return;
        }

        // Check if we're entering a function body
        // This happens when we have a pending function definition
        if let Some(mut pending_func) = self.pending_function.take() {
          // Nested fun (item at statement level per the
          // grammar): save the outer context + SIR stream so
          // the inner body emits into its own buffer. Outer
          // state is restored at the matching `}`, and the
          // inner FunDef + body are spliced before the outer
          // FunDef via `deferred_closures` (reusing the same
          // lane the closure path already flushes through).
          if self.current_function.is_some() {
            let outer_function = self.current_function.take();
            let outer_value_stack = std::mem::take(&mut self.value_stack);
            let outer_ty_stack = std::mem::take(&mut self.ty_stack);
            let outer_sir_values = std::mem::take(&mut self.sir_values);
            let outer_pending_decl = self.pending_decl.take();

            let mut nested_sir = Sir::new();

            nested_sir.next_value_id = self.sir.next_value_id;
            nested_sir.next_label_id = self.sir.next_label_id;

            let outer_sir = std::mem::replace(&mut self.sir, nested_sir);

            self.saved_outer_funs.push(SavedOuterFun {
              function: outer_function,
              value_stack: outer_value_stack,
              ty_stack: outer_ty_stack,
              sir_values: outer_sir_values,
              sir: outer_sir,
              pending_decl: outer_pending_decl,
            });
          }

          // Emit the FunDef instruction first
          // Body will start at the NEXT instruction after FunDef
          let body_start = (self.sir.instructions.len() + 1) as u32;

          let fundef_idx = self.sir.instructions.len();

          self.sir.emit(Insn::FunDef {
            name: pending_func.name,
            params: pending_func.params.clone(),
            return_ty: pending_func.return_ty,
            body_start,
            kind: FunctionKind::UserDefined,
            pubness: pending_func.pubness,
          });

          // Now set the context with the correct body start.
          // scope_depth tracks where we are so only the
          // function body's RBrace triggers function-close.
          self.current_function = Some(FunCtx {
            name: pending_func.name,
            return_ty: pending_func.return_ty,
            body_start,
            fundef_idx,
            has_explicit_return: false,
            has_return_type_annotation: self.pending_fn_has_return_annotation,
            pending_return: false,
            scope_depth: self.scope_stack.len(),
          });

          // Update body_start in the pending function
          pending_func.body_start = body_start;

          // Store function definition for later calls. Upsert:
          // the pre-scan pass may have already registered this
          // function's signature so forward references resolve.
          // The main pass overwrites the stub with the real
          // body_start here.
          self.push_or_replace_fun(pending_func);

          // Clear stacks when entering function body to avoid leftover values
          self.value_stack.clear();
          self.ty_stack.clear();
          self.sir_values.clear();
        }

        // Reject bare blocks at top level: `block_stmt`
        // is only valid inside function bodies.
        if self.current_function.is_none()
          && self.apply_context.is_none()
          && self.pending_function.is_none()
          && self.branch_stack.is_empty()
          && self.pack_context.is_empty()
        {
          let span = self.tree.spans[idx];

          report_error(Error::new(ErrorKind::InvalidTopLevelItem, span));

          return;
        }

        // Emit branch instruction for control flow.
        if let Some(ctx) = self.branch_stack.last_mut()
          && !ctx.branch_emitted
        {
          if let Some(cond_sir) = self.sir_values.last().copied() {
            let target = match ctx.kind {
              BranchKind::If | BranchKind::Ternary => {
                ctx.else_label.unwrap_or(ctx.end_label)
              }
              BranchKind::While | BranchKind::For => ctx.end_label,
            };

            self.sir.emit(Insn::BranchIfNot {
              cond: cond_sir,
              target,
            });

            // Pop the condition from stacks — it was consumed
            // by BranchIfNot and must not leak into the block
            // body (e.g. `if x <= 1 { return 1; } return x *
            // fact(x-1);` — the stale condition boolean would
            // become an operand for `*`).
            self.value_stack.pop();
            self.ty_stack.pop();
            self.sir_values.pop();
          }

          ctx.branch_emitted = true;
        }

        self.push_scope();
      }
      Token::RBrace => {
        // Finalize pending assignments/compounds before
        // closing the block. Assignments evaluate to unit
        // regardless of whether a semicolon follows.
        self.finalize_pending_compound();
        self.finalize_pending_assign();
        // Check for pending return (explicit return without semicolon)
        self.check_pending_return();

        // Check if we're closing the function body (not an
        // inner block like if/else/while).
        // The function body scope is about to be popped.
        // It was pushed AFTER scope_depth was captured, so
        // current depth is scope_depth + 1 at the function
        // body's RBrace, and deeper for inner blocks.
        let at_fn_depth = self
          .current_function
          .as_ref()
          .is_some_and(|c| self.scope_stack.len() == c.scope_depth + 1);

        // Flush deferred binops before implicit return —
        // the function body may end with `x * fact(x - 1)`
        // inside a ternary without a semicolon.
        if at_fn_depth {
          self.apply_deferred_binop();
        }

        if at_fn_depth && let Some(fun_ctx) = &self.current_function {
          // Ternary false arm: the true arm set
          // has_explicit_return, but the false arm still
          // needs its own Return. Emit it here before the
          // function closes.
          let is_ternary_false_arm = fun_ctx.has_explicit_return
            && self
              .branch_stack
              .last()
              .is_some_and(|c| c.kind == BranchKind::Ternary);

          if is_ternary_false_arm {
            let unit_ty = self.ty_checker.unit_type();

            if fun_ctx.return_ty != unit_ty && !self.sir_values.is_empty() {
              let sir_val = self.sir_values.last().copied();
              let ty = self.ty_stack.last().copied().unwrap_or(unit_ty);

              self.sir.emit(Insn::Return {
                value: sir_val,
                ty_id: ty,
              });
            }

            // Pop the ternary and emit its end label.
            if let Some(ctx) = self.branch_stack.pop() {
              self.sir.emit(Insn::Label { id: ctx.end_label });
            }
          }
        }

        // Ternary with φ sink that's the tail expression
        // of the function body: close it now so the merge
        // Load pushes onto the stacks and the
        // implicit-return path below picks it up as the
        // function's return value. mirrors the Semicolon-
        // closing path, just triggered by the closing `}`
        // instead of `;`.
        if at_fn_depth {
          while self.branch_stack.last().is_some_and(|c| {
            c.kind == BranchKind::Ternary && c.value_sink.is_some()
          }) {
            let ctx_idx = self.branch_stack.len() - 1;

            self.emit_branch_sink_store(ctx_idx);

            let ctx = self.branch_stack.pop().unwrap();

            self.sir.emit(Insn::Label { id: ctx.end_label });
            self.emit_branch_sink_load(&ctx);
          }
        }

        if at_fn_depth && let Some(fun_ctx) = &self.current_function {
          // Only emit implicit return if there wasn't an explicit one
          if !fun_ctx.has_explicit_return {
            // Emit implicit return if needed
            // Check if function returns unit type
            let unit_ty = self.ty_checker.unit_type();
            let func_return_ty = fun_ctx.return_ty;

            let has_value =
              !self.value_stack.is_empty() && !self.ty_stack.is_empty();
            let body_ty = self.ty_stack.last().copied().unwrap_or(unit_ty);

            // Use the function definition span for return
            // type errors (not the closing `}`).
            let fn_span = self.tree.spans[fun_ctx.fundef_idx];

            let (return_value, return_ty) = if func_return_ty == unit_ty {
              // Unit functions return void. Only report
              // mismatch when the body has a non-unit value
              // AND the function was explicitly declared with
              // a return type (forced to unit by error recovery
              // in main). Stale values from control flow
              // (while/if) are NOT return values.
              if has_value
                && body_ty != unit_ty
                && fun_ctx.has_return_type_annotation
              {
                report_error(Error::new(ErrorKind::TypeMismatch, fn_span));
              }

              (None, unit_ty)
            } else if has_value {
              // Non-void function with value on stack.
              // Filter sentinels from non-value-producing
              // instructions (Label, Jump, BranchIfNot).
              let sir_value =
                self.sir_values.last().copied().filter(|v| v.0 != u32::MAX);

              (sir_value, body_ty)
            } else {
              report_error(Error::new(ErrorKind::TypeMismatch, fn_span));

              (None, unit_ty)
            };

            // Emit implicit return
            self.sir.emit(Insn::Return {
              value: return_value,
              ty_id: return_ty,
            });
          }

          // Detect intrinsic: empty body (no instructions
          // between body_start and the return we just emitted).
          let current_insn_count = self.sir.instructions.len() as u32;

          if current_insn_count == fun_ctx.body_start + 1 {
            // Only instruction is the implicit return — body
            // was empty. Mark the FunDef as intrinsic.
            if let Some(Insn::FunDef { kind, .. }) =
              self.sir.instructions.get_mut(fun_ctx.fundef_idx)
            {
              *kind = FunctionKind::Intrinsic;
            }
          }

          // Flush deferred closure instructions BEFORE the
          // enclosing function's FunDef. This places closure
          // FunDefs as siblings preceding the containing
          // function, so the codegen and register allocator
          // process them first (no forward references).
          if !self.deferred_closures.is_empty() {
            let mut closures = std::mem::take(&mut self.deferred_closures);

            // Fix body_start offsets: they were relative to
            // the temporary SIR (FunDef at 0, body at 1).
            // After insertion, rebase to the main SIR.
            let insert_pos = fun_ctx.fundef_idx;

            for insn in closures.iter_mut() {
              if let Insn::FunDef { body_start, .. } = insn {
                *body_start += insert_pos as u32;
              }
            }

            let closure_len = closures.len();

            // Splice closures before the enclosing FunDef.
            self
              .sir
              .instructions
              .splice(insert_pos..insert_pos, closures);

            // Adjust the enclosing function's fundef_idx
            // and body_start since we shifted instructions.
            if let Some(Insn::FunDef { body_start, .. }) =
              self.sir.instructions.get_mut(insert_pos + closure_len)
            {
              *body_start += closure_len as u32;
            }
          }

          // Clear function context
          self.current_function = None;

          // Pop body scope + param scope. The param
          // scope was pushed in execute_fun; the body
          // scope was pushed at LBrace. Both must be
          // cleaned up so parameter locals don't leak.
          self.pop_scope(); // body scope
          self.pop_scope(); // param scope

          // Nested fun restore: if we saved an outer
          // function context on the way in, move this
          // nested fun's SIR to `deferred_closures` (so
          // it splices out of the outer body at the outer
          // fun's close) and restore the outer state.
          if let Some(saved) = self.saved_outer_funs.pop() {
            let nested_sir = std::mem::replace(&mut self.sir, saved.sir);

            self.sir.next_value_id = nested_sir.next_value_id;
            self.sir.next_label_id = nested_sir.next_label_id;
            self.deferred_closures.extend(nested_sir.instructions);

            self.current_function = saved.function;
            self.value_stack = saved.value_stack;
            self.ty_stack = saved.ty_stack;
            self.sir_values = saved.sir_values;
            self.pending_decl = saved.pending_decl;
          }
        }

        // Close control flow block — but only if this
        // RBrace belongs to the scope that opened the
        // branch. Inner blocks (e.g. `_ => {}` in match
        // arms) must not consume the outer while/if.
        let at_branch_depth = self
          .branch_stack
          .last()
          .is_some_and(|c| self.scope_stack.len() == c.scope_depth + 1);

        // Flush deferred binops before closing a Ternary.
        // `when x <= 1 ? 1 : x * fact(x-1)` defers `*`
        // and the false arm Return needs the Mul result.
        if at_branch_depth
          && self
            .branch_stack
            .last()
            .is_some_and(|c| c.kind == BranchKind::Ternary)
        {
          self.apply_deferred_binop();
        }

        if at_branch_depth && let Some(ctx) = self.branch_stack.last() {
          match ctx.kind {
            BranchKind::While => {
              if let Some(loop_label) = ctx.loop_label {
                self.sir.emit(Insn::Jump { target: loop_label });
              }

              self.sir.emit(Insn::Label { id: ctx.end_label });
              self.branch_stack.pop();
            }
            BranchKind::For => {
              // Emit: i = i + 1; jump loop_start; label end
              let int_ty = self.ty_checker.int_type();

              if let Some(var_name) = ctx.for_var {
                let ld = ValueId(self.sir.next_value_id);

                self.sir.next_value_id += 1;

                let ld_sir = self.sir.emit(Insn::Load {
                  dst: ld,
                  src: LoadSource::Local(var_name),
                  ty_id: int_ty,
                });

                let one_dst = ValueId(self.sir.next_value_id);
                self.sir.next_value_id += 1;

                let one_sir = self.sir.emit(Insn::ConstInt {
                  dst: one_dst,
                  value: 1,
                  ty_id: int_ty,
                });

                let add_dst = ValueId(self.sir.next_value_id);

                self.sir.next_value_id += 1;

                let add_sir = self.sir.emit(Insn::BinOp {
                  dst: add_dst,
                  op: zo_sir::BinOp::Add,
                  lhs: ld_sir,
                  rhs: one_sir,
                  ty_id: int_ty,
                });

                self.sir.emit(Insn::Store {
                  name: var_name,
                  value: add_sir,
                  ty_id: int_ty,
                });
              }

              if let Some(loop_label) = ctx.loop_label {
                self.sir.emit(Insn::Jump { target: loop_label });
              }

              self.sir.emit(Insn::Label { id: ctx.end_label });
              self.branch_stack.pop();
            }
            BranchKind::If => {
              // Check if the next tree token is Else.
              let next_is_else = self
                .tree
                .nodes
                .get(idx + 1)
                .is_some_and(|n| n.token == Token::Else);

              if next_is_else {
                // Else follows — don't close yet.
                // Token::Else will emit Jump + Label.
              } else {
                // No else: this RBrace closes either the
                // then-arm of an if-without-else OR the
                // else-arm of an if-else (after `Token::Else`
                // already emitted the else-label + the
                // then-arm's sink store).
                //
                // If `else_label` is still set, we're on the
                // no-else path and need to emit it before
                // the merge label. Either way, when the if
                // is an expression (`ctx.value_sink.is_some()`),
                // store the current arm's top-of-stack into
                // the sink BEFORE `end_label`, then `Load`
                // it back AFTER so the merge pushes the
                // expression's value onto the stacks.
                let had_else_label = ctx.else_label.is_some();

                // Capture the arm's value into the sink.
                let ctx_idx = self.branch_stack.len() - 1;

                self.emit_branch_sink_store(ctx_idx);

                // Re-borrow `ctx` after the emit.
                let ctx = self.branch_stack.last_mut().unwrap();

                if had_else_label && let Some(el) = ctx.else_label {
                  self.sir.emit(Insn::Label { id: el });
                }

                self.sir.emit(Insn::Label { id: ctx.end_label });

                let popped = self.branch_stack.pop().unwrap();

                // Merge: reload the sink so the if-as-
                // expression's value lands on the stacks.
                // No-op for statement-position ifs (no sink).
                self.emit_branch_sink_load(&popped);
              }
            }
            BranchKind::Ternary => {
              // Emit Return for the false arm.
              if let Some(ref mut fun_ctx) = self.current_function {
                let unit_ty = self.ty_checker.unit_type();

                let needs_return =
                  fun_ctx.pending_return || fun_ctx.return_ty != unit_ty;

                if needs_return {
                  let sir_val = self.sir_values.last().copied();
                  let ty = self.ty_stack.last().copied().unwrap_or(unit_ty);

                  self.sir.emit(Insn::Return {
                    value: sir_val,
                    ty_id: ty,
                  });

                  fun_ctx.pending_return = false;
                  fun_ctx.has_explicit_return = true;
                }
              }

              self.sir.emit(Insn::Label { id: ctx.end_label });
              self.branch_stack.pop();
            }
          }
        }

        if !at_fn_depth {
          self.pop_scope();
        }
      }

      // === LITERALS (push compile-time constants) ===
      Token::Int => {
        // Get the integer value from the node
        if let Some(NodeValue::Literal(lit_idx)) = self.node_value(idx) {
          // Get actual value from literal store (already u64, no cast needed)
          let value = self.literals.int_literals[lit_idx as usize];

          // Infer type based on value
          let ty_id = self.ty_checker.int_type();

          let dst = ValueId(self.sir.next_value_id);
          self.sir.next_value_id += 1;

          let sir_value = self.sir.emit(Insn::ConstInt { dst, value, ty_id });
          let value_id = self.values.store_int(value);

          self.value_stack.push(value_id);
          self.ty_stack.push(ty_id);
          self.sir_values.push(sir_value);

          // Create annotation for this integer literal
          self.annotations.push(Annotation {
            node_idx: idx,
            ty_id,
          });
        }
      }

      Token::Float => {
        if let Some(NodeValue::Literal(lit_idx)) = self.node_value(idx) {
          let value = self.literals.float_literals[lit_idx as usize];
          let ty_id = self.ty_checker.f64_type();

          let dst = ValueId(self.sir.next_value_id);
          self.sir.next_value_id += 1;

          let sir_value = self.sir.emit(Insn::ConstFloat { dst, value, ty_id });
          let value_id = self.values.store_float(value);

          self.value_stack.push(value_id);
          self.ty_stack.push(ty_id);
          self.sir_values.push(sir_value);

          self.annotations.push(Annotation {
            node_idx: idx,
            ty_id,
          });
        }
      }

      Token::True => {
        let ty_id = self.ty_checker.bool_type();
        let dst = ValueId(self.sir.next_value_id);
        self.sir.next_value_id += 1;

        let sir_value = self.sir.emit(Insn::ConstBool {
          dst,
          value: true,
          ty_id,
        });
        let value_id = self.values.store_bool(true);

        self.value_stack.push(value_id);
        self.ty_stack.push(ty_id);
        self.sir_values.push(sir_value);

        self.annotations.push(Annotation {
          node_idx: idx,
          ty_id,
        });
      }

      Token::False => {
        let ty_id = self.ty_checker.bool_type();

        // Emit SIR instruction for boolean constant
        let dst = ValueId(self.sir.next_value_id);
        self.sir.next_value_id += 1;

        let sir_value = self.sir.emit(Insn::ConstBool {
          dst,
          value: false,
          ty_id,
        });

        // Store in value storage and push to stack
        let value_id = self.values.store_bool(false);

        self.value_stack.push(value_id);
        self.ty_stack.push(ty_id);
        self.sir_values.push(sir_value);

        self.annotations.push(Annotation {
          node_idx: idx,
          ty_id,
        });
      }

      Token::Char => {
        if let Some(NodeValue::Literal(lit_idx)) = self.node_value(idx) {
          let value = self.literals.char_literals[lit_idx as usize] as u64;
          let ty_id = self.ty_checker.char_type();

          let dst = ValueId(self.sir.next_value_id);
          self.sir.next_value_id += 1;

          let sir_value = self.sir.emit(Insn::ConstInt { dst, value, ty_id });
          let value_id = self.values.store_int(value);

          self.value_stack.push(value_id);
          self.ty_stack.push(ty_id);
          self.sir_values.push(sir_value);

          self.annotations.push(Annotation {
            node_idx: idx,
            ty_id,
          });
        }
      }

      Token::Bytes => {
        if let Some(NodeValue::Literal(lit_idx)) = self.node_value(idx) {
          let value = self.literals.bytes_literals[lit_idx as usize] as u64;
          let ty_id = self.ty_checker.bytes_type();

          let dst = ValueId(self.sir.next_value_id);
          self.sir.next_value_id += 1;

          let sir_value = self.sir.emit(Insn::ConstInt { dst, value, ty_id });
          let value_id = self.values.store_int(value);

          self.value_stack.push(value_id);
          self.ty_stack.push(ty_id);
          self.sir_values.push(sir_value);

          self.annotations.push(Annotation {
            node_idx: idx,
            ty_id,
          });
        }
      }

      Token::InterpString => {
        // InterpString stores packed value:
        // low 16 = string_literals idx,
        // high 16 = interp_ranges idx.
        if let Some(NodeValue::Literal(packed)) = self.node_value(idx) {
          let str_idx = (packed & 0xFFFF) as usize;
          let symbol = self.literals.string_literals[str_idx];
          let ty_id = self.ty_checker.str_type();

          // Emit ConstString for the full format string
          // (may become dead code after desugaring).
          let dst = ValueId(self.sir.next_value_id);
          self.sir.next_value_id += 1;

          let sir_value =
            self.sir.emit(Insn::ConstString { dst, symbol, ty_id });
          let value_id = self.values.store_string(symbol);

          self.value_stack.push(value_id);
          self.ty_stack.push(ty_id);
          self.sir_values.push(sir_value);

          self.annotations.push(Annotation {
            node_idx: idx,
            ty_id,
          });
        }
      }

      Token::String | Token::RawString => {
        // String literals are already interned during
        // tokenization.
        if let Some(NodeValue::Symbol(symbol)) = self.node_value(idx) {
          let ty_id = self.ty_checker.str_type();

          let dst = ValueId(self.sir.next_value_id);
          self.sir.next_value_id += 1;

          let sir_value =
            self.sir.emit(Insn::ConstString { dst, symbol, ty_id });
          let value_id = self.values.store_string(symbol);

          self.value_stack.push(value_id);
          self.ty_stack.push(ty_id);
          self.sir_values.push(sir_value);

          self.annotations.push(Annotation {
            node_idx: idx,
            ty_id,
          });
        }
      }

      // === SELF TYPE ===
      Token::SelfUpper => {
        // In apply context, Self acts as the type name.
        // Do nothing here — struct construction handles
        // it via try_struct_construct looking back at
        // the SelfUpper token and resolving the apply
        // context type name.
      }

      // === SELF VALUE ===
      // `self` in expression context — load the receiver
      // parameter. Added as a local with
      // LocalKind::Parameter during function parameter
      // parsing.
      Token::SelfLower => {
        let sym = Symbol::SELF_LOWER;

        let local_info = self
          .lookup_local(sym)
          .map(|l| (l.value_id, l.ty_id, l.local_kind));

        if let Some((_, ty_id, LocalKind::Parameter)) = local_info {
          let dst = ValueId(self.sir.next_value_id);

          self.sir.next_value_id += 1;

          // self is always param 0.
          let src = LoadSource::Param(0);
          let sv = self.sir.emit(Insn::Load { dst, src, ty_id });
          let rid = self.values.store_runtime(0);

          self.value_stack.push(rid);
          self.ty_stack.push(ty_id);
          self.sir_values.push(sv);
        }
      }

      // === IDENTIFIERS ===
      Token::Ident => {
        // Skip modifier idents (e.g., `lt` in `check@lt`).
        // They are handled by execute_check_modifier at
        // RParen time.
        if idx >= 1 && self.tree.nodes[idx - 1].token == Token::At {
          return;
        }

        if let Some(NodeValue::Symbol(sym)) = self.node_value(idx) {
          // Copy fields to avoid borrow issues.
          let local_info = self.lookup_local(sym).map(|l| {
            (l.value_id, l.ty_id, l.sir_value, l.local_kind, l.mutability)
          });

          if let Some((value_id, ty_id, sir_value, local_kind, mutability)) =
            local_info
          {
            // Closure call target: skip pushing if the
            // closure is about to be called. Check both:
            // - direct: Ident LParen (idx+1 is LParen)
            // - operator-separated: Ident Op LParen (idx+2
            //   is LParen, e.g. `5 + f(10)`)
            // If neither, the closure is being passed as an
            // argument — push it for the callee.
            if matches!(
              self.values.kinds.get(value_id.0 as usize),
              Some(Value::Closure)
            ) {
              let next_is_call = self
                .tree
                .nodes
                .get(idx + 1)
                .is_some_and(|n| n.token == Token::LParen);

              let next2_is_call = !next_is_call
                && self
                  .tree
                  .nodes
                  .get(idx + 2)
                  .is_some_and(|n| n.token == Token::LParen);

              if next_is_call || next2_is_call {
                return;
              }
            }

            // Compile-time constant: re-emit the literal
            // value as a fresh SIR instruction each time.
            // No Load, no stack slot.
            if local_kind == LocalKind::Constant {
              let vi = value_id.0 as usize;

              if vi < self.values.kinds.len() {
                let sv = match self.values.kinds[vi] {
                  Value::Int => {
                    let ii = self.values.indices[vi] as usize;
                    let v = self.values.ints[ii];

                    let dst = ValueId(self.sir.next_value_id);
                    self.sir.next_value_id += 1;

                    self.sir.emit(Insn::ConstInt {
                      dst,
                      value: v,
                      ty_id,
                    })
                  }
                  Value::Float => {
                    let fi = self.values.indices[vi] as usize;
                    let v = self.values.floats[fi];

                    let dst = ValueId(self.sir.next_value_id);
                    self.sir.next_value_id += 1;

                    self.sir.emit(Insn::ConstFloat {
                      dst,
                      value: v,
                      ty_id,
                    })
                  }
                  Value::Bool => {
                    let bi = self.values.indices[vi] as usize;
                    let v = self.values.bools[bi];

                    let dst = ValueId(self.sir.next_value_id);
                    self.sir.next_value_id += 1;

                    self.sir.emit(Insn::ConstBool {
                      dst,
                      value: v,
                      ty_id,
                    })
                  }
                  Value::String => {
                    let si = self.values.indices[vi] as usize;
                    let s = self.values.strings[si];

                    let dst = ValueId(self.sir.next_value_id);
                    self.sir.next_value_id += 1;

                    self.sir.emit(Insn::ConstString {
                      dst,
                      symbol: s,
                      ty_id,
                    })
                  }
                  _ => {
                    self.value_stack.push(value_id);
                    self.ty_stack.push(ty_id);

                    if let Some(s) = sir_value {
                      self.sir_values.push(s);
                    }

                    return;
                  }
                };

                self.value_stack.push(value_id);
                self.ty_stack.push(ty_id);
                self.sir_values.push(sv);
              }

              return;
            }

            if self.current_function.is_some() {
              let is_mut = mutability == Mutability::Yes;
              let is_param = local_kind == LocalKind::Parameter;

              if is_param || is_mut {
                // Parameter or mutable local: emit Load.
                // Params use src=param_index (0-7).
                // Mutables use src=100+slot so codegen
                // can distinguish and read from stack.
                let dst = ValueId(self.sir.next_value_id);

                self.sir.next_value_id += 1;

                let src = if is_param {
                  // Look up param index from the
                  // current function's param list.
                  // Match by name (not body_start) to
                  // avoid collisions with closures.
                  let idx = self
                    .current_function
                    .as_ref()
                    .and_then(|ctx| {
                      self.funs.iter().find(|f| f.name == ctx.name).and_then(
                        |f| f.params.iter().position(|(n, _)| *n == sym),
                      )
                    })
                    .unwrap_or(0) as u32;

                  LoadSource::Param(idx)
                } else {
                  LoadSource::Local(sym)
                };

                let sv = self.sir.emit(Insn::Load { dst, src, ty_id });

                let rid = self.values.store_runtime(0);

                self.value_stack.push(rid);
                self.ty_stack.push(ty_id);
                self.sir_values.push(sv);
              } else if sir_value.is_some() {
                // Immutable local: emit Load so
                // liveness analysis tracks it.
                let dst = ValueId(self.sir.next_value_id);
                self.sir.next_value_id += 1;

                let sv = self.sir.emit(Insn::Load {
                  dst,
                  src: LoadSource::Local(sym),
                  ty_id,
                });

                // For closure locals being passed as
                // arguments, preserve the original
                // ClosureValue so the callee can detect
                // and monomorphize the closure param.
                let vi = value_id.0 as usize;
                let is_closure = vi < self.values.kinds.len()
                  && matches!(self.values.kinds[vi], Value::Closure);

                let push_id = if is_closure {
                  value_id
                } else {
                  self.values.store_runtime(0)
                };

                self.value_stack.push(push_id);
                self.ty_stack.push(ty_id);
                self.sir_values.push(sv);
              }
            } else {
              self.value_stack.push(value_id);
              self.ty_stack.push(ty_id);
              self.sir_values.push(value_id);
            }
          } else {
            // Check global constants (module-level val).
            let global = self
              .global_constants
              .iter()
              .find(|c| c.name == sym)
              .map(|c| (c.value_id, c.ty_id));

            if let Some((gval, gty)) = global {
              // Inline re-emission: emit a fresh ConstInt/
              // ConstFloat/etc into the current function's
              // SIR with a proper ValueId.
              let vi = gval.0 as usize;

              if vi < self.values.kinds.len() {
                let sv = match self.values.kinds[vi] {
                  Value::Int => {
                    let ii = self.values.indices[vi] as usize;

                    let dst = ValueId(self.sir.next_value_id);
                    self.sir.next_value_id += 1;

                    self.sir.emit(Insn::ConstInt {
                      dst,
                      value: self.values.ints[ii],
                      ty_id: gty,
                    })
                  }
                  Value::Float => {
                    let fi = self.values.indices[vi] as usize;

                    let dst = ValueId(self.sir.next_value_id);
                    self.sir.next_value_id += 1;

                    self.sir.emit(Insn::ConstFloat {
                      dst,
                      value: self.values.floats[fi],
                      ty_id: gty,
                    })
                  }
                  Value::Bool => {
                    let bi = self.values.indices[vi] as usize;

                    let dst = ValueId(self.sir.next_value_id);
                    self.sir.next_value_id += 1;

                    self.sir.emit(Insn::ConstBool {
                      dst,
                      value: self.values.bools[bi],
                      ty_id: gty,
                    })
                  }
                  Value::String => {
                    let si = self.values.indices[vi] as usize;

                    let dst = ValueId(self.sir.next_value_id);
                    self.sir.next_value_id += 1;

                    self.sir.emit(Insn::ConstString {
                      dst,
                      symbol: self.values.strings[si],
                      ty_id: gty,
                    })
                  }
                  _ => ValueId(u32::MAX),
                };

                self.value_stack.push(gval);
                self.ty_stack.push(gty);
                self.sir_values.push(sv);

                return;
              }
            }

            // Check if this identifier is a known function
            // — call handling happens at RParen, not here.
            // Functions come from prelude imports or
            // explicit `load` — no hardcoded builtins.
            let is_fun = self.funs.iter().any(|f| f.name == sym);
            let sym_str = self.interner.get(sym);
            let is_enum = self.enum_defs.iter().any(|e| e.0 == sym)
              || self
                .pending_imported_enums
                .iter()
                .any(|e| self.interner.get(e.name) == sym_str);
            let is_struct =
              self.ty_checker.ty_table.struct_intern_lookup(sym).is_some();

            // Field/method name idents appear before Dot
            // in postfix order. Push a placeholder so the
            // Dot handler has two values to pop (receiver +
            // member). The actual field name is resolved
            // from the tree node, not the stack value.
            let is_dot_member = idx + 1 < self.tree.nodes.len()
              && self.tree.nodes[idx + 1].token == Token::Dot;

            let is_pack = self.pack_names.contains(&sym);

            if is_dot_member || is_pack {
              // Pack-prefix idents (`inner`, `inner2` in
              // `inner.inner2.hello()`) are namespace
              // segments — not variables. Push a
              // placeholder so the Dot / call sites see
              // balanced stacks; `execute_potential_call`
              // walks the tree to build the mangled name
              // (`inner::inner2::hello`).
              let placeholder = self.values.store_runtime(0);

              self.value_stack.push(placeholder);
              self.ty_stack.push(self.ty_checker.unit_type());
              self.sir_values.push(ValueId(u32::MAX));
            } else if is_fun && !self.ident_is_call_target(idx) {
              // Fun name used as a first-class value (e.g.
              // `ho(direct)` passing `direct` to a `Fn(...)`
              // parameter). Push a synthetic `Value::Closure`
              // with zero captures pointing at the fun —
              // downstream the closure-param mono path
              // (line ~10025) sees `Value::Closure` at the
              // arg slot, builds a `__cl<fun>` specialization
              // of the callee, and binds the param to this
              // fun's name so `f(x)` inside the body lowers
              // to `call <fun>(x)` directly.
              //
              // `ident_is_call_target` mirrors
              // `resolve_call_target`'s logic: direct (`f(`)
              // OR operator-separated (`a + f(` / `a && f(`)
              // both keep the ident as a callee and must NOT
              // push a closure value — that would land on the
              // operator's operand stack and break the binop.
              let fun_def = self.funs.iter().find(|f| f.name == sym);

              let fun_ty = if let Some(fd) = fun_def {
                let param_tys: Vec<TyId> =
                  fd.params.iter().map(|(_, t)| *t).collect();
                let fun_ty_id =
                  self.ty_checker.ty_table.intern_fun(param_tys, fd.return_ty);

                self.ty_checker.intern_ty(Ty::Fun(fun_ty_id))
              } else {
                self.ty_checker.unit_type()
              };

              let closure_val = self.values.store_closure(ClosureValue {
                fun_name: sym,
                captures: Vec::new(),
              });

              self.value_stack.push(closure_val);
              self.ty_stack.push(fun_ty);
              self.sir_values.push(ValueId(u32::MAX));
            } else if !is_fun && !is_enum && !is_struct && !is_pack {
              let span = self.tree.spans[idx];

              report_error(Error::new(ErrorKind::UndefinedVariable, span));

              let error_id = self.values.store_runtime(u32::MAX);

              self.value_stack.push(error_id);
              self.ty_stack.push(self.ty_checker.error_type());
            }
          }
        }
      }

      // === ARRAYS ===
      Token::LBracket => {
        // Determine context: indexing (preceded by an
        // array value on the stack) or literal.
        // For indexing: the array value was pushed by
        // the preceding Ident / RBracket (chained
        // indexing `m[0][0]`) / SelfLower (indexing
        // into `self` inside `apply []T { fun … (self)
        // … self[i] … }`). For literals: stacks have
        // whatever was there before.
        let is_indexing = idx > 0
          && matches!(
            self.tree.nodes[idx - 1].token,
            Token::Ident | Token::RBracket | Token::SelfLower
          );

        let array_name = if is_indexing && idx > 0 {
          self.node_value(idx - 1).and_then(|v| match v {
            NodeValue::Symbol(s) => Some(s),
            _ => None,
          })
        } else {
          None
        };

        let depth = self.sir_values.len();

        self.array_ctx.push((is_indexing, depth, array_name));
      }

      Token::RBracket => {
        if let Some((is_indexing, depth, _array_name)) = self.array_ctx.pop() {
          let int_ty = self.ty_checker.int_type();

          if is_indexing {
            // Slice (`s[lo..hi]` / `s[lo..=hi]`). Detect by
            // looking at the node immediately before the
            // closing bracket — the parser emits postorder,
            // so `DotDot` / `DotDotEq` sits at `idx - 1`.
            // Compile-time only in v1: both bounds and the
            // receiver must be compile-time constants. A
            // runtime slice would need a view layout or
            // heap allocation, neither of which exist yet.
            let is_slice = idx > 0
              && matches!(
                self.tree.nodes[idx - 1].token,
                Token::DotDot | Token::DotDotEq
              );

            if is_slice {
              self.execute_str_slice_const(idx, depth);

              return;
            }

            // Pop index and array from stacks.
            if let (Some(_idx_val), Some(idx_ty)) =
              (self.value_stack.pop(), self.ty_stack.pop())
            {
              let idx_sir = self.sir_values.pop().unwrap_or(ValueId(u32::MAX));

              // Pop array/string value.
              if let (Some(_arr_val), Some(arr_ty)) =
                (self.value_stack.pop(), self.ty_stack.pop())
              {
                let arr_sir =
                  self.sir_values.pop().unwrap_or(ValueId(u32::MAX));

                let span = self.tree.spans[idx];

                // Validate index type is integer.
                let idx_is_int =
                  matches!(self.ty_checker.resolve_ty(idx_ty), Ty::Int { .. });

                if !idx_is_int {
                  report_error(Error::new(ErrorKind::InvalidIndex, span));
                }

                let dst = ValueId(self.sir.next_value_id);

                self.sir.next_value_id += 1;

                // Resolve element type from the base type.
                let base_ty = self.ty_checker.resolve_ty(arr_ty);

                let elem_ty = match base_ty {
                  Ty::Array(aid) => match self.ty_checker.ty_table.array(aid) {
                    Some(at) => at.elem_ty,
                    None => int_ty,
                  },
                  Ty::Str => self.ty_checker.char_type(),
                  _ => {
                    report_error(Error::new(ErrorKind::InvalidIndex, span));

                    int_ty
                  }
                };

                let sv = self.sir.emit(Insn::ArrayIndex {
                  dst,
                  array: arr_sir,
                  index: idx_sir,
                  ty_id: elem_ty,
                });

                let rid = self.values.store_runtime(0);

                self.value_stack.push(rid);
                self.ty_stack.push(elem_ty);
                self.sir_values.push(sv);
              }
            }
          } else {
            // Array literal: collect elements from
            // stacks (everything since depth).
            let count = self.sir_values.len().saturating_sub(depth);
            let mut elements = Vec::with_capacity(count);

            // Infer element type from the first element.
            // Empty arrays get a fresh inference variable so
            // unification with the type annotation resolves it.
            let elem_ty = if depth < self.ty_stack.len() {
              self.ty_stack[depth]
            } else {
              self.ty_checker.fresh_var()
            };

            // Pop elements in reverse, then reverse.
            for _ in 0..count {
              if let Some(sv) = self.sir_values.pop() {
                elements.push(sv);
              }

              self.value_stack.pop();
              self.ty_stack.pop();
            }

            elements.reverse();

            let arr_ty_id = self
              .ty_checker
              .ty_table
              .intern_array(elem_ty, Some(count as u32));

            let arr_ty = self.ty_checker.intern_ty(Ty::Array(arr_ty_id));

            let dst = ValueId(self.sir.next_value_id);
            self.sir.next_value_id += 1;

            let sv = self.sir.emit(Insn::ArrayLiteral {
              dst,
              elements,
              ty_id: arr_ty,
            });

            let rid = self.values.store_runtime(0);

            self.value_stack.push(rid);
            self.ty_stack.push(arr_ty);
            self.sir_values.push(sv);
          }
        }
      }

      // === FUNCTION TYPE ANNOTATION: Fn(T1, T2) -> R ===
      Token::FnType => {
        let (ty_id, skip_to) = self.resolve_fn_type(idx);
        let value_id = self.values.store_type(ty_id);

        self.value_stack.push(value_id);
        self.ty_stack.push(self.ty_checker.type_type());

        self.skip_until = skip_to;
      }

      // === TYPE LITERALS ===
      _ if header.token.is_ty() => {
        let ty_id = self.resolve_type_token(idx);
        let value_id = self.values.store_type(ty_id);

        self.value_stack.push(value_id);
        self.ty_stack.push(self.ty_checker.type_type());
      }

      // === FIELD ACCESS / METHOD CALL: tup.0, s.lo, s.method() ===
      Token::Dot if self.value_stack.len() >= 2 => {
        // Shunting Yard reorders `obj . member` to postfix:
        // `obj member .`. Stack: [..., obj_val, member_val].

        // Pack-dotted chain (`pack.fn()`,
        // `outer.inner.fn()`): the Idents on the stack are
        // pure namespace placeholders, not runtime values,
        // and resolution to the mangled callee happens at
        // RParen. Collapse the two placeholders into a
        // single placeholder for intermediate chain dots
        // (`outer . inner` in `outer.inner.fn()`), or drop
        // both for the leaf dot that precedes `(` — there
        // is no receiver-as-`self` to preserve.
        if self.is_pack_chain_dot(idx) {
          self.value_stack.pop();
          self.ty_stack.pop();
          self.sir_values.pop();
          self.value_stack.pop();
          self.ty_stack.pop();
          self.sir_values.pop();

          let next_is_call = idx + 1 < self.tree.nodes.len()
            && self.tree.nodes[idx + 1].token == Token::LParen;

          if !next_is_call {
            // Intermediate dot: keep a placeholder on the
            // stack for the next Ident in the chain to pair
            // with.
            let placeholder = self.values.store_runtime(0);

            self.value_stack.push(placeholder);
            self.ty_stack.push(self.ty_checker.unit_type());
            self.sir_values.push(ValueId(u32::MAX));
          }

          return;
        }

        // Peek at receiver type to detect method calls.
        // If the member is a method (not a field), skip
        // the Dot — execute_potential_call will handle
        // it at RParen.
        if self.is_dot_method_call(idx) {
          // Don't consume stack — method call needs
          // the receiver as an argument.
          // Pop only the method name ident from stacks
          // (it's not a real value).
          self.value_stack.pop();
          self.ty_stack.pop();
          self.sir_values.pop();
          return;
        }

        // Pop index (integer literal or field name).
        let idx_val = self.value_stack.pop().unwrap();
        let _idx_ty = self.ty_stack.pop();

        self.sir_values.pop();

        // Pop struct/tuple.
        let _tup_val = self.value_stack.pop().unwrap();

        let tup_ty = self.ty_stack.pop().unwrap_or(self.ty_checker.unit_type());

        let tup_sir = self.sir_values.pop().unwrap_or(ValueId(u32::MAX));

        // Read the integer index from ValueStorage.
        let mut field_idx = {
          let vi = idx_val.0 as usize;

          if vi < self.values.kinds.len()
            && matches!(self.values.kinds[vi], Value::Int)
          {
            let ii = self.values.indices[vi] as usize;
            self.values.ints[ii] as u32
          } else {
            0
          }
        };

        // Resolve element type from tuple type.
        // Use kind_of to follow type variable indirections
        // (e.g. when tuple was inferred via := binding).
        let elem_ty = if let Ty::Tuple(tid) = self.ty_checker.kind_of(tup_ty) {
          if let Some(tup) = self.ty_checker.ty_table.tuple(tid) {
            let elems = self.ty_checker.ty_table.tuple_elems(tup);

            if (field_idx as usize) < elems.len() {
              elems[field_idx as usize]
            } else {
              // Out of bounds — compile error.
              let span = self.tree.spans[idx];

              report_error(Error::new(ErrorKind::TypeMismatch, span));
              self.ty_checker.error_type()
            }
          } else {
            self.ty_checker.unit_type()
          }
        } else if let Ty::Struct(sid) = self.ty_checker.kind_of(tup_ty) {
          // Struct field access: resolve field name.
          if let Some(st) = self.ty_checker.ty_table.struct_ty(sid) {
            let st = *st;
            let fields = self.ty_checker.ty_table.struct_fields(&st).to_vec();

            // idx_val is the field name ident.
            let field_name = self.node_value(idx - 1).and_then(|v| match v {
              NodeValue::Symbol(s) => Some(s),
              _ => None,
            });

            if let Some(fname) = field_name {
              let fname_str = self.interner.get(fname).to_owned();

              fields
                .iter()
                .enumerate()
                .find(|(_, f)| self.interner.get(f.name) == fname_str)
                .map(|(i, f)| {
                  field_idx = i as u32;
                  f.ty_id
                })
                .unwrap_or(self.ty_checker.unit_type())
            } else {
              self.ty_checker.unit_type()
            }
          } else {
            self.ty_checker.unit_type()
          }
        } else if matches!(self.ty_checker.kind_of(tup_ty), Ty::Array(_)) {
          // Array property access: only `.len` supported.
          let member_name = self.node_value(idx - 1).and_then(|v| match v {
            NodeValue::Symbol(s) => Some(s),
            _ => None,
          });

          let is_len =
            member_name.is_some_and(|s| self.interner.get(s) == "len");

          if is_len {
            let int_ty = self.ty_checker.int_type();
            let dst = ValueId(self.sir.next_value_id);

            self.sir.next_value_id += 1;

            let sv = self.sir.emit(Insn::ArrayLen {
              dst,
              array: tup_sir,
              ty_id: int_ty,
            });

            let rid = self.values.store_runtime(0);

            self.value_stack.push(rid);
            self.ty_stack.push(int_ty);
            self.sir_values.push(sv);

            return;
          }

          let span = self.tree.spans[idx];

          report_error(Error::new(ErrorKind::InvalidFieldAccess, span));

          self.ty_checker.unit_type()
        } else if matches!(self.ty_checker.kind_of(tup_ty), Ty::Str) {
          // String property access: only `.len` supported.
          let member_name = self.node_value(idx - 1).and_then(|v| match v {
            NodeValue::Symbol(s) => Some(s),
            _ => None,
          });

          let is_len =
            member_name.is_some_and(|s| self.interner.get(s) == "len");

          if is_len {
            // String layout: [len:8][data...]. Same as ArrayLen.
            let int_ty = self.ty_checker.int_type();
            let dst = ValueId(self.sir.next_value_id);

            self.sir.next_value_id += 1;

            let sv = self.sir.emit(Insn::ArrayLen {
              dst,
              array: tup_sir,
              ty_id: int_ty,
            });

            let rid = self.values.store_runtime(0);

            self.value_stack.push(rid);
            self.ty_stack.push(int_ty);
            self.sir_values.push(sv);

            return;
          }

          let span = self.tree.spans[idx];

          report_error(Error::new(ErrorKind::InvalidFieldAccess, span));

          self.ty_checker.unit_type()
        } else {
          self.ty_checker.unit_type()
        };

        let dst = ValueId(self.sir.next_value_id);

        self.sir.next_value_id += 1;

        let sv = self.sir.emit(Insn::TupleIndex {
          dst,
          tuple: tup_sir,
          index: field_idx,
          ty_id: elem_ty,
        });

        let rid = self.values.store_runtime(0);

        self.value_stack.push(rid);
        self.ty_stack.push(elem_ty);
        self.sir_values.push(sv);
      }

      // === BINARY OPERATORS ===
      Token::Plus => self.execute_binop(BinOp::Add, idx),
      Token::PlusPlus => self.execute_concat(idx),
      Token::Minus => self.execute_binop(BinOp::Sub, idx),
      Token::UnaryMinus => self.execute_unop(UnOp::Neg, idx),
      Token::Star => self.execute_binop(BinOp::Mul, idx),
      Token::Slash => self.execute_binop(BinOp::Div, idx),
      Token::Percent => self.execute_binop(BinOp::Rem, idx),

      // === COMPARISON OPERATORS ===
      Token::EqEq => self.execute_binop(BinOp::Eq, idx),
      Token::BangEq => self.execute_binop(BinOp::Neq, idx),
      Token::Lt => self.execute_binop(BinOp::Lt, idx),
      Token::LtEq => self.execute_binop(BinOp::Lte, idx),
      Token::Gt => self.execute_binop(BinOp::Gt, idx),
      Token::GtEq => self.execute_binop(BinOp::Gte, idx),

      // === LOGICAL OPERATORS ===
      Token::AmpAmp => self.execute_logical_binop(BinOp::And, idx),
      Token::PipePipe => self.execute_logical_binop(BinOp::Or, idx),

      // === BITWISE OPERATORS ===
      Token::Amp => self.execute_binop(BinOp::BitAnd, idx),
      Token::Pipe => self.execute_binop(BinOp::BitOr, idx),
      Token::Caret => self.execute_binop(BinOp::BitXor, idx),
      Token::LShift => self.execute_binop(BinOp::Shl, idx),
      Token::RShift => self.execute_binop(BinOp::Shr, idx),

      // === UNARY OPERATORS ===
      Token::Bang => self.execute_unop(UnOp::Not, idx),

      // === TYPE CAST: expr as Type ===
      Token::As => {
        // The next token should be a type keyword. Read it
        // and emit Cast. The value to cast is on the stack.
        if idx + 1 < self.tree.nodes.len()
          && self.tree.nodes[idx + 1].token.is_ty()
        {
          let to_ty = self.resolve_type_token(idx + 1);

          self.skip_until = idx + 2;

          if let (Some(_val), Some(from_ty)) =
            (self.value_stack.pop(), self.ty_stack.pop())
          {
            let src_sir = self.sir_values.pop().unwrap_or(ValueId(u32::MAX));

            let dst = ValueId(self.sir.next_value_id);
            self.sir.next_value_id += 1;

            let sv = self.sir.emit(Insn::Cast {
              dst,
              src: src_sir,
              from_ty,
              to_ty,
            });

            let rid = self.values.store_runtime(0);

            self.value_stack.push(rid);
            self.ty_stack.push(to_ty);
            self.sir_values.push(sv);
          }
        }
      }

      // === ENUM VARIANT ACCESS: Foo::Ok ===
      Token::ColonColon => {
        self.execute_enum_access(idx);
      }

      // === TYPE ANNOTATION / TERNARY FALSE ARM ===
      // === TYPE ANNOTATION ===
      Token::Colon => {
        if self
          .branch_stack
          .last()
          .is_some_and(|c| c.kind == BranchKind::Ternary && c.branch_emitted)
        {
          let ctx_idx = self.branch_stack.len() - 1;
          let ctx = self.branch_stack.last().unwrap();
          let end_label = ctx.end_label;
          let else_label = ctx.else_label.unwrap();
          let has_sink = ctx.value_sink.is_some();

          // Emit Return for the true arm — handles both
          // explicit `return when ...` and implicit return
          // (when as last expression in non-void function).
          //
          // Skipped when the ternary has a value_sink:
          // the sink captures the arm's value for the
          // merge Load, and that merged Load is what the
          // function ultimately returns. Emitting Return
          // here would consume the value before the Store
          // and break the φ.
          if !has_sink && let Some(ref mut fun_ctx) = self.current_function {
            let unit_ty = self.ty_checker.unit_type();

            let needs_return =
              fun_ctx.pending_return || fun_ctx.return_ty != unit_ty;

            if needs_return {
              let sir_val = self.sir_values.last().copied();
              let ty = self.ty_stack.last().copied().unwrap_or(unit_ty);

              self.sir.emit(Insn::Return {
                value: sir_val,
                ty_id: ty,
              });

              // Pop the true arm value so it doesn't leak
              // into the false arm (same fix as
              // check_pending_return).
              if sir_val.is_some() {
                self.value_stack.pop();
                self.ty_stack.pop();
                self.sir_values.pop();
              }

              fun_ctx.has_explicit_return = true;
            }
          }

          // φ merge: capture the true-arm's value into the
          // ternary's sink. pops stack top if the sink is
          // live; no-op otherwise. must happen BEFORE the
          // Jump so the Store lands on the true-arm's
          // control-flow path.
          if has_sink {
            self.emit_branch_sink_store(ctx_idx);
          }

          self.sir.emit(Insn::Jump { target: end_label });
          self.sir.emit(Insn::Label { id: else_label });
        } else {
          self.execute_ty_annotation(idx);
        }
      }

      // === TEMPLATE TOKENS ===
      Token::TemplateAssign => {
        let children_end = (header.child_start + header.child_count) as usize;
        self.execute_template_assign(idx, children_end);
      }

      Token::TemplateFragmentStart => {
        let children_end = (header.child_start + header.child_count) as usize;
        self.execute_template_fragment(idx, children_end);
        // Skip past the fragment so the parent loop
        // doesn't reprocess tag/text tokens.
        self.skip_until = children_end;
      }

      Token::TemplateText => {
        // Template text is now interned in tokenizer and comes as Symbol
        if let Some(NodeValue::Symbol(symbol)) = self.node_value(idx) {
          let value_id = self.values.store_string(symbol);
          self.value_stack.push(value_id);
          self.ty_stack.push(self.ty_checker.str_type());
        }
      }

      // === CONTROL FLOW ===
      Token::Return => self.execute_return(idx),

      Token::Break => {
        if let Some(ctx) = self
          .branch_stack
          .iter()
          .rev()
          .find(|c| matches!(c.kind, BranchKind::While | BranchKind::For))
        {
          self.sir.emit(Insn::Jump {
            target: ctx.end_label,
          });
        }
      }

      Token::Continue => {
        if let Some(ctx) = self
          .branch_stack
          .iter()
          .rev()
          .find(|c| matches!(c.kind, BranchKind::While | BranchKind::For))
        {
          // For `for` loops, emit the increment before
          // jumping back to the condition.
          if ctx.kind == BranchKind::For
            && let Some(var_name) = ctx.for_var
          {
            let int_ty = self.ty_checker.int_type();
            let ld = ValueId(self.sir.next_value_id);

            self.sir.next_value_id += 1;

            let ld_sir = self.sir.emit(Insn::Load {
              dst: ld,
              src: LoadSource::Local(var_name),
              ty_id: int_ty,
            });

            let one_dst = ValueId(self.sir.next_value_id);
            self.sir.next_value_id += 1;

            let one_sir = self.sir.emit(Insn::ConstInt {
              dst: one_dst,
              value: 1,
              ty_id: int_ty,
            });

            let add_dst = ValueId(self.sir.next_value_id);

            self.sir.next_value_id += 1;

            let add_sir = self.sir.emit(Insn::BinOp {
              dst: add_dst,
              op: zo_sir::BinOp::Add,
              lhs: ld_sir,
              rhs: one_sir,
              ty_id: int_ty,
            });

            self.sir.emit(Insn::Store {
              name: var_name,
              value: add_sir,
              ty_id: int_ty,
            });
          }

          if let Some(loop_label) = ctx.loop_label {
            self.sir.emit(Insn::Jump { target: loop_label });
          }
        }
      }

      // === STATEMENT TERMINATOR ===
      Token::Semicolon => {
        // Close ternary expressions.
        while self
          .branch_stack
          .last()
          .is_some_and(|c| c.kind == BranchKind::Ternary)
        {
          // φ: capture the false-arm's value into the
          // ternary's sink (mirrors the true-arm Store
          // emitted at the ternary's `:`). must happen
          // BEFORE the merge Label so both arms' paths
          // reach a Store before reconverging.
          let ctx_idx = self.branch_stack.len() - 1;

          self.emit_branch_sink_store(ctx_idx);

          let ctx = self.branch_stack.pop().unwrap();

          self.sir.emit(Insn::Label { id: ctx.end_label });

          // Merge: load the sink as the ternary's
          // result and push it onto the stacks. pure
          // no-op when the ternary is in statement
          // position (no sink).
          self.emit_branch_sink_load(&ctx);
        }

        // Finalize pending compound assignment (x += expr;).
        let _had_compound = self.pending_compound.is_some();
        self.finalize_pending_compound();

        // Finalize pending assignment (x = expr;).
        self.finalize_pending_array_assign();

        let had_assign = self.pending_assign.is_some();
        self.finalize_pending_assign();

        // Finalize any pending variable declaration.
        let had_decl = self.pending_decl.is_some();
        self.finalize_pending_decl();

        // Flush deferred binops before return — without
        // this, `return x * fact(x - 1)` loses the `*`
        // because the Call result is taken as the return
        // value before the deferred Mul resolves.
        self.apply_deferred_binop();

        // Clear any deferred binops that couldn't be
        // applied because the operand stack was empty —
        // they can't belong to the next statement, and
        // leaving them would let the next statement's
        // first pushed value be consumed as a stale rhs.
        //
        // Repro: `check((1) == 1); check(a | b == c);` —
        // the Eq from the first statement defers (parser
        // emits it before the second operand), Semicolon
        // fires before the defer resolves, and the second
        // statement's first push gets eaten as Eq's rhs.
        // Fix: discard any remaining deferred binops here;
        // statement boundaries are hard barriers for the
        // defer queue.
        self.deferred_binops.clear();
        // Same barrier applies to deferred short-circuits
        // that never finalized (e.g. malformed expressions).
        // Emit the dangling `end_label` for each — otherwise
        // the emitted `BranchIfNot` targets an undefined
        // label and codegen produces an invalid jump.
        while let Some(pending) = self.deferred_short_circuits.pop() {
          self.sir.emit(Insn::Label {
            id: pending.end_label,
          });
        }

        // Check if we have a pending return to complete.
        let had_return = self
          .current_function
          .as_ref()
          .is_some_and(|ctx| ctx.pending_return);
        self.check_pending_return();

        // Enforce grammar: assign_stmt and expr_stmt are
        // only valid inside function bodies.
        if self.current_function.is_none()
          && self.apply_context.is_none()
          && self.pack_context.is_empty()
          && (had_assign || (!had_decl && !had_return))
        {
          let span = self.tree.spans[idx];

          report_error(Error::new(ErrorKind::InvalidTopLevelItem, span));
        }

        // If nothing consumed the stacks, discard the
        // expression value so it doesn't leak to `}`.
        if !had_assign && !had_decl && !had_return {
          self.value_stack.pop();
          self.ty_stack.pop();
          self.sir_values.pop();
        }
      }

      // === ASSIGNMENT ===
      // Defer: the RHS hasn't been processed yet.
      // Pop the target identifier's value (it was pushed
      // as a variable reference but it's actually the
      // assignment target). Record the target name.
      // The Semicolon will finalize after the RHS.
      Token::Eq if idx >= 1 => {
        let target_idx = idx - 1;
        if let Token::Ident = self.tree.nodes[target_idx].token
          && let Some(NodeValue::Symbol(name)) = self.node_value(target_idx)
        {
          self.value_stack.pop();
          self.ty_stack.pop();
          self.sir_values.pop();

          let span = self.tree.spans[target_idx];

          self.pending_assign = Some((name, span));
        } else if self.tree.nodes[target_idx].token == Token::RBracket {
          // Array element assignment: arr[i] = value.
          // The ArrayIndex result is on the stack. Extract
          // array and index from the last ArrayIndex insn.
          if let Some(Insn::ArrayIndex { array, index, .. }) =
            self.sir.instructions.last()
          {
            let array_sir = *array;
            let index_sir = *index;

            // Find the array name from the Load instruction.
            let array_name =
              self.sir.instructions.iter().rev().find_map(|insn| {
                if let Insn::Load {
                  dst,
                  src: LoadSource::Local(sym),
                  ..
                } = insn
                  && *dst == array_sir
                {
                  Some(*sym)
                } else {
                  None
                }
              });

            if let Some(name) = array_name {
              // Pop the ArrayIndex result from stacks.
              self.value_stack.pop();
              self.ty_stack.pop();
              self.sir_values.pop();

              let span = self.tree.spans[target_idx];

              self.pending_array_assign =
                Some((array_sir, index_sir, name, span));
            }
          }
        }
      }

      // === COMPOUND ASSIGNMENTS ===
      Token::PlusEq => self.execute_compound_assignment(BinOp::Add, idx),
      Token::MinusEq => self.execute_compound_assignment(BinOp::Sub, idx),
      Token::StarEq => self.execute_compound_assignment(BinOp::Mul, idx),
      Token::SlashEq => self.execute_compound_assignment(BinOp::Div, idx),
      Token::PercentEq => self.execute_compound_assignment(BinOp::Rem, idx),
      Token::AmpEq => self.execute_compound_assignment(BinOp::BitAnd, idx),
      Token::PipeEq => self.execute_compound_assignment(BinOp::BitOr, idx),
      Token::CaretEq => self.execute_compound_assignment(BinOp::BitXor, idx),
      Token::LShiftEq => self.execute_compound_assignment(BinOp::Shl, idx),
      Token::RShiftEq => self.execute_compound_assignment(BinOp::Shr, idx),

      // Type keywords used as variable names in pattern
      // bindings (e.g., `Result::Ok(bytes)` where `bytes`
      // is tokenized as `BytesType`). Check for a local
      // whose name matches the keyword text and, if found,
      // treat it as a variable reference.
      _ => {
        if let Some(kw) = header.token.ty_keyword_str() {
          let sym = self.interner.intern(kw);
          let local_info = self
            .lookup_local(sym)
            .map(|l| (l.ty_id, l.sir_value, l.mutability));

          if let Some((ty_id, sir_value, _mutability)) = local_info
            && self.current_function.is_some()
            && sir_value.is_some()
          {
            let dst = ValueId(self.sir.next_value_id);
            self.sir.next_value_id += 1;

            let sv = self.sir.emit(Insn::Load {
              dst,
              src: LoadSource::Local(sym),
              ty_id,
            });

            let rid = self.values.store_runtime(0);

            self.value_stack.push(rid);
            self.ty_stack.push(ty_id);
            self.sir_values.push(sv);
          }
        }
      }
    }
  }

  /// Applies a deferred binary operator if its RHS is ready.
  fn apply_deferred_binop(&mut self) {
    // Finalize any deferred short-circuit whose RHS has
    // just landed on the stacks. Must run BEFORE the
    // regular binop drain — the short-circuit's RHS must
    // be captured into the φ sink before any enclosing
    // deferred binop tries to consume the stack top.
    self.apply_deferred_short_circuit();

    while !self.deferred_binops.is_empty()
      && !self.value_stack.is_empty()
      && !self.ty_stack.is_empty()
      && !self.sir_values.is_empty()
    {
      let (op, lhs, lhs_ty, lhs_sir, op_idx) =
        self.deferred_binops.pop().unwrap();

      let rhs = self.value_stack.pop().unwrap();
      let rhs_ty = self.ty_stack.pop().unwrap();
      let rhs_sir = self.sir_values.pop().unwrap();

      self.value_stack.push(lhs);
      self.ty_stack.push(lhs_ty);
      self.sir_values.push(lhs_sir);

      self.value_stack.push(rhs);
      self.ty_stack.push(rhs_ty);
      self.sir_values.push(rhs_sir);

      self.execute_binop(op, op_idx);

      // After this binop resolves, an enclosing short-
      // circuit's RHS may now be on the stacks. Drain
      // again so nested `a && b && c()` chains finalize
      // in the right order.
      self.apply_deferred_short_circuit();
    }
  }

  /// Finalize any deferred short-circuit whose RHS has
  /// arrived on the stacks. Emits the φ-sink tail
  /// (`Store sink, rhs; Label end; Load dst <- sink`).
  ///
  /// Guards (must ALL hold for the top pending SC to fire):
  ///   - stacks hold ≥ `pre_rhs_depth + 1` entries (the
  ///     RHS produced one new top-of-stack value);
  ///   - `tuple_ctx.len() <= pre_tuple_ctx_len` (we're back
  ///     out of any tuple/group entered after the op);
  ///   - `direct_call_depth <= pre_direct_call_depth` (we're
  ///     out of any direct call entered after the op — this
  ///     is what prevents `a || f(y)` from capturing the
  ///     call's `y` arg as the RHS before the call emits).
  ///
  /// When the guard fails (RHS not yet complete) we stop
  /// draining: the inner SC on the LIFO stack is not ready,
  /// and nothing outer could be either.
  fn apply_deferred_short_circuit(&mut self) {
    while let Some(pending) = self.deferred_short_circuits.last() {
      let depth_ok = self.sir_values.len() > pending.pre_rhs_depth
        && self.ty_stack.len() > pending.pre_rhs_depth
        && self.value_stack.len() > pending.pre_rhs_depth;
      let tuple_ok = self.tuple_ctx.len() <= pending.pre_tuple_ctx_len;
      let call_ok = self.direct_call_depth <= pending.pre_direct_call_depth;

      if !(depth_ok && tuple_ok && call_ok) {
        break;
      }

      let pending = self.deferred_short_circuits.pop().unwrap();

      let rhs_sir = self.sir_values.pop().unwrap();
      let _rhs_val = self.value_stack.pop().unwrap();
      let _rhs_ty = self.ty_stack.pop().unwrap();

      // Store the RHS into the sink. Both arms of the
      // short-circuit now Store into the same slot; the
      // Load that follows picks whichever ran.
      self.sir.emit(Insn::Store {
        name: pending.sink,
        value: rhs_sir,
        ty_id: pending.bool_ty,
      });

      // Merge label reached from both the LHS short-
      // circuit path and the full RHS path.
      self.sir.emit(Insn::Label {
        id: pending.end_label,
      });

      // Load the merged result as the expression's value.
      let dst = ValueId(self.sir.next_value_id);
      self.sir.next_value_id += 1;

      let sv = self.sir.emit(Insn::Load {
        dst,
        src: LoadSource::Local(pending.sink),
        ty_id: pending.bool_ty,
      });

      let rid = self.values.store_runtime(0);

      self.value_stack.push(rid);
      self.ty_stack.push(pending.bool_ty);
      self.sir_values.push(sv);
    }
  }

  /// Execute a short-circuit logical operator (`&&`/`||`).
  ///
  /// Mirrors the ternary φ-sink: allocate a synthetic
  /// `__branch_result_N__` slot, store the LHS into it,
  /// and emit a `BranchIfNot` that skips RHS evaluation
  /// whenever the LHS already determines the result.
  ///
  /// - `&&`: skip RHS when LHS is false. `BranchIfNot lhs`.
  /// - `||`: skip RHS when LHS is true. We don't have
  ///   `BranchIf`, so synthesize `UnOp::Not` into a temp
  ///   and `BranchIfNot` on that (path of least resistance,
  ///   matches the ternary's "no new Insn kinds" policy).
  ///
  /// If the RHS is already on the stacks at this point —
  /// pure constant `true && false` etc. — no side effects
  /// are possible, so delegate to `execute_binop` which
  /// handles constant folding. Only non-trivial RHS (call,
  /// grouping, nested op) takes the control-flow path.
  fn execute_logical_binop(&mut self, op: BinOp, node_idx: usize) {
    debug_assert!(matches!(op, BinOp::And | BinOp::Or));

    // Both operands already on stacks → no RHS work
    // remaining, no side effects to guard. Delegate to
    // the eager binop which handles const folding; codegen
    // of `BinOp::And/Or` on two evaluated bools is fine.
    if self.value_stack.len() >= 2
      && self.ty_stack.len() >= 2
      && self.sir_values.len() >= 2
    {
      self.execute_binop(op, node_idx);
      return;
    }

    // RHS pending — emit the short-circuit skeleton.
    if self.value_stack.is_empty()
      || self.ty_stack.is_empty()
      || self.sir_values.is_empty()
    {
      // Nothing to anchor on — degrade to the eager path
      // which already has its own "not enough operands"
      // defer branch.
      self.execute_binop(op, node_idx);
      return;
    }

    let _lhs_val = self.value_stack.pop().unwrap();
    let _lhs_ty = self.ty_stack.pop().unwrap();
    let lhs_sir = self.sir_values.pop().unwrap();

    // Snapshot stack/context depths *after* the LHS pop —
    // this is the state the RHS expression starts from, and
    // what finalization compares against to know the RHS
    // has fully materialized.
    let pre_rhs_depth = self.sir_values.len();
    let pre_tuple_ctx_len = self.tuple_ctx.len();
    let pre_direct_call_depth = self.direct_call_depth;

    let bool_ty = self.ty_checker.bool_type();

    // Allocate the φ sink.
    let n = self.branch_result_counter;
    self.branch_result_counter += 1;

    let sink = self.interner.intern(&format!("__branch_result_{n}__"));

    // Store(sink, lhs): default result is LHS, so the
    // skipped-RHS path reads LHS back through the sink.
    self.sir.emit(Insn::Store {
      name: sink,
      value: lhs_sir,
      ty_id: bool_ty,
    });

    let end_label = self.sir.next_label();

    // Emit the branch condition.
    //
    // `&&`: skip RHS when LHS is false → `BranchIfNot lhs`.
    // `||`: skip RHS when LHS is true → synthesize `!lhs`
    //       and `BranchIfNot !lhs`.
    let cond_sir = match op {
      BinOp::And => lhs_sir,
      BinOp::Or => {
        let dst = ValueId(self.sir.next_value_id);
        self.sir.next_value_id += 1;

        self.sir.emit(Insn::UnOp {
          dst,
          op: UnOp::Not,
          rhs: lhs_sir,
          ty_id: bool_ty,
        })
      }
      _ => unreachable!("execute_logical_binop called with non-logical op"),
    };

    self.sir.emit(Insn::BranchIfNot {
      cond: cond_sir,
      target: end_label,
    });

    self.deferred_short_circuits.push(DeferredShortCircuit {
      sink,
      end_label,
      bool_ty,
      pre_rhs_depth,
      pre_tuple_ctx_len,
      pre_direct_call_depth,
    });

    // `node_idx` kept in the signature for symmetry with
    // `execute_binop` — used for future error reporting at
    // the op's span. Silence the unused warning.
    let _ = node_idx;
  }

  /// Executes a binary operator.
  fn execute_binop(&mut self, op: BinOp, node_idx: usize) {
    // Pop operands (postfix order: left then right)
    if self.value_stack.len() < 2
      || self.ty_stack.len() < 2
      || self.sir_values.len() < 2
    {
      // Not enough operands — the RHS is inside a grouping
      // that hasn't closed yet. Defer this operator: pop the
      // LHS now and re-apply when RParen closes the group.
      if let (Some(lhs_sir), Some(lhs_ty), Some(lhs)) = (
        self.sir_values.pop(),
        self.ty_stack.pop(),
        self.value_stack.pop(),
      ) {
        self
          .deferred_binops
          .push((op, lhs, lhs_ty, lhs_sir, node_idx));
      }

      return;
    }

    let rhs = self.value_stack.pop().unwrap();
    let lhs = self.value_stack.pop().unwrap();

    let rhs_ty = self.ty_stack.pop().unwrap();
    let lhs_ty = self.ty_stack.pop().unwrap();

    // Pop SIR values for operands
    let rhs_sir = self.sir_values.pop().unwrap();
    let lhs_sir = self.sir_values.pop().unwrap();

    // Get span from the spans array (1:1 with nodes)
    let span = self.tree.spans[node_idx];

    match self.ty_checker.unify(lhs_ty, rhs_ty, span) {
      Some(ty_id) => {
        // Try constant folding — but only if both operands
        // are compile-time constants. Skip when either is a
        // runtime value (e.g., function call result) to
        // avoid incorrect folding across executor passes.
        let lhs_is_const =
          self.values.kinds.get(lhs.0 as usize).is_some_and(|k| {
            matches!(k, Value::Int | Value::Float | Value::Bool | Value::Char)
          });

        let rhs_is_const =
          self.values.kinds.get(rhs.0 as usize).is_some_and(|k| {
            matches!(k, Value::Int | Value::Float | Value::Bool | Value::Char)
          });

        let mut constprop = ConstFold::new(&self.values, self.interner);
        let resolved_ty = self.ty_checker.resolve_ty(ty_id);

        if lhs_is_const
          && rhs_is_const
          && let Some(folded) =
            constprop.fold_binop(op, lhs, rhs, span, resolved_ty)
        {
          match folded {
            FoldResult::Int(value) => {
              self.nop_folded_operands(lhs_sir, rhs_sir);

              let dst = ValueId(self.sir.next_value_id);
              self.sir.next_value_id += 1;

              let sir_value =
                self.sir.emit(Insn::ConstInt { dst, value, ty_id });
              let value_id = self.values.store_int(value);

              self.value_stack.push(value_id);
              self.ty_stack.push(ty_id);
              self.sir_values.push(sir_value);
              self.annotations.push(Annotation { node_idx, ty_id });

              return;
            }
            FoldResult::Float(value) => {
              self.nop_folded_operands(lhs_sir, rhs_sir);

              let dst = ValueId(self.sir.next_value_id);
              self.sir.next_value_id += 1;

              let sir_value =
                self.sir.emit(Insn::ConstFloat { dst, value, ty_id });
              let value_id = self.values.store_float(value);

              self.value_stack.push(value_id);
              self.ty_stack.push(ty_id);
              self.sir_values.push(sir_value);
              self.annotations.push(Annotation { node_idx, ty_id });

              return;
            }
            FoldResult::Bool(value) => {
              self.nop_folded_operands(lhs_sir, rhs_sir);

              let ty_id = self.ty_checker.bool_type();

              let dst = ValueId(self.sir.next_value_id);
              self.sir.next_value_id += 1;

              let sir_value =
                self.sir.emit(Insn::ConstBool { dst, value, ty_id });
              let value_id = self.values.store_bool(value);

              self.value_stack.push(value_id);
              self.ty_stack.push(ty_id);
              self.sir_values.push(sir_value);
              self.annotations.push(Annotation { node_idx, ty_id });

              return;
            }
            FoldResult::Str(symbol) => {
              self.nop_folded_operands(lhs_sir, rhs_sir);

              let str_ty = self.ty_checker.str_type();

              let dst = ValueId(self.sir.next_value_id);
              self.sir.next_value_id += 1;

              let sir_value = self.sir.emit(Insn::ConstString {
                dst,
                symbol,
                ty_id: str_ty,
              });
              let value_id = self.values.store_string(symbol);

              self.value_stack.push(value_id);
              self.ty_stack.push(str_ty);
              self.sir_values.push(sir_value);
              self.annotations.push(Annotation {
                node_idx,
                ty_id: str_ty,
              });

              return;
            }
            FoldResult::Forward(operand) => {
              let (fwd_val, fwd_sir) = match operand {
                Operand::Lhs => (lhs, lhs_sir),
                Operand::Rhs => (rhs, rhs_sir),
              };

              self.value_stack.push(fwd_val);
              self.ty_stack.push(ty_id);
              self.sir_values.push(fwd_sir);
              self.annotations.push(Annotation { node_idx, ty_id });

              return;
            }
            FoldResult::Strength(new_op, const_rhs) => {
              // emit the constant rhs (shift amount or mask).
              let rhs_dst = ValueId(self.sir.next_value_id);
              self.sir.next_value_id += 1;

              let rhs_sir_val = self.sir.emit(Insn::ConstInt {
                dst: rhs_dst,
                value: const_rhs,
                ty_id,
              });

              // emit the cheaper op with lhs forwarded.
              let dst = ValueId(self.sir.next_value_id);

              self.sir.next_value_id += 1;

              let sir_value = self.sir.emit(Insn::BinOp {
                dst,
                op: new_op,
                lhs: lhs_sir,
                rhs: rhs_sir_val,
                ty_id,
              });

              let runtime_id = self.values.store_runtime(0);

              self.value_stack.push(runtime_id);
              self.ty_stack.push(ty_id);
              self.sir_values.push(sir_value);
              self.annotations.push(Annotation { node_idx, ty_id });

              return;
            }
            FoldResult::Error(error) => {
              report_error(error);

              // [note] — push error values to maintain stack consistency.
              let error_id = self.values.store_runtime(u32::MAX);

              self.value_stack.push(error_id);
              self.ty_stack.push(self.ty_checker.error_type());
              self.sir_values.push(ValueId(u32::MAX));

              return;
            }
          }
        }

        // Abstract operator dispatch: if operands are
        // structs with an Eq impl, call Type::eq instead
        // of emitting a primitive BinOp.
        if matches!(op, BinOp::Eq | BinOp::Neq) {
          let resolved = self.ty_checker.kind_of(ty_id);

          let type_name = match resolved {
            Ty::Struct(sid) => {
              self.ty_checker.ty_table.struct_ty(sid).map(|s| s.name)
            }
            _ => None,
          };

          if let Some(tname) = type_name {
            let eq_sym = self.interner.intern("Eq");

            if self.abstract_impls.contains_key(&(eq_sym, tname)) {
              let ts = self.interner.get(tname).to_owned();
              let mangled = format!("{ts}::eq");
              let eq_fn = self.interner.intern(&mangled);

              if self.funs.iter().any(|f| f.name == eq_fn) {
                let call_dst = ValueId(self.sir.next_value_id);
                self.sir.next_value_id += 1;

                let bool_ty = self.ty_checker.bool_type();
                let mut call_sir = self.sir.emit(Insn::Call {
                  dst: call_dst,
                  name: eq_fn,
                  args: vec![lhs_sir, rhs_sir],
                  ty_id: bool_ty,
                });

                // Neq: negate the result.
                if op == BinOp::Neq {
                  let neg_dst = ValueId(self.sir.next_value_id);
                  self.sir.next_value_id += 1;

                  call_sir = self.sir.emit(Insn::UnOp {
                    dst: neg_dst,
                    op: UnOp::Not,
                    rhs: call_sir,
                    ty_id: bool_ty,
                  });
                }

                let runtime_id = self.values.store_runtime(0);

                self.value_stack.push(runtime_id);
                self.ty_stack.push(bool_ty);
                self.sir_values.push(call_sir);
                self.annotations.push(Annotation {
                  node_idx,
                  ty_id: bool_ty,
                });

                return;
              }
            }
          }
        }

        // Comparison ops produce bool for the type
        // stack; the SIR keeps the operand type so
        // codegen can distinguish int vs float.
        let stack_ty = match op {
          BinOp::Eq
          | BinOp::Neq
          | BinOp::Lt
          | BinOp::Lte
          | BinOp::Gt
          | BinOp::Gte => self.ty_checker.bool_type(),
          _ => ty_id,
        };

        let dst = ValueId(self.sir.next_value_id);

        self.sir.next_value_id += 1;

        let sir_value = self.sir.emit(Insn::BinOp {
          dst,
          op,
          lhs: lhs_sir,
          rhs: rhs_sir,
          ty_id,
        });

        let runtime_id = self.values.store_runtime(0);

        self.value_stack.push(runtime_id);
        self.ty_stack.push(stack_ty);
        self.sir_values.push(sir_value);
        self.annotations.push(Annotation {
          node_idx,
          ty_id: stack_ty,
        });
      }
      None => {
        let error_id = self.values.store_runtime(u32::MAX);

        self.value_stack.push(error_id);
        self.ty_stack.push(self.ty_checker.error_type()); // Error type
      }
    }
  }

  /// Executes string concatenation (`++`).
  ///
  /// If both operands are compile-time strings, folds into
  /// a single interned ConstString. Otherwise emits a
  /// runtime BinOp::Concat.
  fn execute_concat(&mut self, node_idx: usize) {
    if self.value_stack.len() < 2
      || self.ty_stack.len() < 2
      || self.sir_values.len() < 2
    {
      return;
    }

    let rhs = self.value_stack.pop().unwrap();
    let lhs = self.value_stack.pop().unwrap();

    let rhs_ty = self.ty_stack.pop().unwrap();
    let lhs_ty = self.ty_stack.pop().unwrap();

    let rhs_sir = self.sir_values.pop().unwrap();
    let lhs_sir = self.sir_values.pop().unwrap();

    let span = self.tree.spans[node_idx];
    let str_ty = self.ty_checker.str_type();

    // Type check: both must be str.
    if self.ty_checker.unify(lhs_ty, str_ty, span).is_none()
      || self.ty_checker.unify(rhs_ty, str_ty, span).is_none()
    {
      let error_id = self.values.store_runtime(u32::MAX);

      self.value_stack.push(error_id);
      self.ty_stack.push(self.ty_checker.error_type());

      return;
    }

    // Compile-time fold. Resolve string symbols from
    // value storage (direct literals) or by tracing the
    // SIR Load back to the local's original string value.
    let resolve_sym = |vid: ValueId,
                       sir_vid: ValueId,
                       values: &ValueStorage,
                       locals: &[Local],
                       sir: &Sir|
     -> Option<Symbol> {
      // Direct string value (literal operand).
      let vi = vid.0 as usize;

      if vi < values.kinds.len() && matches!(values.kinds[vi], Value::String) {
        let si = values.indices[vi] as usize;

        return Some(values.strings[si]);
      }

      // Runtime value — find the Load instruction in
      // SIR, get the local name, then resolve.
      for insn in sir.instructions.iter() {
        if let Insn::Load {
          dst,
          src: LoadSource::Local(sym),
          ..
        } = insn
          && *dst == sir_vid
          && let Some(local) = locals.iter().rev().find(|l| l.name == *sym)
        {
          let lvi = local.value_id.0 as usize;

          if lvi < values.kinds.len()
            && matches!(values.kinds[lvi], Value::String)
          {
            let si = values.indices[lvi] as usize;

            return Some(values.strings[si]);
          }
        }
      }

      None
    };

    let lhs_sym =
      resolve_sym(lhs, lhs_sir, &self.values, &self.locals, &self.sir);
    let rhs_sym =
      resolve_sym(rhs, rhs_sir, &self.values, &self.locals, &self.sir);

    if let (Some(ls), Some(rs)) = (lhs_sym, rhs_sym) {
      let lstr = self.interner.get(ls);
      let rstr = self.interner.get(rs);
      let result = format!("{lstr}{rstr}");
      let sym = self.interner.intern(&result);

      let dst = ValueId(self.sir.next_value_id);
      self.sir.next_value_id += 1;

      let sir_value = self.sir.emit(Insn::ConstString {
        dst,
        symbol: sym,
        ty_id: str_ty,
      });
      let value_id = self.values.store_string(sym);

      self.value_stack.push(value_id);
      self.ty_stack.push(str_ty);
      self.sir_values.push(sir_value);

      self.annotations.push(Annotation {
        node_idx,
        ty_id: str_ty,
      });

      return;
    }

    // Runtime concat — emit BinOp::Concat.
    let dst = ValueId(self.sir.next_value_id);

    self.sir.next_value_id += 1;

    let sir_value = self.sir.emit(Insn::BinOp {
      dst,
      op: BinOp::Concat,
      lhs: lhs_sir,
      rhs: rhs_sir,
      ty_id: str_ty,
    });

    let runtime_id = self.values.store_runtime(0);

    self.value_stack.push(runtime_id);
    self.ty_stack.push(str_ty);
    self.sir_values.push(sir_value);
    self.annotations.push(Annotation {
      node_idx,
      ty_id: str_ty,
    });
  }

  /// Executes a unary operator.
  fn execute_unop(&mut self, op: UnOp, node_idx: usize) {
    if self.value_stack.is_empty()
      || self.ty_stack.is_empty()
      || self.sir_values.is_empty()
    {
      return;
    }

    let rhs_id = self.value_stack.pop().unwrap();
    let rhs_ty = self.ty_stack.pop().unwrap();
    let operand_sir = self.sir_values.pop().unwrap();

    // Get span from the spans array (1:1 with nodes)
    let span = self.tree.spans[node_idx];

    // Type check based on operator
    let ty_id = match op {
      UnOp::Neg => rhs_ty,
      UnOp::Not => {
        // Logical not requires bool
        let bool_ty = self.ty_checker.bool_type();

        match self.ty_checker.unify(rhs_ty, bool_ty, span) {
          Some(ty_id) => ty_id,
          None => {
            self.value_stack.push(self.values.store_runtime(u32::MAX));
            self.ty_stack.push(self.ty_checker.error_type());

            return;
          }
        }
      }
      // TODO: Handle these properly
      UnOp::Ref | UnOp::Deref | UnOp::BitNot => rhs_ty,
    };

    // Try constant folding using the ConstFold module
    let constprop = ConstFold::new(&self.values, self.interner);
    let resolved_ty = self.ty_checker.resolve_ty(ty_id);

    if let Some(folded) = constprop.fold_unop(op, rhs_id, span, resolved_ty) {
      match folded {
        FoldResult::Int(value) => {
          let dst = ValueId(self.sir.next_value_id);
          self.sir.next_value_id += 1;

          let sir_value = self.sir.emit(Insn::ConstInt { dst, value, ty_id });
          let value_id = self.values.store_int(value);

          self.value_stack.push(value_id);
          self.ty_stack.push(ty_id);
          self.sir_values.push(sir_value);
          self.annotations.push(Annotation { node_idx, ty_id });

          return;
        }
        FoldResult::Float(value) => {
          let dst = ValueId(self.sir.next_value_id);
          self.sir.next_value_id += 1;

          let sir_value = self.sir.emit(Insn::ConstFloat { dst, value, ty_id });
          let value_id = self.values.store_float(value);

          self.value_stack.push(value_id);
          self.ty_stack.push(ty_id);
          self.sir_values.push(sir_value);
          self.annotations.push(Annotation { node_idx, ty_id });

          return;
        }
        FoldResult::Bool(value) => {
          let dst = ValueId(self.sir.next_value_id);
          self.sir.next_value_id += 1;

          let sir_value = self.sir.emit(Insn::ConstBool { dst, value, ty_id });
          let value_id = self.values.store_bool(value);

          self.value_stack.push(value_id);
          self.ty_stack.push(ty_id);
          self.sir_values.push(sir_value);
          self.annotations.push(Annotation { node_idx, ty_id });

          return;
        }
        // note: Forward/Strength/Str are unreachable for unary ops,
        // but handle for exhaustiveness.
        FoldResult::Str(_)
        | FoldResult::Forward(_)
        | FoldResult::Strength(..) => {
          self.value_stack.push(rhs_id);
          self.ty_stack.push(ty_id);
          self.sir_values.push(operand_sir);
          self.annotations.push(Annotation { node_idx, ty_id });

          return;
        }
        FoldResult::Error(error) => {
          report_error(error);

          // [note] — push error values to maintain stack consistency.
          let error_id = self.values.store_runtime(u32::MAX);

          self.value_stack.push(error_id);
          self.ty_stack.push(self.ty_checker.error_type());
          self.sir_values.push(ValueId(u32::MAX));

          return;
        }
      }
    }

    // Runtime operation
    let dst = ValueId(self.sir.next_value_id);
    self.sir.next_value_id += 1;

    let sir_value = self.sir.emit(Insn::UnOp {
      dst,
      op,
      rhs: operand_sir,
      ty_id,
    });

    let runtime_id = self.values.store_runtime(0);

    self.value_stack.push(runtime_id);
    self.ty_stack.push(ty_id);
    self.sir_values.push(sir_value);
    self.annotations.push(Annotation { node_idx, ty_id });
  }

  /// Executes type annotation.
  fn execute_ty_annotation(&mut self, idx: usize) {
    if self.value_stack.len() >= 2 && self.ty_stack.len() >= 2 {
      // Pop type value
      let ty_value = self.value_stack.pop().unwrap();
      let _ty_ty = self.ty_stack.pop().unwrap(); // Should be Type type
      let span = self.tree.spans[idx];

      if let Some(unified) = self
        .ty_value(ty_value)
        .and_then(|ty| self.ty_stack.last().map(|&var_ty| (ty, var_ty)))
        .and_then(|(ty, var_ty)| self.ty_checker.unify(var_ty, ty, span))
      {
        self.ty_stack.pop();
        self.ty_stack.push(unified);
      }
    }
  }

  /// Executes function declaration.
  /// Executes a `load` statement.
  ///
  /// Extracts path segments from children (Ident nodes between
  /// ColonColon separators) and emits `Insn::ModuleLoad`.
  fn execute_load(&mut self, _start_idx: usize, end_idx: usize) {
    let mut path = Vec::new();

    for child_idx in (_start_idx + 1)..end_idx {
      if let Some(node) = self.tree.nodes.get(child_idx)
        && node.token == Token::Ident
        && let Some(NodeValue::Symbol(sym)) = self.node_value(child_idx)
      {
        path.push(sym);
      }
    }

    self.sir.emit(Insn::ModuleLoad {
      path,
      imported_symbols: Vec::new(),
    });
  }

  /// Executes a `pack` statement.
  ///
  /// Extracts the pack name from children and emits
  /// `Insn::PackDecl`.
  fn execute_pack(&mut self, start_idx: usize, end_idx: usize) {
    // First Ident after `pack` is the pack name. Scanning
    // matches the `execute_apply` shape (parser may place
    // minor tokens between `pack` and the name).
    let name_idx = ((start_idx + 1)..end_idx)
      .find(|&i| self.tree.nodes[i].token == Token::Ident);

    let name = name_idx.and_then(|i| match self.node_value(i) {
      Some(NodeValue::Symbol(s)) => Some(s),
      _ => None,
    });

    // Locate the pack's own `{` and its matching `}` by
    // bracket-depth counting. `end_idx` from the parser's
    // child-span is UNRELIABLE here — when sibling items
    // (`fun main` after `pack inner { ... }`) are listed
    // under the Pack node, `end_idx` inflates past the
    // pack's own `}`. Same child-span-inflation class as
    // CL13's match / for fixes: always bound iteration by
    // the construct's own terminator.
    let lbrace_idx = name_idx.map(|i| i + 1).unwrap_or(start_idx + 2);
    let lbrace_idx = (lbrace_idx..end_idx)
      .find(|&i| self.tree.nodes[i].token == Token::LBrace);

    let rbrace_idx = lbrace_idx.and_then(|lb| {
      let mut depth = 1_i32;
      let mut j = lb + 1;

      while j < self.tree.nodes.len() && depth > 0 {
        match self.tree.nodes[j].token {
          Token::LBrace => depth += 1,
          Token::RBrace => {
            depth -= 1;

            if depth == 0 {
              return Some(j);
            }
          }
          _ => {}
        }

        j += 1;
      }

      None
    });

    // Real end for this pack's body is rbrace_idx + 1 (so
    // the `}` itself is processed by the body loop for
    // scope pop symmetry).
    let pack_end = rbrace_idx.map(|i| i + 1).unwrap_or(end_idx);

    if let Some(name) = name {
      self.sir.emit(Insn::PackDecl {
        name,
        pubness: if self.is_pub(start_idx) {
          Pubness::Yes
        } else {
          Pubness::No
        },
      });

      // Register the simple pack name so `inner.hello()`
      // style calls can recognise `inner` / `inner2` as
      // namespace prefixes (the Ident handler would
      // otherwise flag them as undefined variables).
      self.pack_names.insert(name);
    }

    // Enter pack context: nested `fun`s will be mangled
    // through `execute_fun` as `outer::inner::name`.
    if let Some(sym) = name {
      self.pack_context.push((sym, pack_end));
    }

    // Advance past the pack's name + `{` to the first body
    // child.
    let mut idx = match lbrace_idx {
      Some(lb) => lb + 1,
      None => pack_end,
    };

    while idx < pack_end {
      if idx < self.skip_until {
        idx += 1;
        continue;
      }

      let node = self.tree.nodes[idx];

      self.execute_node(&node, idx);
      idx += 1;
    }

    if name.is_some() {
      self.pack_context.pop();
    }

    // Prevent the outer main loop from re-walking the
    // pack's own Ident / LBrace / body / RBrace; anything
    // AFTER the pack's own `}` (e.g. `fun main` that the
    // parser folded under our child-span) must fall back
    // to the outer loop.
    self.skip_until = pack_end;
  }

  /// Resolves an N-dimensional array type starting at `start`.
  /// Parses `[N1][N2]...[Nk]T` and returns `(TyId, next_idx)`.
  /// The first `[` is at `start`.
  fn resolve_array_type(
    &mut self,
    start: usize,
    end: usize,
  ) -> Option<(TyId, usize)> {
    let mut j = start;
    let mut dims: Vec<Option<u32>> = Vec::new();

    // Collect all [N] dimensions.
    while j < end && self.tree.nodes[j].token == Token::LBracket {
      let mut k = j + 1;
      let mut dim_size: Option<u32> = None;

      if k < end && self.tree.nodes[k].token == Token::Int {
        if let Some(NodeValue::Literal(lit_idx)) = self.node_value(k) {
          dim_size = Some(self.literals.int_literals[lit_idx as usize] as u32);
        }
        k += 1;
      }

      if k < end && self.tree.nodes[k].token == Token::RBracket {
        k += 1;
      }

      dims.push(dim_size);
      j = k;
    }

    // Resolve the base element type.
    if j < end && self.tree.nodes[j].token.is_ty() {
      let base_ty = self.resolve_type_token(j);
      j += 1;

      // Build from inside out:
      // [2][3]int → [3]int → [2][3]int.
      let mut ty = base_ty;

      for dim in dims.iter().rev() {
        let aid = self.ty_checker.ty_table.intern_array(ty, *dim);
        ty = self.ty_checker.intern_ty(Ty::Array(aid));
      }

      Some((ty, j))
    } else {
      None
    }
  }

  /// Resolves a type token at `idx` to a [`TyId`].
  fn resolve_type_token(&mut self, idx: usize) -> TyId {
    match self.tree.nodes[idx].token {
      Token::IntType => self.ty_checker.int_type(),
      Token::S8Type => self.ty_checker.intern_ty(Ty::Int {
        signed: true,
        width: zo_ty::IntWidth::S8,
      }),
      Token::S16Type => self.ty_checker.intern_ty(Ty::Int {
        signed: true,
        width: zo_ty::IntWidth::S16,
      }),
      Token::S32Type => self.ty_checker.s32_type(),
      Token::S64Type => self.ty_checker.intern_ty(Ty::Int {
        signed: true,
        width: zo_ty::IntWidth::S64,
      }),
      Token::UintType => self.ty_checker.intern_ty(Ty::Int {
        signed: false,
        width: zo_ty::IntWidth::U32,
      }),
      Token::U8Type => self.ty_checker.intern_ty(Ty::Int {
        signed: false,
        width: zo_ty::IntWidth::U8,
      }),
      Token::U16Type => self.ty_checker.intern_ty(Ty::Int {
        signed: false,
        width: zo_ty::IntWidth::U16,
      }),
      Token::U32Type => self.ty_checker.u32_type(),
      Token::U64Type => self.ty_checker.intern_ty(Ty::Int {
        signed: false,
        width: zo_ty::IntWidth::U64,
      }),
      Token::FloatType => self.ty_checker.f64_type(),
      Token::F32Type => self.ty_checker.f32_type(),
      Token::F64Type => self.ty_checker.f64_type(),
      Token::BoolType => self.ty_checker.bool_type(),
      Token::CharType => self.ty_checker.char_type(),
      Token::StrType => self.ty_checker.str_type(),
      Token::BytesType => self.ty_checker.bytes_type(),
      Token::TemplateType => self.ty_checker.template_ty(),
      Token::Ident => {
        if let Some(NodeValue::Symbol(sym)) = self.node_value(idx) {
          self
            .ty_checker
            .resolve_ty_symbol(sym, self.interner)
            .unwrap_or_else(|| self.ty_checker.unit_type())
        } else {
          self.ty_checker.unit_type()
        }
      }
      Token::SelfUpper => {
        // Resolve Self to the applied type.
        if let Some(type_name) = self.apply_context {
          self
            .ty_checker
            .resolve_ty_name(type_name)
            .unwrap_or_else(|| self.ty_checker.unit_type())
        } else {
          self.ty_checker.unit_type()
        }
      }
      // Generic type parameter: $T.
      // Dollar is followed by Ident(T). Look up in the
      // active type_params mapping.
      Token::Dollar => {
        if idx + 1 < self.tree.nodes.len()
          && self.tree.nodes[idx + 1].token == Token::Ident
        {
          if let Some(NodeValue::Symbol(sym)) = self.node_value(idx + 1) {
            // Find the type param's inference var.
            if let Some((_, ty)) =
              self.type_params.iter().find(|(name, _)| *name == sym)
            {
              *ty
            } else {
              // $U not declared in <$T, ...>.
              let span = self.tree.spans[idx];

              report_error(Error::new(ErrorKind::UndefinedTypeParam, span));

              self.ty_checker.error_type()
            }
          } else {
            self.ty_checker.fresh_var()
          }
        } else {
          self.ty_checker.fresh_var()
        }
      }
      _ => self.ty_checker.unit_type(),
    }
  }

  /// Resolves a `Fn(T1, T2) -> R` type annotation.
  ///
  /// Scans forward from the FnType token to consume the full
  /// pattern: `FnType ( type1 , type2 ) -> return_type`.
  /// Returns `(TyId, skip_to)` where skip_to is the index
  /// past the last consumed node.
  fn resolve_fn_type(&mut self, idx: usize) -> (TyId, usize) {
    let len = self.tree.nodes.len();
    let mut j = idx + 1;
    let mut param_tys = Vec::new();
    let mut return_ty = self.ty_checker.unit_type();

    // Skip (
    if j < len && self.tree.nodes[j].token == Token::LParen {
      j += 1;
    }

    // Collect param types until )
    while j < len && self.tree.nodes[j].token != Token::RParen {
      let tok = self.tree.nodes[j].token;

      if tok == Token::Comma {
        j += 1;

        continue;
      }

      if tok == Token::FnType {
        // Nested Fn type: Fn(Fn(int) -> int) -> int
        let (nested_ty, skip) = self.resolve_fn_type(j);

        param_tys.push(nested_ty);

        j = skip;

        continue;
      }

      if tok.is_ty() {
        param_tys.push(self.resolve_type_token(j));
      }

      j += 1;
    }

    // Skip )
    if j < len && self.tree.nodes[j].token == Token::RParen {
      j += 1;
    }

    // Check for -> return type
    if j < len && self.tree.nodes[j].token == Token::Arrow {
      j += 1;

      if j < len {
        let tok = self.tree.nodes[j].token;

        if tok == Token::FnType {
          // Return type is a Fn type
          let (nested_ty, skip) = self.resolve_fn_type(j);

          return_ty = nested_ty;
          j = skip;
        } else if tok.is_ty() {
          return_ty = self.resolve_type_token(j);

          j += 1;
        }
      }
    }

    let fun_ty_id = self.ty_checker.ty_table.intern_fun(param_tys, return_ty);
    let ty_id = self.ty_checker.intern_ty(Ty::Fun(fun_ty_id));

    (ty_id, j)
  }

  /// Resolves a `(T1, T2, ...) ` tuple type annotation.
  ///
  /// Scans forward from `(` to consume the full pattern.
  /// Returns `(TyId, skip_to)`.
  fn resolve_tuple_type(&mut self, idx: usize) -> (TyId, usize) {
    let len = self.tree.nodes.len();
    let mut j = idx + 1; // Skip (
    let mut elem_tys = Vec::new();

    while j < len && self.tree.nodes[j].token != Token::RParen {
      let tok = self.tree.nodes[j].token;

      if tok == Token::Comma {
        j += 1;

        continue;
      }

      if tok == Token::FnType {
        let (nested, skip) = self.resolve_fn_type(j);
        elem_tys.push(nested);

        j = skip;

        continue;
      }

      if tok == Token::LParen {
        // Nested tuple type.
        let (nested, skip) = self.resolve_tuple_type(j);
        elem_tys.push(nested);

        j = skip;

        continue;
      }

      // Accept both keyword types (`int`, `bool`, …) and
      // user-defined idents (struct/enum/alias names).
      // Without the Ident arm, `(Point, Point)` produced an
      // empty-element tuple — `resolve_type_token` already
      // knows how to resolve idents, but `is_ty()` is
      // deliberately keyword-only, so the tuple loop needs
      // its own arm.
      if tok.is_ty() || tok == Token::Ident {
        elem_tys.push(self.resolve_type_token(j));
      }

      j += 1;
    }

    // Skip )
    if j < len && self.tree.nodes[j].token == Token::RParen {
      j += 1;
    }

    let tuple_ty_id = self.ty_checker.ty_table.intern_tuple(elem_tys);
    let ty_id = self.ty_checker.intern_ty(Ty::Tuple(tuple_ty_id));

    (ty_id, j)
  }

  /// Scans closure body for identifiers that reference
  /// outer-scope locals (captures). Returns deduplicated list
  /// with type and mutability info.
  fn identify_captures(
    &self,
    body_start: usize,
    body_end: usize,
    params: &[(Symbol, TyId)],
  ) -> Vec<(Symbol, TyId, bool)> {
    let mut captures = Vec::new();
    let mut seen = Vec::new();

    for idx in body_start..body_end {
      if self.tree.nodes[idx].token != Token::Ident {
        continue;
      }

      if let Some(NodeValue::Symbol(sym)) = self.node_value(idx) {
        // Skip closure params.
        if params.iter().any(|(n, _)| *n == sym) {
          continue;
        }

        // Skip self-reference (recursive closure).
        if self.pending_decl.as_ref().is_some_and(|d| d.name == sym) {
          continue;
        }

        // Skip already captured.
        if seen.contains(&sym) {
          continue;
        }

        // Check if it's an outer local.
        if let Some(local) = self.lookup_local(sym) {
          let is_mutable = local.mutability == Mutability::Yes;

          captures.push((sym, local.ty_id, is_mutable));
          seen.push(sym);
        }
      }
    }

    captures
  }

  /// Executes a closure expression: `fn(params) { body }`
  /// or `fn(params) => expr`.
  ///
  /// Closures are anonymous functions with by-copy capture.
  /// Captures become prepended parameters in the generated
  /// FunDef. The closure value is pushed onto the stack.
  fn execute_closure(&mut self, start_idx: usize, end_idx: usize) {
    // -- 1. Parse parameters ---------------------------------

    let mut params: Vec<(Symbol, TyId)> = Vec::new();
    let mut return_ty = self.ty_checker.unit_type();
    let mut idx = start_idx + 1; // Skip Fn token.

    // Skip LParen.
    if idx < end_idx && self.tree.nodes[idx].token == Token::LParen {
      idx += 1;

      while idx < end_idx {
        match self.tree.nodes[idx].token {
          Token::RParen => {
            idx += 1;

            break;
          }
          Token::Ident => {
            if let Some(NodeValue::Symbol(pname)) = self.node_value(idx) {
              idx += 1;

              // Typed param: `x: int` or untyped: `x`
              let pty = if idx < end_idx && self.tree.nodes[idx].token.is_ty() {
                let ty = self.resolve_type_token(idx);

                idx += 1;

                ty
              } else {
                self.ty_checker.fresh_var()
              };

              params.push((pname, pty));

              // Skip comma.
              if idx < end_idx && self.tree.nodes[idx].token == Token::Comma {
                idx += 1;
              }
            } else {
              idx += 1;
            }
          }
          _ => idx += 1,
        }
      }
    }

    // Check for return type annotation.
    while idx < end_idx {
      match self.tree.nodes[idx].token {
        Token::Arrow => {
          if idx + 1 < end_idx {
            idx += 1;

            if self.tree.nodes[idx].token == Token::FnType {
              let (ty, skip) = self.resolve_fn_type(idx);

              return_ty = ty;
              idx = skip;
            } else if self.tree.nodes[idx].token == Token::LBracket {
              if let Some((ty, next)) = self.resolve_array_type(idx, end_idx) {
                return_ty = ty;
                idx = next;
              }
            } else {
              return_ty = self.resolve_type_token(idx);
              idx += 1;
            }
          }

          break;
        }
        Token::LBrace | Token::FatArrow => break,
        _ => idx += 1,
      }
    }

    // -- 1b. Propagate types from declaration annotation ------
    // If the enclosing declaration has a Fn type annotation,
    // unify its param/return types with the closure's inferred
    // types BEFORE executing the body. This allows untyped
    // params like `fn(x)` to resolve via `Fn(int) -> int`.
    if let Some(ref decl) = self.pending_decl
      && let Some(ann_ty) = decl.annotated_ty
    {
      let ann = self.ty_checker.resolve_ty(ann_ty);

      if let Ty::Fun(fun_ty_id) = ann
        && let Some(fun_ty) = self.ty_checker.ty_table.fun(&fun_ty_id).copied()
      {
        let ann_params = self.ty_checker.ty_table.fun_params(&fun_ty).to_vec();

        // Unify each param type pairwise.
        for (i, (_, pty)) in params.iter_mut().enumerate() {
          if let Some(&ann_pty) = ann_params.get(i) {
            let span = self.tree.spans[start_idx];

            if let Some(unified) = self.ty_checker.unify(*pty, ann_pty, span) {
              *pty = unified;
            }
          }
        }

        // Propagate return type if closure has none.
        if return_ty == self.ty_checker.unit_type()
          && fun_ty.return_ty != self.ty_checker.unit_type()
        {
          return_ty = fun_ty.return_ty;
        }
      }
    }

    // -- 2. Determine body range -----------------------------

    let (body_start_idx, body_end_idx) =
      if idx < end_idx && self.tree.nodes[idx].token == Token::FatArrow {
        // Inline form: fn(x) => expr
        // Exclude trailing Semicolon — it belongs to the
        // enclosing declaration, not the closure body.
        let end = if end_idx > 0
          && self
            .tree
            .nodes
            .get(end_idx - 1)
            .is_some_and(|n| n.token == Token::Semicolon)
        {
          end_idx - 1
        } else {
          end_idx
        };

        (idx + 1, end)
      } else if idx < end_idx && self.tree.nodes[idx].token == Token::LBrace {
        // Block form: fn(x) { body }
        // Find matching RBrace within children.
        let brace_start = idx;
        let brace_header = self.tree.nodes[brace_start];

        let brace_children_end =
          (brace_header.child_start + brace_header.child_count) as usize;

        // Body is the block's children.
        // RBrace is at end_idx - 1 (sibling after block).
        (brace_start + 1, brace_children_end)
      } else {
        // Malformed closure.
        self.skip_until = end_idx;
        return;
      };

    // -- 3. Capture analysis ---------------------------------

    let captures =
      self.identify_captures(body_start_idx, body_end_idx, &params);

    // -- 4. Build combined params: captures + user params ----

    let capture_count = captures.len() as u32;
    let mut combined_params = Vec::with_capacity(captures.len() + params.len());

    for (name, ty_id, _is_mutable) in &captures {
      combined_params.push((*name, *ty_id));
    }

    combined_params.extend_from_slice(&params);

    // -- 5. Generate unique closure name ---------------------

    let closure_name = self
      .interner
      .intern(&format!("__closure_{}", self.closure_counter));

    self.closure_counter += 1;

    // -- 6. Save outer state ---------------------------------

    let outer_value_stack = std::mem::take(&mut self.value_stack);
    let outer_ty_stack = std::mem::take(&mut self.ty_stack);
    let outer_sir_values = std::mem::take(&mut self.sir_values);
    let outer_function = self.current_function.take();

    // -- 7. Emit FunDef into temporary SIR -------------------
    // Closure instructions are buffered and flushed after the
    // enclosing function's Return. This prevents DCE from
    // treating nested FunDefs as function boundaries.

    let mut closure_sir = Sir::new();
    closure_sir.next_value_id = self.sir.next_value_id;

    let outer_sir = std::mem::replace(&mut self.sir, closure_sir);

    let body_start = 1u32; // Body starts right after FunDef.
    let fundef_idx = 0;

    self.sir.emit(Insn::FunDef {
      name: closure_name,
      params: combined_params.clone(),
      return_ty,
      body_start,
      kind: FunctionKind::Closure { capture_count },
      pubness: Pubness::No,
    });

    // Register for call resolution.
    self.funs.push(FunDef {
      name: closure_name,
      params: combined_params.clone(),
      return_ty,
      body_start,
      kind: FunctionKind::Closure { capture_count },
      pubness: Pubness::No,
      type_params: Vec::new(),
      return_type_args: Vec::new(),
    });

    // Update pre-registered letrec local (if any) so
    // recursive calls inside the closure body can
    // resolve via resolve_closure_call.
    if let Some(decl) = &self.pending_decl {
      let decl_name = decl.name;

      if let Some(pos) = self.locals.iter().rposition(|l| l.name == decl_name) {
        let cv = self.values.store_closure(ClosureValue {
          fun_name: closure_name,
          captures: Vec::new(),
        });

        self.locals[pos].value_id = cv;
      }
    }

    // Save pending_decl so the closure body's semicolons
    // don't consume the outer imu declaration.
    let outer_pending_decl = self.pending_decl.take();

    // -- 8. Set function context + scope ---------------------

    self.current_function = Some(FunCtx {
      name: closure_name,
      return_ty,
      body_start,
      fundef_idx,
      has_explicit_return: false,
      has_return_type_annotation: return_ty != self.ty_checker.unit_type(),
      pending_return: false,
      scope_depth: self.scope_stack.len(),
    });

    // Param scope.
    self.push_scope();

    for (i, (pname, pty)) in combined_params.iter().enumerate() {
      // For captured closure variables, reuse the outer
      // ClosureValue so resolve_closure_call works inside
      // the closure body. Regular params get Value::Runtime.
      let value_id = if (i as u32) < capture_count {
        self
          .locals
          .iter()
          .rev()
          .find(|l| l.name == *pname)
          .filter(|l| {
            let vi = l.value_id.0 as usize;

            vi < self.values.kinds.len()
              && matches!(self.values.kinds[vi], Value::Closure)
          })
          .map(|l| l.value_id)
          .unwrap_or_else(|| self.values.store_runtime(i as u32))
      } else {
        self.values.store_runtime(i as u32)
      };

      // Captured mut variables retain their mutability
      // inside the closure so `count -= 1` works.
      let param_mutability = if (i as u32) < capture_count {
        captures
          .get(i)
          .filter(|(_, _, is_mut)| *is_mut)
          .map(|_| Mutability::Yes)
          .unwrap_or(Mutability::No)
      } else {
        Mutability::No
      };

      self.locals.push(Local {
        name: *pname,
        ty_id: *pty,
        value_id,
        pubness: Pubness::No,
        mutability: param_mutability,
        sir_value: None,
        local_kind: LocalKind::Parameter,
      });

      if let Some(frame) = self.scope_stack.last_mut() {
        frame.count += 1;
      }
    }

    // Body scope (maintains scope_depth invariant).
    self.push_scope();

    // -- 9. Execute body nodes -------------------------------

    let saved_skip = self.skip_until;

    self.skip_until = 0;

    for i in body_start_idx..body_end_idx {
      if i < self.skip_until {
        continue;
      }

      let node = self.tree.nodes[i];

      self.execute_node(&node, i);
    }

    // -- 9b. Finalize pending operations ----------------------
    // Inline closures (`fn() => expr`) have no Semicolon to
    // trigger compound assignment finalization. Do it here
    // so `fn() => count += 1` emits the BinOp + Store.

    self.apply_deferred_binop();

    // Compound/regular assignments are statements — they
    // don't produce a return value. Track whether one was
    // finalized so the implicit return emits unit.
    let had_compound = self.pending_compound.is_some();
    let had_assign = self.pending_assign.is_some();

    self.finalize_pending_compound();
    self.finalize_pending_assign();

    // -- 10. Emit implicit return ----------------------------

    let has_explicit = self
      .current_function
      .as_ref()
      .is_some_and(|c| c.has_explicit_return);

    if !has_explicit {
      // Assignments are statements — return unit, not
      // the assignment result.
      let return_value = if had_compound || had_assign {
        None
      } else {
        self.sir_values.last().copied().filter(|v| v.0 != u32::MAX)
      };

      let return_ty_actual = if had_compound || had_assign {
        return_ty
      } else {
        self.ty_stack.last().copied().unwrap_or(return_ty)
      };

      self.sir.emit(Insn::Return {
        value: return_value,
        ty_id: return_ty_actual,
      });
    }

    // -- 11. Tear down ---------------------------------------

    self.pop_scope(); // Body scope.
    self.pop_scope(); // Param scope.

    // Move closure SIR to deferred buffer + restore outer SIR.
    let closure_sir = std::mem::replace(&mut self.sir, outer_sir);

    self.sir.next_value_id = closure_sir.next_value_id;
    self.deferred_closures.extend(closure_sir.instructions);

    self.current_function = outer_function;
    self.skip_until = saved_skip;
    self.pending_decl = outer_pending_decl;

    // Restore outer stacks.
    self.value_stack = outer_value_stack;
    self.ty_stack = outer_ty_stack;
    self.sir_values = outer_sir_values;

    // -- 12. Push closure value onto outer stack -------------

    // Build Ty::Fun for the user-visible params (not captures).
    let user_param_tys = params.iter().map(|(_, ty)| *ty).collect::<Vec<_>>();

    let fun_ty_id = self
      .ty_checker
      .ty_table
      .intern_fun(user_param_tys, return_ty);

    let closure_ty = self.ty_checker.intern_ty(Ty::Fun(fun_ty_id));

    // Collect capture SIR values for prepending at call sites.
    // Use the outer scope's SIR values (by-copy semantics:
    // the value is fixed at closure creation time).
    let capture_infos = captures
      .iter()
      .map(|(name, _, is_mutable)| {
        let sir_val = self
          .locals
          .iter()
          .rev()
          .find(|l| l.name == *name)
          .and_then(|l| l.sir_value)
          .unwrap_or(ValueId(u32::MAX));

        CaptureInfo {
          name: *name,
          sir_value: sir_val,
          is_mutable: *is_mutable,
        }
      })
      .collect::<Vec<_>>();

    let closure_val = self.values.store_closure(ClosureValue {
      fun_name: closure_name,
      captures: capture_infos,
    });

    self.value_stack.push(closure_val);
    self.ty_stack.push(closure_ty);
    self.sir_values.push(ValueId(u32::MAX));

    // Skip past the closure tokens in the main loop,
    // but not the trailing Semicolon — it belongs to the
    // enclosing `imu`/`mut` declaration.
    let skip_end = if end_idx > 0
      && self
        .tree
        .nodes
        .get(end_idx - 1)
        .is_some_and(|n| n.token == Token::Semicolon)
    {
      end_idx - 1
    } else {
      end_idx
    };

    self.skip_until = skip_end;
  }

  /// Finds the span of the return type token after `->`.
  fn find_return_type_span(&self, start: usize, end: usize) -> Option<Span> {
    let mut found_arrow = false;

    for i in start..end {
      let tok = self.tree.nodes[i].token;

      if tok == Token::Arrow {
        found_arrow = true;
      } else if found_arrow && (tok.is_ty() || tok == Token::Ident) {
        return Some(self.tree.spans[i]);
      }
    }

    None
  }

  fn execute_fun(&mut self, start_idx: usize, _end_idx: usize) {
    // Parse the function signature and set it as pending
    // The actual FunDef will be emitted when we hit LBrace

    let name = self
      .tree
      .nodes
      .get(start_idx + 1)
      .filter(|node| matches!(node.token, Token::Ident))
      .and_then(|_| self.node_value(start_idx + 1))
      .and_then(|val| match val {
        NodeValue::Symbol(sym) => Some(sym),
        _ => None,
      });

    if name.is_none() {
      return;
    }

    // Mangle name in apply / pack context:
    //   `apply Type { fun m }`         → `Type::m`
    //   `pack p { fun h }`             → `p::h`
    //   `pack p { pack q { fun h } }`  → `p::q::h`
    //   `apply T { fun m }` inside a pack → `p::T::m`
    let raw_name = name.unwrap();
    let name = if self.apply_context.is_some() || !self.pack_context.is_empty()
    {
      let mut parts: Vec<String> = self
        .pack_context
        .iter()
        .map(|(s, _)| self.interner.get(*s).to_owned())
        .collect();

      if let Some(type_name) = self.apply_context {
        parts.push(self.interner.get(type_name).to_owned());
      }

      parts.push(self.interner.get(raw_name).to_owned());

      self.interner.intern(&parts.join("::"))
    } else {
      raw_name
    };

    // Parse parameters: (name, type, mutability).
    let mut params: Vec<(Symbol, TyId, Mutability)> = Vec::new();
    let mut return_ty = self.ty_checker.unit_type();
    let mut return_type_args: Vec<TyId> = Vec::new();
    let mut idx = start_idx + 2; // Skip Fun and name

    // Parse optional type parameters: <$T, $A>.
    // Creates fresh inference vars for each.
    // Preserve apply-level type params so $T from
    // `apply Pair<$T>` is available inside methods.
    let outer_type_params = std::mem::take(&mut self.type_params);

    if idx < _end_idx && self.tree.nodes[idx].token == Token::LAngle {
      idx += 1; // skip <

      while idx < _end_idx {
        let tok = self.tree.nodes[idx].token;

        if tok == Token::RAngle {
          idx += 1;

          break;
        }

        if tok == Token::Dollar && idx + 1 < _end_idx {
          idx += 1; // skip $

          if let Some(NodeValue::Symbol(sym)) = self.node_value(idx) {
            let var = self.ty_checker.fresh_var();

            self.type_params.push((sym, var));

            // Check for constraint: $T: Abstract.
            if idx + 2 < _end_idx
              && self.tree.nodes[idx + 1].token == Token::Colon
              && self.tree.nodes[idx + 2].token == Token::Ident
            {
              if let Some(NodeValue::Symbol(abs)) = self.node_value(idx + 2) {
                self.type_constraints.insert(sym, abs);
              }

              idx += 2; // skip : and Ident
            }
          }
        }

        idx += 1;
      }
    }

    // Remember whether THIS function declared its own
    // `$T` — distinct from apply-level / outer-scope leaks
    // (e.g. a generic struct earlier in the source pushes
    // its `$T` onto `self.type_params` and `main`'s outer
    // scope still sees them). Only a function that owns
    // type parameters counts as "generic" for the
    // body-skip check below.
    let fn_had_type_params = !self.type_params.is_empty();

    // If no function-level type params, restore
    // apply-level params (e.g., $T from apply Pair<$T>).
    if self.type_params.is_empty() {
      self.type_params = outer_type_params;
    }

    // Skip past LParen
    if idx < _end_idx && self.tree.nodes[idx].token == Token::LParen {
      idx += 1;

      // Parse parameters until we hit RParen
      while idx < _end_idx {
        // Check for `mut` modifier before the param name.
        let is_mut = self.tree.nodes[idx].token == Token::Mut;

        if is_mut {
          idx += 1;
        }

        let token = self.tree.nodes[idx].token;

        match token {
          Token::RParen => {
            idx += 1;

            break;
          }
          Token::SelfLower => {
            // `self` param in apply context — type is
            // the applied type. `resolve_ty_symbol`
            // (not `resolve_ty_name`) so primitive applies
            // (`apply char { fun … (self) … }`) resolve
            // `self: char` via the interner-string table;
            // structs/enums fall through to `resolve_ty_name`
            // internally.
            if let Some(type_name) = self.apply_context {
              let self_sym = zo_interner::Symbol::SELF_LOWER;
              let self_ty = self
                .resolve_apply_self_ty(type_name)
                .unwrap_or_else(|| self.ty_checker.unit_type());

              let mutability = if is_mut {
                Mutability::Yes
              } else {
                Mutability::No
              };

              params.push((self_sym, self_ty, mutability));
            }

            idx += 1;

            if idx < _end_idx && self.tree.nodes[idx].token == Token::Comma {
              idx += 1;
            }
          }
          Token::Ident => {
            // Get parameter name
            if let Some(NodeValue::Symbol(param_name)) = self.node_value(idx) {
              idx += 1;

              // Next should be the type (no colon token).
              // For `$T`, skip Dollar + Ident (2 tokens).
              // For `[]type`, skip LBracket + RBracket + type.
              if idx < _end_idx {
                let param_ty = if self.tree.nodes[idx].token == Token::LBracket
                {
                  if let Some((ty, next)) =
                    self.resolve_array_type(idx, _end_idx)
                  {
                    idx = next - 1;
                    ty
                  } else {
                    self.ty_checker.int_type()
                  }
                } else if self.tree.nodes[idx].token == Token::FnType {
                  let (ty, skip) = self.resolve_fn_type(idx);

                  idx = skip - 1;

                  ty
                } else if self.tree.nodes[idx].token == Token::LParen {
                  // Tuple param type: `fun f(t: (int, int))`.
                  // Without this branch, `resolve_type_token`
                  // saw the `(` as non-type and returned unit —
                  // tuple-index access on the param then landed
                  // in the `_ => unit` fallthrough of the field-
                  // access dispatcher and emitted TypeMismatch.
                  let (ty, skip) = self.resolve_tuple_type(idx);

                  idx = skip - 1;

                  ty
                } else {
                  self.resolve_type_token(idx)
                };

                // Skip extra token for $T type params.
                if self.tree.nodes[idx].token == Token::Dollar {
                  idx += 1; // skip Dollar
                }

                let mutability = if is_mut {
                  Mutability::Yes
                } else {
                  Mutability::No
                };

                params.push((param_name, param_ty, mutability));

                idx += 1;

                // Skip comma if present
                if idx < _end_idx && self.tree.nodes[idx].token == Token::Comma
                {
                  idx += 1;
                }
              }
            } else {
              idx += 1;
            }
          }
          _ => idx += 1,
        }
      }
    }

    // Look for return type
    while idx < _end_idx {
      match self.tree.nodes[idx].token {
        Token::Arrow => {
          if idx + 1 < _end_idx {
            idx += 1;

            if self.tree.nodes[idx].token == Token::LBracket {
              if let Some((ty, _next)) = self.resolve_array_type(idx, _end_idx)
              {
                return_ty = ty;
              }
            } else {
              return_ty = self.resolve_type_token(idx);
              idx += 1;

              // Collect generic type arguments after the
              // return type name (e.g. `-> Result<str, int>`).
              while idx < _end_idx {
                let tok = self.tree.nodes[idx].token;

                if tok.is_ty() || tok == Token::Ident {
                  return_type_args.push(self.resolve_type_token(idx));
                  idx += 1;
                } else if matches!(tok, Token::Lt | Token::Gt | Token::Comma) {
                  idx += 1;
                } else {
                  break;
                }
              }
            }
          }

          break;
        }
        Token::LBrace => break,
        Token::Colon => {
          // `:` after `)` is wrong — user meant `->`.
          let span = self.tree.spans[idx];

          report_error(Error::new(ErrorKind::ExpectedArrow, span));

          // Recover: treat as `->` so codegen proceeds.
          if idx + 1 < _end_idx {
            idx += 1;
            return_ty = self.resolve_type_token(idx);
          }

          break;
        }
        _ => idx += 1,
      }
    }

    // Skip signature tokens in the main loop — they've
    // been consumed above.  The LBrace must still be
    // processed (it triggers function body entry).
    let lbrace_idx = (start_idx + 1.._end_idx)
      .find(|&i| self.tree.nodes[i].token == Token::LBrace)
      .unwrap_or(_end_idx);

    self.skip_until = lbrace_idx;

    // Set the function as pending - it will be processed when we hit LBrace
    let pubness = if self.is_pub(start_idx) {
      Pubness::Yes
    } else {
      Pubness::No
    };

    // Track if user explicitly wrote `-> Type`.
    let unit_ty = self.ty_checker.unit_type();
    self.pending_fn_has_return_annotation = return_ty != unit_ty;

    // FunDef stores (name, ty) — strip mutability.
    let sir_params =
      params.iter().map(|(n, t, _)| (*n, *t)).collect::<Vec<_>>();

    // Signature-only pre-scan path. Registers the FunDef so
    // forward references (mutual recursion, out-of-order
    // calls) resolve correctly during the main pass, then
    // returns before any body-level state is touched. The
    // main pass replays `execute_fun` for the same node and
    // `push_or_replace_fun` upserts the entry with real
    // `body_start`. Diagnostics (e.g. `main() must return
    // unit`) are left to the main pass so they fire exactly
    // once.
    if self.prescan_only {
      self.push_or_replace_fun(FunDef {
        name,
        params: sir_params,
        return_ty,
        body_start: 0,
        kind: FunctionKind::UserDefined,
        pubness,
        type_params: self.type_params.iter().map(|(_, ty)| *ty).collect(),
        return_type_args: return_type_args
          .iter()
          .map(|t| self.ty_checker.resolve_ty(*t))
          .collect(),
      });

      // Drop any type_params minted during this signature
      // parse — the real main-pass `execute_fun` will re-mint
      // them. Leaving them in would leak `$T` into unrelated
      // top-level items processed by the prescan walker.
      if fn_had_type_params {
        self.type_params.clear();
      }

      return;
    }

    // main() must return unit — no other return type.
    if self.interner.get(name) == "main" && return_ty != unit_ty {
      // Point the span at the return type token (after ->).
      let span = self
        .find_return_type_span(start_idx, _end_idx)
        .unwrap_or(self.tree.spans[start_idx]);

      report_error(Error::new(ErrorKind::InvalidReturnType, span));

      return_ty = unit_ty;
    }

    let is_generic_outer_pass =
      fn_had_type_params && self.mono_name_override.is_none();

    // Compute the tree-index boundary that sits one past
    // this function's matching `}` — i.e. the first index
    // the main loop should resume at after skipping the
    // whole function. The caller's `children_end` is the
    // end of the Fun node's child span and on some parser
    // paths that span reaches far beyond the function's
    // own block (e.g. top-level siblings share a child
    // range), so we explicitly walk from the body's
    // `LBrace` with a depth counter to find the matching
    // `RBrace`. This boundary is used both for the
    // generic-outer-pass skip AND as the tree range
    // recorded for the re-execution pass.
    let end_of_block = {
      let lbrace_idx = (start_idx + 1.._end_idx)
        .find(|&i| self.tree.nodes[i].token == Token::LBrace);

      if let Some(lb) = lbrace_idx {
        let mut depth = 1i32;
        let mut j = lb + 1;

        while j < self.tree.nodes.len() && depth > 0 {
          match self.tree.nodes[j].token {
            Token::LBrace => depth += 1,
            Token::RBrace => depth -= 1,
            _ => {}
          }

          if depth == 0 {
            break;
          }

          j += 1;
        }

        // `j` is the matching RBrace; advance past it so
        // the main loop resumes at the next sibling.
        j + 1
      } else {
        _end_idx
      }
    };

    // Record the tree range of every user function so the
    // instantiation pass can re-execute the body per
    // substitution — generic-type mono needs this for `$T`
    // resolution, closure-param mono needs it for binding
    // Fn-typed params to concrete closures. Keyed by the
    // resolved (apply-mangled) name; skipped when the
    // re-execution pass itself is running (mono override
    // points at the mangled symbol for the instantiation
    // we're emitting).
    if self.mono_name_override.is_none() {
      self
        .generic_tree_ranges
        .insert(name, (start_idx as u32, end_of_block as u32));
    }

    // Skip body execution for a generic's outer pass. The
    // generic has no callers by its original name — every
    // call site routes through a mangled instantiation — so
    // emitting the generic body's SIR is redundant. Register
    // the FunDef in `self.funs` so call resolution can
    // enumerate `type_params` to build substitutions, then
    // skip past the closing `}` so the outer loop never
    // enters the body. The re-execution pass replays the
    // tree range per instantiation to produce the real
    // concrete bodies.
    if is_generic_outer_pass {
      self.push_or_replace_fun(FunDef {
        name,
        params: sir_params,
        return_ty,
        body_start: 0,
        kind: FunctionKind::UserDefined,
        pubness,
        type_params: self.type_params.iter().map(|(_, ty)| *ty).collect(),
        return_type_args: return_type_args
          .iter()
          .map(|t| self.ty_checker.resolve_ty(*t))
          .collect(),
      });

      // Restore outer type_params scope (signature parse
      // mutated it; the generic's type_params are scoped
      // to each re-execution, not to the outer pass).
      self.type_params.clear();
      self.skip_until = end_of_block;

      return;
    }

    // When the instantiation pass re-executes a generic, it
    // sets `mono_name_override` before replaying the Fun
    // node; use that instead of the tree's literal name so
    // the emitted FunDef carries the mangled symbol
    // (e.g. `are_equal__Point`).
    let name = self.mono_name_override.take().unwrap_or(name);

    self.pending_function = Some(FunDef {
      name,
      params: sir_params,
      return_ty,
      body_start: 0,
      kind: FunctionKind::UserDefined,
      pubness,
      type_params: self.type_params.iter().map(|(_, ty)| *ty).collect(),
      return_type_args: return_type_args
        .iter()
        .map(|t| self.ty_checker.resolve_ty(*t))
        .collect(),
    });

    // Push a scope for the function parameters
    self.push_scope();

    // Add parameters as local variables.
    for (i, (param_name, param_ty, mutability)) in params.iter().enumerate() {
      let value_id = self.values.store_runtime(i as u32);

      self.locals.push(Local {
        name: *param_name,
        ty_id: *param_ty,
        value_id,
        pubness: Pubness::No,
        mutability: *mutability,
        sir_value: None,
        local_kind: LocalKind::Parameter,
      });

      if let Some(frame) = self.scope_stack.last_mut() {
        frame.count += 1;
      }
    }
  }

  /// Begin a variable declaration (Imu/Val/Mut).
  ///
  /// Instead of processing children immediately, we defer to
  /// [`finalize_pending_decl`] at the Semicolon. This lets
  /// the main loop process children (especially the init
  /// expression) so the init value is on the stacks.
  fn begin_decl(
    &mut self,
    idx: usize,
    header: &NodeHeader,
    is_mutable: bool,
    is_constant: bool,
  ) {
    let children_end = (header.child_start + header.child_count) as usize;

    // Check if this is a template assignment.
    let has_template = ((idx + 1)..children_end)
      .any(|i| matches!(self.tree.nodes[i].token, Token::TemplateAssign));

    if has_template {
      // Template declarations still use the old path.
      if is_mutable {
        self.execute_mut(idx, children_end);
      } else {
        self.execute_imu(idx, children_end);
      }
      self.skip_until = children_end;
      return;
    }

    // Extract variable name from tree (first Ident child).
    let name = self
      .tree
      .nodes
      .get(idx + 1)
      .filter(|n| matches!(n.token, Token::Ident))
      .and_then(|_| self.node_value(idx + 1))
      .and_then(|val| match val {
        NodeValue::Symbol(sym) => Some(sym),
        _ => None,
      });

    if let Some(name) = name {
      let pubness = if self.is_pub(idx) {
        Pubness::Yes
      } else {
        Pubness::No
      };

      // Parse optional type annotation between name
      // and = / :=. Scan: Ident, [Colon, Type], Eq.
      let mut annotated_ty = None;
      let mut skip_to = idx + 2; // skip Imu + name

      let mut has_colon = false;

      let mut i = idx + 2;

      while i < children_end {
        let tok = self.tree.nodes[i].token;

        if tok == Token::Colon {
          has_colon = true;
        }

        if tok == Token::ColonEq {
          // val forbids `:=` — requires explicit type.
          if is_constant {
            let span = self.tree.spans[i];

            report_error(Error::new(
              ErrorKind::ValRequiresTypeAnnotation,
              span,
            ));

            self.skip_until = children_end;

            return;
          }

          skip_to = i + 1;

          break;
        }

        if tok == Token::Eq {
          // `=` requires a type annotation (`: Type =`).
          // Without `:`, use `:=` for inference.
          if !has_colon && annotated_ty.is_none() {
            let span = self.tree.spans[i];

            report_error(Error::new(ErrorKind::ExpectedTypeAnnotation, span));
          }

          skip_to = i + 1;

          break;
        }

        // Tuple type annotation: (int, float, str).
        if tok == Token::LParen && annotated_ty.is_none() {
          let (ty_id, skip) = self.resolve_tuple_type(i);
          annotated_ty = Some(ty_id);
          i = skip;

          continue;
        }

        // Array type annotation: []type, [N]type, [N][M]type.
        if tok == Token::LBracket
          && annotated_ty.is_none()
          && let Some((ty, next)) = self.resolve_array_type(i, children_end)
        {
          annotated_ty = Some(ty);
          i = next;
          skip_to = i;
          continue;
        }

        // Function type annotation: Fn(T1, T2) -> R.
        if tok == Token::FnType && annotated_ty.is_none() {
          let (ty_id, skip) = self.resolve_fn_type(i);
          annotated_ty = Some(ty_id);
          i = skip;

          continue;
        }

        // Type token after the colon.
        if tok.is_ty() && annotated_ty.is_none() {
          annotated_ty = Some(self.resolve_type_token(i));
        }

        // Struct/enum name as type annotation.
        if tok == Token::Ident
          && annotated_ty.is_none()
          && let Some(NodeValue::Symbol(sym)) = self.node_value(i)
        {
          annotated_ty = self.ty_checker.resolve_ty_name(sym);
        }

        skip_to = i + 1;
        i += 1;
      }

      self.pending_decl = Some(PendingDecl {
        name,
        is_mutable,
        is_constant,
        pubness,
        annotated_ty,
        span: self.tree.spans[idx],
      });

      // Pre-register for recursive closures (letrec).
      // If the init expression is a closure, the body
      // may reference the variable by name. Register a
      // placeholder local so lookup_local succeeds
      // during closure body execution.
      let has_closure =
        (skip_to..children_end).any(|i| self.tree.nodes[i].token == Token::Fn);

      if has_closure {
        let placeholder = self.values.store_runtime(u32::MAX);

        let ty = self.ty_checker.fresh_var();

        self.locals.push(Local {
          name,
          ty_id: ty,
          value_id: placeholder,
          pubness,
          mutability: if is_mutable {
            Mutability::Yes
          } else {
            Mutability::No
          },
          sir_value: Some(ValueId(u32::MAX)),
          local_kind: LocalKind::Variable,
        });

        if let Some(frame) = self.scope_stack.last_mut() {
          frame.count += 1;
        }
      }

      self.skip_until = skip_to;
    }
  }

  /// Finalize a pending array element assignment (arr[i] = value;).
  fn finalize_pending_array_assign(&mut self) {
    let (array_sir, index_sir, array_name, span) =
      match self.pending_array_assign.take() {
        Some(a) => a,
        None => return,
      };

    // Pop the RHS value.
    if let (Some(_value), Some(value_ty)) =
      (self.value_stack.pop(), self.ty_stack.pop())
    {
      let value_sir = self.sir_values.pop();

      // Check mutability.
      let is_mutable = self
        .locals
        .iter()
        .rev()
        .find(|l| l.name == array_name)
        .is_some_and(|l| l.mutability == Mutability::Yes);

      if !is_mutable {
        report_error(Error::new(ErrorKind::ImmutableVariable, span));

        return;
      }

      if let Some(sv) = value_sir {
        self.sir.emit(Insn::ArrayStore {
          array: array_sir,
          index: index_sir,
          value: sv,
          ty_id: value_ty,
        });
      }
    }
  }

  /// Finalize a pending variable declaration.
  ///
  /// Finalize a pending assignment (x = expr;).
  fn finalize_pending_assign(&mut self) {
    let (name, span) = match self.pending_assign.take() {
      Some(ns) => ns,
      None => return,
    };

    if let (Some(value), Some(value_ty)) =
      (self.value_stack.pop(), self.ty_stack.pop())
    {
      let value_sir = self.sir_values.pop();

      if let Some(local) = self.locals.iter_mut().rev().find(|l| l.name == name)
      {
        if local.mutability != Mutability::Yes {
          report_error(Error::new(ErrorKind::ImmutableVariable, span));

          return;
        }

        if let Some(unified_ty) =
          self.ty_checker.unify(local.ty_id, value_ty, span)
        {
          local.value_id = value;
          local.sir_value = value_sir;

          if let Some(sv) = value_sir {
            self.sir.emit(Insn::Store {
              name,
              value: sv,
              ty_id: unified_ty,
            });
          }
        }
      }
    }
  }

  /// Constant-fold a string slice expression `s[lo..hi]`
  /// (or `s[lo..=hi]`). Runtime slicing is not supported in
  /// v1 — `str` has no view layout and no heap allocator, so
  /// a runtime slice cannot be materialized. Both bounds and
  /// the receiver must be compile-time constants.
  ///
  /// Called from the `Token::RBracket` handler when the node
  /// immediately preceding the bracket is `DotDot` /
  /// `DotDotEq`. Expects three entries on the stacks:
  /// `[receiver_str, lo_int, hi_int]` (in push order). Pops
  /// all three, validates, and pushes a fresh
  /// `Insn::ConstString` with the sliced bytes.
  fn execute_str_slice_const(&mut self, r_bracket_idx: usize, _depth: usize) {
    let span = self.tree.spans[r_bracket_idx];
    let inclusive = r_bracket_idx > 0
      && self.tree.nodes[r_bracket_idx - 1].token == Token::DotDotEq;

    let (hi_vid, _hi_ty, _hi_sir) = match (
      self.value_stack.pop(),
      self.ty_stack.pop(),
      self.sir_values.pop(),
    ) {
      (Some(v), Some(t), Some(s)) => (v, t, s),
      _ => {
        report_error(Error::new(ErrorKind::StrSliceRequiresConstBounds, span));

        return;
      }
    };

    let (lo_vid, _lo_ty, _lo_sir) = match (
      self.value_stack.pop(),
      self.ty_stack.pop(),
      self.sir_values.pop(),
    ) {
      (Some(v), Some(t), Some(s)) => (v, t, s),
      _ => {
        report_error(Error::new(ErrorKind::StrSliceRequiresConstBounds, span));

        return;
      }
    };

    let (_recv_vid, _recv_ty, recv_sir) = match (
      self.value_stack.pop(),
      self.ty_stack.pop(),
      self.sir_values.pop(),
    ) {
      (Some(v), Some(t), Some(s)) => (v, t, s),
      _ => {
        report_error(Error::new(ErrorKind::StrSliceRequiresStr, span));

        return;
      }
    };

    // Receiver must resolve to a compile-time string. Ident
    // references are lowered to `Insn::Load { dst, src:
    // Local(name), .. }` by the expression path, so the
    // receiver's ValueId on the value stack is a fresh
    // runtime id — not the original string value. Trace back
    // through the Load to find the local, then check its
    // underlying `Value::String`.
    let recv_sym = match self.resolve_const_str_sym(recv_sir) {
      Some(sym) => sym,
      None => {
        report_error(Error::new(ErrorKind::StrSliceRequiresStr, span));

        return;
      }
    };

    // Both bounds must resolve to compile-time ints.
    let lo = match self.value_as_const_int(lo_vid) {
      Some(v) => v,
      None => {
        report_error(Error::new(ErrorKind::StrSliceRequiresConstBounds, span));

        return;
      }
    };

    let hi_raw = match self.value_as_const_int(hi_vid) {
      Some(v) => v,
      None => {
        report_error(Error::new(ErrorKind::StrSliceRequiresConstBounds, span));

        return;
      }
    };

    let hi = if inclusive {
      hi_raw.saturating_add(1)
    } else {
      hi_raw
    };

    if lo > hi {
      report_error(Error::new(ErrorKind::StrSliceInvalidRange, span));

      return;
    }

    let src = self.interner.get(recv_sym).to_owned();
    let src_bytes = src.as_bytes();

    if (hi as usize) > src_bytes.len() {
      report_error(Error::new(ErrorKind::StrSliceOutOfBounds, span));

      return;
    }

    let slice_bytes = &src_bytes[lo as usize..hi as usize];
    let slice_str = match std::str::from_utf8(slice_bytes) {
      Ok(s) => s.to_owned(),
      Err(_) => {
        // Slice doesn't land on UTF-8 boundary — invalid.
        report_error(Error::new(ErrorKind::StrSliceOutOfBounds, span));

        return;
      }
    };

    let slice_sym = self.interner.intern(&slice_str);
    let str_ty = self.ty_checker.str_type();
    let dst = ValueId(self.sir.next_value_id);

    self.sir.next_value_id += 1;

    let sir_value = self.sir.emit(Insn::ConstString {
      dst,
      symbol: slice_sym,
      ty_id: str_ty,
    });

    let value_id = self.values.store_string(slice_sym);

    self.value_stack.push(value_id);
    self.ty_stack.push(str_ty);
    self.sir_values.push(sir_value);
  }

  /// Trace a SIR `ValueId` back to its compile-time string
  /// symbol, if it was lowered from a `Value::String` local
  /// via `Insn::Load { src: Local(..), .. }` or emitted
  /// directly as `Insn::ConstString`.
  ///
  /// Used by `execute_str_slice_const` — an Ident reference
  /// to a `str` local pushes a fresh runtime `ValueId` onto
  /// the value stack, but the SIR `Load` preserves the link
  /// to the source local. A chained slice
  /// (`s[0..10][2..5]`) hits the `ConstString` path directly.
  fn resolve_const_str_sym(&self, sir_vid: ValueId) -> Option<Symbol> {
    for insn in &self.sir.instructions {
      match insn {
        Insn::ConstString { dst, symbol, .. } if *dst == sir_vid => {
          return Some(*symbol);
        }
        Insn::Load {
          dst,
          src: LoadSource::Local(sym),
          ..
        } if *dst == sir_vid => {
          let local = self.locals.iter().rev().find(|l| l.name == *sym)?;
          let lvi = local.value_id.0 as usize;

          if lvi < self.values.kinds.len()
            && matches!(self.values.kinds[lvi], Value::String)
          {
            let si = self.values.indices[lvi] as usize;

            return Some(self.values.strings[si]);
          }

          return None;
        }
        _ => {}
      }
    }

    None
  }

  /// Resolves a `ValueId` to its compile-time integer value
  /// when the associated `Value` is a `Value::Int`. Used by
  /// compile-time-only paths that need constant bounds
  /// (e.g. `execute_str_slice_const`).
  fn value_as_const_int(&self, vid: ValueId) -> Option<u64> {
    let vi = vid.0 as usize;

    if vi < self.values.kinds.len()
      && matches!(self.values.kinds[vi], Value::Int)
    {
      let ii = self.values.indices[vi] as usize;

      Some(self.values.ints[ii])
    } else {
      None
    }
  }

  /// Narrow a default-typed integer literal's SIR `ConstInt`
  /// type to `target_ty`.
  ///
  /// Integer literals are parsed with the default integer type
  /// (`int` = s32). When used in a context that expects a
  /// different concrete integer type — a type annotation like
  /// `imu x: uint = 10` or a comparison like `check@eq(x, 10)`
  /// where `x: u64` — the literal's `ConstInt.ty_id` is
  /// rewritten to `target_ty` so unification succeeds. This
  /// matches Rust's polymorphic integer literal behavior while
  /// keeping all type flow explicit and local.
  ///
  /// Returns `true` when the rewrite happened.
  ///
  /// Scope: direct literals only. Expressions like `10 + 5`
  /// produce a `BinOp` whose result is s32; retyping those is
  /// a broader change (polymorphic literal propagation) and is
  /// not attempted here.
  fn narrow_int_literal(
    &mut self,
    sir_val: ValueId,
    src_ty: TyId,
    target_ty: TyId,
  ) -> bool {
    let default_int_ty = self.ty_checker.int_type();

    if src_ty != default_int_ty || target_ty == default_int_ty {
      return false;
    }

    if !matches!(self.ty_checker.kind_of(target_ty), Ty::Int { .. }) {
      return false;
    }

    if let Some(insn) = self
      .sir
      .instructions
      .iter_mut()
      .rev()
      .find(|i| matches!(i, Insn::ConstInt { dst, .. } if *dst == sir_val))
      && let Insn::ConstInt { ty_id: cty, .. } = insn
    {
      *cty = target_ty;

      return true;
    }

    false
  }

  /// Called at Semicolon after the init expression has been
  /// evaluated and its value is on the stacks.
  fn finalize_pending_decl(&mut self) {
    let decl = match self.pending_decl.take() {
      Some(d) => d,
      None => return,
    };

    if let (Some(init_value), Some(mut init_ty)) =
      (self.value_stack.pop(), self.ty_stack.pop())
    {
      let sir_init = self.sir_values.pop();

      // Narrow a default-typed int literal to the annotation.
      if let (Some(ann_ty), Some(sv)) = (decl.annotated_ty, sir_init)
        && self.narrow_int_literal(sv, init_ty, ann_ty)
      {
        init_ty = ann_ty;
      }

      // Unify annotated type with init type. For enum values,
      // the annotation `Option<int>` can't be fully parsed yet
      // (generic type annotations are a follow-up), so skip
      // unification when the init type is an enum and the
      // annotation refers to a non-enum type. The init type is
      // the ground truth in that case.
      let ty_id = if let Some(ann_ty) = decl.annotated_ty {
        let init_is_enum = self.enum_defs.iter().any(|e| e.2 == init_ty);

        if init_is_enum {
          init_ty
        } else {
          self
            .ty_checker
            .unify(ann_ty, init_ty, decl.span)
            .unwrap_or(init_ty)
        }
      } else {
        init_ty
      };

      if decl.is_constant {
        // --- val path: compile-time constant ---
        // Validate: init must be a compile-time value.
        let vi = init_value.0 as usize;

        let is_const = vi < self.values.kinds.len()
          && matches!(
            self.values.kinds[vi],
            Value::Int
              | Value::Float
              | Value::Bool
              | Value::String
              | Value::Char
          );

        if !is_const {
          report_error(Error::new(
            ErrorKind::ValRequiresConstantInit,
            decl.span,
          ));

          return;
        }

        let constant_local = Local {
          name: decl.name,
          ty_id,
          value_id: init_value,
          pubness: decl.pubness,
          mutability: Mutability::No,
          sir_value: sir_init,
          local_kind: LocalKind::Constant,
        };

        if self.current_function.is_none() {
          // Module-level val — strip the ConstInt that the
          // main loop emitted for the init expression. It
          // would shift ValueId numbering after DCE.
          // Don't emit ConstDef either — the constant is
          // fully resolved at the executor level.
          if let Some(
            Insn::ConstInt { .. }
            | Insn::ConstFloat { .. }
            | Insn::ConstBool { .. }
            | Insn::ConstString { .. },
          ) = self.sir.instructions.last()
          {
            self.sir.instructions.pop();
            // Undo the auto-increment from sir.emit()
            // so inline re-emissions get correct ValueIds.
            if self.sir.next_value_id > 0 {
              self.sir.next_value_id -= 1;
            }
          }

          self.global_constants.push(constant_local);
        } else {
          // Function-local val — emit ConstDef as
          // metadata and push to locals for inline
          // re-emission.
          self.sir.emit(Insn::ConstDef {
            name: decl.name,
            ty_id,
            value: sir_init.unwrap_or(ValueId(u32::MAX)),
            pubness: decl.pubness,
          });

          self.locals.push(constant_local);

          if let Some(frame) = self.scope_stack.last_mut() {
            frame.count += 1;
          }
        }

        return;
      }

      // --- imu/mut path ---
      let mutability = if decl.is_mutable {
        Mutability::Yes
      } else {
        Mutability::No
      };

      let _sir_value = self.sir.emit(Insn::VarDef {
        name: decl.name,
        ty_id,
        init: sir_init,
        mutability,
        pubness: decl.pubness,
      });

      // Update pre-registered local (letrec) or push new.
      if let Some(local) =
        self.locals.iter_mut().rev().find(|l| l.name == decl.name)
      {
        local.ty_id = ty_id;
        local.value_id = init_value;
        local.sir_value = sir_init;
      } else {
        self.locals.push(Local {
          name: decl.name,
          ty_id,
          value_id: init_value,
          pubness: decl.pubness,
          mutability,
          sir_value: sir_init,
          local_kind: LocalKind::Variable,
        });

        if let Some(frame) = self.scope_stack.last_mut() {
          frame.count += 1;
        }
      }

      // Emit initial Store so the value is on the stack
      // frame. Load instructions will read from this
      // slot.
      if self.current_function.is_some()
        && let Some(sv) = sir_init
      {
        self.sir.emit(Insn::Store {
          name: decl.name,
          value: sv,
          ty_id,
        });
      }
    }
  }

  /// Executes immutable declaration (legacy path for
  /// template assignments).
  fn execute_imu(&mut self, start_idx: usize, end_idx: usize) {
    // Check if this is a template assignment by looking for TemplateAssign in
    // children
    let has_template = (start_idx + 1..end_idx).any(|idx| {
      self
        .tree
        .nodes
        .get(idx)
        .map(|n| n.token == Token::TemplateAssign)
        .unwrap_or(false)
    });

    if has_template {
      // Template assignment: imu view: </> ::= <>...
      // Store variable name, then execute children
      // starting from the TemplateAssign token onward.
      // Skip the declaration header (ident, colon, type)
      // to avoid treating the variable name as a
      // reference.
      let tpl_name = self.get_var_name(start_idx, end_idx);

      if let Some(name) = tpl_name {
        self.pending_var_name = Some(name);
      }

      // Find TemplateAssign (::=) and execute only that.
      // It internally finds and runs the fragment.
      // Don't iterate other children — the fragment
      // handles all tag/text tokens internally.
      let tpl_assign = ((start_idx + 1)..end_idx)
        .find(|&i| self.tree.nodes[i].token == Token::TemplateAssign);

      if let Some(ta_idx) = tpl_assign {
        let node = self.tree.nodes[ta_idx];
        self.execute_node(&node, ta_idx);
      }

      return;
    }

    // Pop the init value from stack
    if let (Some(init_value), Some(init_ty)) =
      (self.value_stack.pop(), self.ty_stack.pop())
    {
      let sir_init = self.sir_values.pop();

      // Look back in the tree to find the variable name
      // The Ident should be the first child after Imu
      let name = self
        .tree
        .nodes
        .get(start_idx + 1)
        .filter(|node| {
          start_idx + 1 < end_idx && matches!(node.token, Token::Ident)
        })
        .and_then(|_| self.node_value(start_idx + 1))
        .and_then(|val| match val {
          NodeValue::Symbol(sym) => Some(sym),
          _ => None,
        });

      if let Some(name) = name {
        let pubness = if self.is_pub(start_idx) {
          Pubness::Yes
        } else {
          Pubness::No
        };

        let sir_value = self.sir.emit(Insn::VarDef {
          name,
          ty_id: init_ty,
          init: sir_init,
          mutability: Mutability::No,
          pubness,
        });

        self.locals.push(Local {
          name,
          ty_id: init_ty,
          value_id: init_value,
          pubness,
          mutability: Mutability::No,
          sir_value: sir_init,
          local_kind: LocalKind::Variable,
        });

        if let Some(frame) = self.scope_stack.last_mut() {
          frame.count += 1;
        }

        // Don't push anything back - declarations don't produce values
        // Just track the SIR value for completeness
        self.sir_values.push(sir_value);
      }
    }
  }

  /// Executes mutable declaration.
  fn execute_mut(&mut self, _start_idx: usize, _end_idx: usize) {
    // Same as imu but with mutability flag set
    // Pop the init value from stack
    if let (Some(init_value), Some(init_ty)) =
      (self.value_stack.pop(), self.ty_stack.pop())
    {
      let sir_init = self.sir_values.pop();

      // Look back in the tree to find the variable name
      // The Ident should be the first child after Mut
      let name = self
        .tree
        .nodes
        .get(_start_idx + 1)
        .filter(|node| {
          _start_idx + 1 < _end_idx && matches!(node.token, Token::Ident)
        })
        .and_then(|_| self.node_value(_start_idx + 1))
        .and_then(|val| match val {
          NodeValue::Symbol(sym) => Some(sym),
          _ => None,
        });

      if let Some(name) = name {
        let pubness = if self.is_pub(_start_idx) {
          Pubness::Yes
        } else {
          Pubness::No
        };

        let sir_value = self.sir.emit(Insn::VarDef {
          name,
          ty_id: init_ty,
          init: sir_init,
          mutability: Mutability::Yes,
          pubness,
        });

        self.locals.push(Local {
          name,
          ty_id: init_ty,
          value_id: init_value,
          mutability: Mutability::Yes,
          pubness,
          sir_value: sir_init,
          local_kind: LocalKind::Variable,
        });

        if let Some(frame) = self.scope_stack.last_mut() {
          frame.count += 1;
        }

        // Don't push anything back - declarations don't produce values
        self.sir_values.push(sir_value);
      }
    }
  }

  /// Executes an `ffi` declaration — an intrinsic function
  /// with no body. Emits `FunDef { is_intrinsic: true }`.
  fn execute_ffi(&mut self, start_idx: usize, end_idx: usize) {
    // Parse signature: ffi name(params) -> return_ty;
    let name = self
      .tree
      .nodes
      .get(start_idx + 1)
      .filter(|n| n.token == Token::Ident)
      .and_then(|_| self.node_value(start_idx + 1))
      .and_then(|v| match v {
        NodeValue::Symbol(s) => Some(s),
        _ => None,
      });

    if name.is_none() {
      self.skip_until = end_idx;

      return;
    }

    let name = name.unwrap();
    let mut params = Vec::new();
    let mut return_ty = self.ty_checker.unit_type();
    let mut idx = start_idx + 2;

    // Parse optional type parameters: <$T>.
    let outer_type_params = std::mem::take(&mut self.type_params);

    if idx < end_idx && self.tree.nodes[idx].token == Token::LAngle {
      idx += 1;

      while idx < end_idx {
        let tok = self.tree.nodes[idx].token;

        if tok == Token::RAngle {
          idx += 1;
          break;
        }

        if tok == Token::Dollar && idx + 1 < end_idx {
          idx += 1;

          if let Some(NodeValue::Symbol(sym)) = self.node_value(idx) {
            let var = self.ty_checker.fresh_var();

            self.type_params.push((sym, var));
          }
        }

        idx += 1;
      }
    }

    if self.type_params.is_empty() {
      self.type_params = outer_type_params;
    }

    // Parse parameters.
    if idx < end_idx && self.tree.nodes[idx].token == Token::LParen {
      idx += 1;

      while idx < end_idx {
        match &self.tree.nodes[idx].token {
          Token::RParen => {
            idx += 1;
            break;
          }
          Token::Ident => {
            if let Some(NodeValue::Symbol(param_name)) = self.node_value(idx) {
              idx += 1;

              if idx < end_idx {
                let param_ty = self.resolve_type_token(idx);

                // Skip extra token for $T.
                if self.tree.nodes[idx].token == Token::Dollar {
                  idx += 1;
                }

                params.push((param_name, param_ty));
                idx += 1;

                if idx < end_idx && self.tree.nodes[idx].token == Token::Comma {
                  idx += 1;
                }
              }
            } else {
              idx += 1;
            }
          }
          _ => idx += 1,
        }
      }
    }

    // Parse return type, including generic type arguments
    // like `Result<str, int>`.
    let mut type_args: Vec<TyId> = Vec::new();

    while idx < end_idx {
      match self.tree.nodes[idx].token {
        Token::Arrow => {
          if idx + 1 < end_idx {
            idx += 1;
            return_ty = self.resolve_type_token(idx);
            idx += 1;

            // Collect type arguments after the base type.
            // The parser emits `<` as Token::Lt in normal
            // code mode (not LAngle, which is template-only).
            while idx < end_idx {
              let tok = self.tree.nodes[idx].token;

              if tok.is_ty() || tok == Token::Ident {
                type_args.push(self.resolve_type_token(idx));
                idx += 1;
              } else if matches!(tok, Token::Lt | Token::Gt | Token::Comma) {
                idx += 1;
              } else {
                break;
              }
            }
          }

          break;
        }
        Token::Semicolon => break,
        _ => idx += 1,
      }
    }

    // Each ext function returning a parameterized type
    // (e.g. Result<str,int> vs Result<int,int>) must get
    // its own independent return type. Using a fresh
    // inference variable prevents multiple ext signatures
    // from fighting over shared type-parameter bindings.
    if !type_args.is_empty() {
      return_ty = self.ty_checker.fresh_var();
    }

    let pubness = if self.is_pub(start_idx) {
      Pubness::Yes
    } else {
      Pubness::No
    };

    self.sir.emit(Insn::FunDef {
      name,
      params: params.clone(),
      return_ty,
      body_start: 0,
      kind: FunctionKind::Intrinsic,
      pubness,
    });

    // Register as known function.
    self.funs.push(FunDef {
      name,
      params,
      return_ty,
      body_start: 0,
      kind: FunctionKind::Intrinsic,
      pubness,
      type_params: self.type_params.iter().map(|(_, ty)| *ty).collect(),
      return_type_args: type_args
        .iter()
        .map(|t| self.ty_checker.resolve_ty(*t))
        .collect(),
    });

    // Skip all children — no body to process.
    self.skip_until = end_idx;
  }

  /// Executes an enum declaration.
  ///
  /// Parses: `enum Name { V1, V2(Type), V3 = N, ... }`
  /// Emits `Insn::EnumDef` and registers the enum type.
  fn execute_enum(&mut self, start_idx: usize, end_idx: usize) {
    // Parse name.
    let name = self
      .tree
      .nodes
      .get(start_idx + 1)
      .filter(|n| n.token == Token::Ident)
      .and_then(|_| self.node_value(start_idx + 1))
      .and_then(|v| match v {
        NodeValue::Symbol(s) => Some(s),
        _ => None,
      });

    let name = match name {
      Some(n) => n,
      None => {
        self.skip_until = end_idx;
        return;
      }
    };

    let pubness = if self.is_pub(start_idx) {
      Pubness::Yes
    } else {
      Pubness::No
    };

    // Parse variants inside { ... }.
    // Tree children: Ident(name), LBrace, [variant tokens], RBrace
    let mut variants: Vec<(Symbol, u32, Vec<TyId>)> = Vec::new();
    let mut disc: u32 = 0;
    let mut idx = start_idx + 2;

    // Skip to LBrace.
    while idx < end_idx && self.tree.nodes[idx].token != Token::LBrace {
      idx += 1;
    }

    if idx < end_idx {
      idx += 1; // skip LBrace
    }

    // Parse variants.
    while idx < end_idx {
      match self.tree.nodes[idx].token {
        Token::RBrace => break,
        Token::Comma => idx += 1,

        Token::Ident => {
          let vname = self.node_value(idx).and_then(|v| match v {
            NodeValue::Symbol(s) => Some(s),
            _ => None,
          });

          if let Some(vname) = vname {
            idx += 1;
            let mut fields = Vec::new();

            // Check for tuple payload: Variant(Type, ...)
            if idx < end_idx && self.tree.nodes[idx].token == Token::LParen {
              idx += 1; // skip (

              while idx < end_idx {
                match self.tree.nodes[idx].token {
                  Token::RParen => {
                    idx += 1;
                    break;
                  }
                  Token::Comma => idx += 1,
                  _ if self.tree.nodes[idx].token.is_ty() => {
                    let ty = self.resolve_type_token(idx);
                    fields.push(ty);
                    idx += 1;
                  }
                  Token::Ident => {
                    // Named type (e.g. error).
                    let ty = self.ty_checker.fresh_var();
                    fields.push(ty);
                    idx += 1;
                  }
                  _ => idx += 1,
                }
              }
            }

            // Check for explicit discriminant: V = N
            if idx < end_idx && self.tree.nodes[idx].token == Token::Eq {
              idx += 1; // skip =

              if idx < end_idx {
                if let Some(NodeValue::Literal(lit)) = self.node_value(idx) {
                  disc = self.literals.int_literals[lit as usize] as u32;
                }

                idx += 1;
              }
            }

            variants.push((vname, disc, fields));
            disc += 1;
          } else {
            idx += 1;
          }
        }
        _ => idx += 1,
      }
    }

    // Intern enum type.
    let enum_ty_id = self.ty_checker.ty_table.intern_enum(name, &variants);
    let ty_id = self.ty_checker.intern_ty(Ty::Enum(enum_ty_id));

    self.sir.emit(Insn::EnumDef {
      name,
      ty_id,
      variants,
      pubness,
    });

    // Register for variant construction lookup.
    self.enum_defs.push((name, enum_ty_id, ty_id));

    self.skip_until = end_idx;
  }

  /// Tries to handle `LBrace` as struct construction.
  /// Returns true if it was a struct construct, false
  /// if it should be handled as a normal scope block.
  fn try_struct_construct(&mut self, brace_idx: usize) -> bool {
    // Don't intercept function body braces.
    if self.pending_function.is_some() {
      return false;
    }

    if brace_idx < 1 {
      return false;
    }

    // Previous token must be an ident or Self matching
    // a struct.
    let prev = brace_idx - 1;
    let prev_tok = self.tree.nodes[prev].token;

    let struct_name = match prev_tok {
      Token::Ident => match self.node_value(prev) {
        Some(NodeValue::Symbol(s)) => s,
        _ => return false,
      },
      Token::SelfUpper => match self.apply_context {
        Some(s) => s,
        None => return false,
      },
      _ => return false,
    };

    let entry = self
      .ty_checker
      .ty_table
      .struct_intern_lookup(struct_name)
      .copied();

    let sty_id = match entry {
      Some(id) => id,
      None => return false,
    };

    let struct_ty = match self.ty_checker.ty_table.struct_ty(sty_id) {
      Some(st) => *st,
      None => return false,
    };

    let ty_id = self.ty_checker.intern_ty(Ty::Struct(sty_id));
    let field_defs =
      self.ty_checker.ty_table.struct_fields(&struct_ty).to_vec();

    // Find matching RBrace.
    let header = self.tree.nodes[brace_idx];
    let children_end = (header.child_start + header.child_count) as usize;

    // Process field assignments: name: expr, ...
    // Execute children between { and } to evaluate
    // field value expressions.
    let mut field_values = vec![None; field_defs.len()];
    let mut idx = brace_idx + 1;

    while idx < children_end {
      match self.tree.nodes[idx].token {
        Token::RBrace => break,
        Token::Comma => idx += 1,

        Token::Ident => {
          let fname = self.node_value(idx).and_then(|v| match v {
            NodeValue::Symbol(s) => Some(s),
            _ => None,
          });

          if let Some(fname) = fname {
            // Find field index.
            let fname_str = self.interner.get(fname).to_owned();
            let field_idx = field_defs
              .iter()
              .position(|f| self.interner.get(f.name) == fname_str);

            idx += 1;

            // Check for shorthand: `{ lo, hi }` where
            // field name = variable name (no colon).
            if idx < children_end && self.tree.nodes[idx].token == Token::Colon
            {
              idx += 1; // skip colon

              // Execute value expression nodes until
              // next comma or RBrace.
              let expr_start = idx;

              while idx < children_end
                && !matches!(
                  self.tree.nodes[idx].token,
                  Token::Comma | Token::RBrace
                )
              {
                let node = self.tree.nodes[idx];
                self.execute_node(&node, idx);
                idx += 1;
              }

              if idx > expr_start
                && let Some(sir_val) = self.sir_values.pop()
              {
                self.value_stack.pop();
                let val_ty =
                  self.ty_stack.pop().unwrap_or(self.ty_checker.unit_type());

                if let Some(fi) = field_idx {
                  // Unify value type with field type.
                  let field_ty = field_defs[fi].ty_id;
                  let span = self.tree.spans[expr_start];

                  self.ty_checker.unify(field_ty, val_ty, span);

                  field_values[fi] = Some(sir_val);
                }
              }
            } else {
              // Shorthand: field name IS the value.
              // Emit a Load for the variable with the
              // same name as the field.
              if let Some(local) =
                self.lookup_local(fname).map(|l| (l.ty_id, l.sir_value))
              {
                let (var_ty, sir_value) = local;

                let sir_val = match sir_value {
                  Some(sv) => sv,
                  None => {
                    let dst = ValueId(self.sir.next_value_id);
                    self.sir.next_value_id += 1;

                    self.sir.emit(Insn::Load {
                      dst,
                      src: LoadSource::Local(fname),
                      ty_id: var_ty,
                    })
                  }
                };

                if let Some(fi) = field_idx {
                  field_values[fi] = Some(sir_val);
                }
              }
            }
          } else {
            idx += 1;
          }
        }
        _ => idx += 1,
      }
    }

    // Collect field ValueIds (use default placeholder
    // for missing fields).
    let fields = field_values
      .into_iter()
      .map(|v| v.unwrap_or(ValueId(u32::MAX)))
      .collect::<Vec<_>>();

    let dst = ValueId(self.sir.next_value_id);
    self.sir.next_value_id += 1;

    let sv = self.sir.emit(Insn::StructConstruct {
      dst,
      struct_name,
      fields,
      ty_id,
    });

    let rid = self.values.store_runtime(0);

    self.value_stack.push(rid);
    self.ty_stack.push(ty_id);
    self.sir_values.push(sv);

    // Skip past the struct construct block. Find the
    // RBrace position.
    let mut skip = brace_idx + 1;

    while skip < self.tree.nodes.len() {
      if self.tree.nodes[skip].token == Token::RBrace {
        skip += 1;

        break;
      }
      skip += 1;
    }

    self.skip_until = skip;

    true
  }

  /// Executes a struct declaration.
  ///
  /// Parses: `struct Name { field: Type, ... }`
  /// Emits `Insn::StructDef` and registers the struct type.
  /// Executes `type Foo = int;` — registers a type alias.
  fn execute_type_alias(&mut self, start_idx: usize, end_idx: usize) {
    // Extract alias name (first Ident child).
    let name = self
      .tree
      .nodes
      .get(start_idx + 1)
      .filter(|n| n.token == Token::Ident)
      .and_then(|_| self.node_value(start_idx + 1))
      .and_then(|v| match v {
        NodeValue::Symbol(s) => Some(s),
        _ => None,
      });

    let name = match name {
      Some(n) => n,
      None => return,
    };

    // Scan for target type after `=`.
    let mut target_ty: Option<TyId> = None;
    let mut idx = start_idx + 2;

    while idx < end_idx {
      let tok = self.tree.nodes[idx].token;

      if tok == Token::Eq {
        idx += 1;

        continue;
      }

      // Semicolon ends the declaration.
      if tok == Token::Semicolon {
        break;
      }

      // Tuple type: (int, float).
      if tok == Token::LParen {
        let (ty_id, skip) = self.resolve_tuple_type(idx);

        target_ty = Some(ty_id);
        idx = skip;

        continue;
      }

      // Function type: Fn(int) -> int.
      if tok == Token::FnType {
        let (ty_id, skip) = self.resolve_fn_type(idx);

        target_ty = Some(ty_id);
        idx = skip;

        continue;
      }

      // Array type: token followed by [].
      if tok.is_ty() || tok == Token::Ident {
        let base_ty = if tok == Token::Ident {
          self
            .node_value(idx)
            .and_then(|v| match v {
              NodeValue::Symbol(s) => Some(s),
              _ => None,
            })
            .and_then(|sym| {
              self.ty_checker.resolve_ty_symbol(sym, self.interner)
            })
            .unwrap_or_else(|| self.ty_checker.unit_type())
        } else {
          self.resolve_type_token(idx)
        };

        target_ty = Some(base_ty);
        idx += 1;

        continue;
      }

      idx += 1;
    }

    if let Some(ty) = target_ty {
      self.ty_checker.define_ty_alias(name, ty);
    }
  }

  /// Executes `group type Foo = int and Bar = float;`.
  fn execute_group_type(&mut self, start_idx: usize, end_idx: usize) {
    let mut idx = start_idx + 1;

    while idx < end_idx {
      let tok = self.tree.nodes[idx].token;

      if tok == Token::Semicolon {
        break;
      }

      // Each `type` sub-node is a full alias.
      if tok == Token::Type {
        let header = self.tree.nodes[idx];
        let child_end = (header.child_start + header.child_count) as usize;

        self.execute_type_alias(idx, child_end);

        idx = child_end;

        continue;
      }

      // `and` separator — skip.
      if tok == Token::And {
        idx += 1;

        continue;
      }

      idx += 1;
    }
  }

  fn execute_struct(&mut self, start_idx: usize, end_idx: usize) {
    let name = self
      .tree
      .nodes
      .get(start_idx + 1)
      .filter(|n| n.token == Token::Ident)
      .and_then(|_| self.node_value(start_idx + 1))
      .and_then(|v| match v {
        NodeValue::Symbol(s) => Some(s),
        _ => None,
      });

    let name = match name {
      Some(n) => n,
      None => {
        self.skip_until = end_idx;
        return;
      }
    };

    let pubness = if self.is_pub(start_idx) {
      Pubness::Yes
    } else {
      Pubness::No
    };

    // Parse optional type parameters: <$T, $A>.
    self.type_params.clear();

    let mut idx = start_idx + 2;

    if idx < end_idx && self.tree.nodes[idx].token == Token::LAngle {
      idx += 1; // skip <

      while idx < end_idx {
        let tok = self.tree.nodes[idx].token;

        if tok == Token::RAngle {
          idx += 1;
          break;
        }

        if tok == Token::Dollar && idx + 1 < end_idx {
          idx += 1;

          if let Some(NodeValue::Symbol(sym)) = self.node_value(idx) {
            let var = self.ty_checker.fresh_var();

            self.type_params.push((sym, var));
          }
        }

        idx += 1;
      }
    }

    // Skip to LBrace.
    while idx < end_idx && self.tree.nodes[idx].token != Token::LBrace {
      idx += 1;
    }

    if idx < end_idx {
      idx += 1; // skip LBrace
    }

    // Parse fields: name: Type, name: Type = default, ...
    let mut fields: Vec<(Symbol, TyId, bool)> = Vec::new();
    // Default value tree indices: (token, node_index, field_ty).
    let mut default_nodes: Vec<Option<(Token, usize, TyId)>> = Vec::new();

    while idx < end_idx {
      match self.tree.nodes[idx].token {
        Token::RBrace => break,
        Token::Comma => idx += 1,
        Token::Pub => idx += 1,

        Token::Ident => {
          let fname = self.node_value(idx).and_then(|v| match v {
            NodeValue::Symbol(s) => Some(s),
            _ => None,
          });

          if let Some(fname) = fname {
            idx += 1;

            // Skip colon between name and type.
            if idx < end_idx && self.tree.nodes[idx].token == Token::Colon {
              idx += 1;
            }

            // Expect type token after field name.
            // Handle $T (Dollar + Ident) for generic fields.
            let fty =
              if idx < end_idx && self.tree.nodes[idx].token == Token::Dollar {
                let ty = self.resolve_type_token(idx);

                idx += 2; // skip Dollar + Ident
                ty
              } else if idx < end_idx && self.tree.nodes[idx].token.is_ty() {
                let ty = self.resolve_type_token(idx);

                idx += 1;
                ty
              } else {
                self.ty_checker.fresh_var()
              };

            // Check for default value: = expr
            let has_default =
              idx < end_idx && self.tree.nodes[idx].token == Token::Eq;

            if has_default {
              idx += 1; // skip =

              // Capture the default value node.
              if idx < end_idx {
                let def_tok = self.tree.nodes[idx].token;

                default_nodes.push(Some((def_tok, idx, fty)));
                idx += 1;
              } else {
                default_nodes.push(None);
              }
            } else {
              default_nodes.push(None);
            }

            fields.push((fname, fty, has_default));
          } else {
            idx += 1;
          }
        }
        _ => idx += 1,
      }
    }

    // Intern struct type.
    let struct_ty_id = self.ty_checker.ty_table.intern_struct(name, &fields);
    let ty_id = self.ty_checker.intern_ty(Ty::Struct(struct_ty_id));

    self.sir.emit(Insn::StructDef {
      name,
      ty_id,
      fields: fields.clone(),
      pubness,
    });

    // Auto-generate `Type::default()` if all fields have
    // default values. Emits a synthetic FunDef that constructs
    // the struct with the default literals.
    let all_have_defaults =
      !fields.is_empty() && default_nodes.iter().all(|d| d.is_some());

    if all_have_defaults {
      let type_str = self.interner.get(name).to_owned();
      let fn_name = self.interner.intern(&format!("{type_str}::default"));

      let body_start = (self.sir.instructions.len() + 1) as u32;

      self.sir.emit(Insn::FunDef {
        name: fn_name,
        params: vec![],
        return_ty: ty_id,
        body_start,
        kind: FunctionKind::UserDefined,
        pubness,
      });

      // Emit default value constants.
      let mut field_sirs = Vec::with_capacity(fields.len());

      for (tok, node_idx, fty) in default_nodes.iter().flatten() {
        let dst = ValueId(self.sir.next_value_id);
        self.sir.next_value_id += 1;

        let sv = match tok {
          Token::Int => {
            let value = match self.node_value(*node_idx) {
              Some(NodeValue::Literal(lit)) => {
                self.literals.int_literals[lit as usize]
              }
              _ => 0,
            };

            self.sir.emit(Insn::ConstInt {
              dst,
              value,
              ty_id: *fty,
            })
          }
          Token::Float => {
            let value = match self.node_value(*node_idx) {
              Some(NodeValue::Literal(lit)) => {
                self.literals.float_literals[lit as usize]
              }
              _ => 0.0,
            };

            self.sir.emit(Insn::ConstFloat {
              dst,
              value,
              ty_id: *fty,
            })
          }
          Token::True => self.sir.emit(Insn::ConstBool {
            dst,
            value: true,
            ty_id: *fty,
          }),
          Token::False => self.sir.emit(Insn::ConstBool {
            dst,
            value: false,
            ty_id: *fty,
          }),
          Token::String => {
            let symbol = match self.node_value(*node_idx) {
              Some(NodeValue::Literal(lit)) => {
                self.literals.identifiers[lit as usize]
              }
              Some(NodeValue::Symbol(sym)) => sym,
              _ => self.interner.intern(""),
            };

            self.sir.emit(Insn::ConstString {
              dst,
              symbol,
              ty_id: *fty,
            })
          }
          _ => {
            // Unsupported default expression type.
            self.sir.emit(Insn::ConstInt {
              dst,
              value: 0,
              ty_id: *fty,
            })
          }
        };

        field_sirs.push(sv);
      }

      // Emit StructConstruct + Return.
      let construct_dst = ValueId(self.sir.next_value_id);
      self.sir.next_value_id += 1;

      let construct_sv = self.sir.emit(Insn::StructConstruct {
        dst: construct_dst,
        struct_name: name,
        fields: field_sirs,
        ty_id,
      });

      self.sir.emit(Insn::Return {
        value: Some(construct_sv),
        ty_id,
      });

      // Register the synthetic function.
      self.funs.push(FunDef {
        name: fn_name,
        params: vec![],
        return_ty: ty_id,
        body_start,
        kind: FunctionKind::UserDefined,
        pubness,
        type_params: vec![],
        return_type_args: vec![],
      });
    }

    self.skip_until = end_idx;
  }

  /// Executes `abstract Name { fun method(self) -> Type; }`
  ///
  /// Parses method signatures (no bodies) and registers
  /// the abstract definition. Methods end with `;`.
  fn execute_abstract(&mut self, start_idx: usize, end_idx: usize) {
    // Parse abstract name.
    let name = match self
      .tree
      .nodes
      .get(start_idx + 1)
      .filter(|n| n.token == Token::Ident)
      .and_then(|_| self.node_value(start_idx + 1))
    {
      Some(NodeValue::Symbol(s)) => s,
      _ => {
        self.skip_until = end_idx;
        return;
      }
    };

    // Find LBrace.
    let mut idx = start_idx + 2;

    while idx < end_idx && self.tree.nodes[idx].token != Token::LBrace {
      idx += 1;
    }

    if idx < end_idx {
      idx += 1; // skip LBrace
    }

    // Parse method signatures.
    let mut methods = Vec::new();

    while idx < end_idx {
      let tok = self.tree.nodes[idx].token;

      if tok == Token::RBrace {
        break;
      }

      // Each method: Fun Ident LParen params RParen
      //              [ Arrow Type ] Semicolon
      if tok == Token::Fun {
        idx += 1; // skip Fun

        // Method name.
        let method_name =
          if idx < end_idx && self.tree.nodes[idx].token == Token::Ident {
            let sym = self.node_value(idx).and_then(|v| match v {
              NodeValue::Symbol(s) => Some(s),
              _ => None,
            });
            idx += 1;
            sym
          } else {
            None
          };

        // Skip LParen.
        if idx < end_idx && self.tree.nodes[idx].token == Token::LParen {
          idx += 1;
        }

        // Parse params until RParen.
        let mut params = Vec::new();

        while idx < end_idx && self.tree.nodes[idx].token != Token::RParen {
          let ptok = self.tree.nodes[idx].token;

          if ptok == Token::Comma {
            idx += 1;
            continue;
          }

          // `self` param.
          if ptok == Token::SelfLower {
            let self_sym = self.interner.intern("self");
            // Placeholder type — resolved at apply time.
            let self_ty = self.ty_checker.fresh_var();

            params.push((self_sym, self_ty));
            idx += 1;

            // Skip optional `: Type` after self.
            if idx < end_idx && self.tree.nodes[idx].token == Token::Colon {
              idx += 1; // skip :

              if idx < end_idx && self.tree.nodes[idx].token.is_ty() {
                idx += 1; // skip type
              }
            }

            continue;
          }

          // Named param: name : Type
          if ptok == Token::Ident {
            let pname = self
              .node_value(idx)
              .and_then(|v| match v {
                NodeValue::Symbol(s) => Some(s),
                _ => None,
              })
              .unwrap_or(Symbol::UNDERSCORE);

            idx += 1;

            // Skip colon.
            if idx < end_idx && self.tree.nodes[idx].token == Token::Colon {
              idx += 1;
            }

            // Parse type.
            let pty = if idx < end_idx && self.tree.nodes[idx].token.is_ty() {
              let ty = self.resolve_type_token(idx);
              idx += 1;
              ty
            } else if idx < end_idx
              && self.tree.nodes[idx].token == Token::SelfUpper
            {
              // Self type — placeholder.
              idx += 1;
              self.ty_checker.fresh_var()
            } else {
              self.ty_checker.fresh_var()
            };

            params.push((pname, pty));
            continue;
          }

          idx += 1;
        }

        // Skip RParen.
        if idx < end_idx && self.tree.nodes[idx].token == Token::RParen {
          idx += 1;
        }

        // Optional return type: -> Type
        let return_ty =
          if idx < end_idx && self.tree.nodes[idx].token == Token::Arrow {
            idx += 1; // skip ->

            if idx < end_idx && self.tree.nodes[idx].token.is_ty() {
              let ty = self.resolve_type_token(idx);
              idx += 1;
              ty
            } else if idx < end_idx
              && self.tree.nodes[idx].token == Token::SelfUpper
            {
              idx += 1;
              self.ty_checker.fresh_var()
            } else {
              self.ty_checker.unit_type()
            }
          } else {
            self.ty_checker.unit_type()
          };

        // Skip semicolon (abstract methods have no body).
        if idx < end_idx && self.tree.nodes[idx].token == Token::Semicolon {
          idx += 1;
        }

        if let Some(mname) = method_name {
          methods.push(AbstractMethod {
            name: mname,
            params,
            return_ty,
          });
        }

        continue;
      }

      idx += 1;
    }

    self.abstract_defs.insert(name, AbstractDef { methods });
    self.skip_until = end_idx;
  }

  /// Executes `apply Type { fun_defs... }`.
  ///
  /// Sets the apply context so child function definitions
  /// get mangled names (`Type::method`). `Self` resolves
  /// to the applied type.
  /// Primitive-type tokens accepted as the target of
  /// `apply <T> { ... }` (inherent methods on primitives).
  /// Kept as a single predicate so the parser check,
  /// `execute_apply`'s scan, the `For <Target>` rescan, and
  /// the call-site mangling helpers stay in sync.
  fn is_primitive_type_token(tok: Token) -> bool {
    matches!(
      tok,
      Token::CharType
        | Token::StrType
        | Token::IntType
        | Token::UintType
        | Token::FloatType
        | Token::BoolType
    )
  }

  /// Canonical keyword string for a primitive `Ty` — must
  /// match `Token::ty_keyword_str` so the mangling symbol
  /// used at the call site (`"char" + "::" + method`) lines
  /// up with the one `execute_apply` interned at definition
  /// time. Widths collapse to their canonical spelling
  /// (`int = s32`, `uint = u32`, `float = f64`) so that
  /// `apply int` dispatches regardless of which sized-int
  /// literal the user wrote. Returns `None` for
  /// non-primitive Ty kinds.
  fn primitive_ty_name_str(ty: &Ty) -> Option<&'static str> {
    match ty {
      Ty::Char => Some("char"),
      Ty::Str => Some("str"),
      Ty::Bool => Some("bool"),
      Ty::Int { signed: true, .. } => Some("int"),
      Ty::Int { signed: false, .. } => Some("uint"),
      Ty::Float(_) => Some("float"),
      _ => None,
    }
  }

  /// Canonical mangling prefix for an array `Ty` — returns
  /// `arr_<elem>` when the element is a primitive. Mirrors
  /// `execute_apply`'s `apply []<primitive>` name synthesis
  /// so call-site dispatch on `Ty::Array(...)` receivers
  /// resolves to the same mangled fun registered at
  /// definition time. Nested arrays (`[][]int`) and array
  /// of struct/enum are out of scope for now.
  fn array_ty_name_str(&mut self, resolved: &Ty) -> Option<String> {
    let Ty::Array(aid) = resolved else {
      return None;
    };

    let arr = self.ty_checker.ty_table.array(*aid).copied()?;
    let elem = self.ty_checker.kind_of(arr.elem_ty);

    Self::primitive_ty_name_str(&elem).map(|s| format!("arr_{s}"))
  }

  /// Resolve the `apply` target's `type_name` symbol to a
  /// concrete `TyId` for the `self` parameter. Handles the
  /// three shapes `execute_apply` mints:
  ///   - primitive keyword names (`"char"`, `"int"`, …) —
  ///     delegate to `resolve_ty_symbol`.
  ///   - `"arr_<primitive>"` — decode the suffix and intern
  ///     the matching `Ty::Array`.
  ///   - Ident names (user types) — fall through to
  ///     `resolve_ty_symbol` which hits the struct/enum
  ///     tables.
  fn resolve_apply_self_ty(&mut self, type_name: Symbol) -> Option<TyId> {
    let name_owned = self.interner.get(type_name).to_owned();

    if let Some(elem_name) = name_owned.strip_prefix("arr_") {
      let elem_sym = self.interner.intern(elem_name);
      let elem_ty =
        self.ty_checker.resolve_ty_symbol(elem_sym, self.interner)?;

      let aid = self.ty_checker.ty_table.intern_array(elem_ty, None);

      return Some(self.ty_checker.intern_ty(Ty::Array(aid)));
    }

    self.ty_checker.resolve_ty_symbol(type_name, self.interner)
  }

  fn execute_apply(&mut self, start_idx: usize, end_idx: usize) {
    // Parse the applied type. Three shapes:
    //   1. `Ident` — user type (struct/enum/alias).
    //   2. primitive keyword (`CharType` et al.) — inherent
    //      methods on that primitive.
    //   3. `LBracket RBracket <primitive>` — inherent
    //      methods on `[]T` arrays. Mangled as
    //      `arr_<primitive>` so `apply []int { fun sum }`
    //      registers `arr_int::sum`.
    let first_ty_idx = (start_idx + 1..end_idx).find(|&i| {
      let tok = self.tree.nodes[i].token;

      tok == Token::Ident
        || tok == Token::LBracket
        || Self::is_primitive_type_token(tok)
    });

    let first_name = first_ty_idx.and_then(|i| {
      let tok = self.tree.nodes[i].token;

      if tok == Token::Ident {
        self.node_value(i).and_then(|v| match v {
          NodeValue::Symbol(s) => Some(s),
          _ => None,
        })
      } else if tok == Token::LBracket {
        // Array target: expect `[ ] <primitive>` and build
        // the canonical mangling prefix `arr_<primitive>`.
        // The call-site helper `array_ty_name_str` must
        // produce the same string for `Ty::Array(elem)`.
        let elem_idx = (i + 1..end_idx).find(|&j| {
          let t = self.tree.nodes[j].token;

          t != Token::LBracket && t != Token::RBracket
        })?;

        let elem_tok = self.tree.nodes[elem_idx].token;

        if Self::is_primitive_type_token(elem_tok)
          && let Some(elem_name) = elem_tok.ty_keyword_str()
        {
          let mangled = format!("arr_{elem_name}");

          Some(self.interner.intern(&mangled))
        } else {
          None
        }
      } else {
        // Primitive keyword — synthesize its symbol via the
        // token's canonical name string ("char" / "str" /
        // "int" / "float" / "bool") so mangling (and the
        // `self`-type lookup) keys off the same string as
        // `resolve_ty_symbol` uses.
        tok.ty_keyword_str().map(|s| self.interner.intern(s))
      }
    });

    let first_name = match first_name {
      Some(n) => n,
      None => {
        self.skip_until = end_idx;
        return;
      }
    };

    // Detect `apply Abstract for Type { ... }`.
    // The parser may place `For Ident(Type)` either before
    // or inside the LBrace. Scan from start_idx+2 through
    // the first few children for Token::For.
    let mut abstract_name: Option<Symbol> = None;
    let mut type_name = first_name;

    let scan_start = first_ty_idx.map(|i| i + 1).unwrap_or(start_idx + 2);

    for scan in scan_start..end_idx.min(scan_start + 8) {
      if self.tree.nodes[scan].token == Token::For {
        abstract_name = Some(first_name);

        // Next token after For is the target type. Accept
        // Ident OR primitive keyword (same widening).
        if scan + 1 < end_idx {
          let tok = self.tree.nodes[scan + 1].token;

          if tok == Token::Ident
            && let Some(NodeValue::Symbol(s)) = self.node_value(scan + 1)
          {
            type_name = s;
          } else if Self::is_primitive_type_token(tok)
            && let Some(s) = tok.ty_keyword_str()
          {
            type_name = self.interner.intern(s);
          }
        }

        break;
      }

      // Stop at Fun (start of method body).
      if self.tree.nodes[scan].token == Token::Fun {
        break;
      }
    }

    // Set apply context to the TARGET type (not the abstract).
    let outer_apply = self.apply_context.take();

    self.apply_context = Some(type_name);

    // Parse optional type parameters: <$T, $A>.
    // These become available in method signatures.
    self.type_params.clear();

    let mut idx = start_idx + 2;

    if idx < end_idx && self.tree.nodes[idx].token == Token::LAngle {
      idx += 1;

      while idx < end_idx {
        let tok = self.tree.nodes[idx].token;

        if tok == Token::RAngle {
          idx += 1;
          break;
        }

        if tok == Token::Dollar && idx + 1 < end_idx {
          idx += 1;

          if let Some(NodeValue::Symbol(sym)) = self.node_value(idx) {
            let var = self.ty_checker.fresh_var();

            self.type_params.push((sym, var));
          }
        }

        idx += 1;
      }
    }

    // Skip to LBrace, then process children normally.
    // The fun handler will read apply_context to mangle
    // names and resolve Self.
    while idx < end_idx && self.tree.nodes[idx].token != Token::LBrace {
      idx += 1;
    }

    // Process children inside { ... }.
    if idx < end_idx {
      idx += 1; // skip LBrace
    }

    while idx < end_idx {
      if idx < self.skip_until {
        idx += 1;
        continue;
      }

      let node = self.tree.nodes[idx];

      self.execute_node(&node, idx);

      idx += 1;
    }

    // Register abstract implementation if this was
    // `apply Abstract for Type { ... }`.
    if let Some(abs_name) = abstract_name {
      // Collect method names that were mangled as
      // Type::method during apply processing.
      let type_str = self.interner.get(type_name).to_owned();
      let method_names: Vec<Symbol> = self
        .funs
        .iter()
        .filter_map(|f| {
          let fname = self.interner.get(f.name);

          if fname.starts_with(&type_str) && fname.contains("::") {
            Some(f.name)
          } else {
            None
          }
        })
        .collect();

      self
        .abstract_impls
        .insert((abs_name, type_name), method_names);
    }

    // Restore outer context.
    self.apply_context = outer_apply;
    self.skip_until = end_idx;
  }

  /// Resolves `Foo::Ok` or `Foo::Ok(42)` enum variant
  /// access at `::` position.
  fn execute_enum_access(&mut self, idx: usize) {
    if idx < 1 || idx + 1 >= self.tree.nodes.len() {
      return;
    }

    // Previous token: enum/struct name or Self.
    let prev_tok = self.tree.nodes[idx - 1].token;

    let enum_name = match prev_tok {
      Token::Ident => match self.node_value(idx - 1) {
        Some(NodeValue::Symbol(s)) => s,
        _ => return,
      },
      Token::SelfUpper => match self.apply_context {
        Some(s) => s,
        None => return,
      },
      _ => return,
    };

    // Next token: must be an ident.
    if self.tree.nodes[idx + 1].token != Token::Ident {
      return;
    }

    let member_name = match self.node_value(idx + 1) {
      Some(NodeValue::Symbol(s)) => s,
      _ => return,
    };

    // Try enum variant first. If not found in enum_defs,
    // check pending imports and lazy-intern on first use.
    let mut entry = self.enum_defs.iter().find(|e| e.0 == enum_name).copied();

    if entry.is_none() {
      let enum_name_str = self.interner.get(enum_name).to_owned();

      if let Some(pos) = self
        .pending_imported_enums
        .iter()
        .position(|e| self.interner.get(e.name) == enum_name_str)
      {
        let en = self.pending_imported_enums.remove(pos);

        // Use fresh inference variables for generic field
        // types so monomorphization can substitute concrete
        // types (e.g. `$T` → `int`). Bump the level to
        // isolate these from function-body generalization —
        // without this, they pollute the HM state and break
        // ternary/if-else type unification.
        self.ty_checker.push_scope();

        let fresh_variants: Vec<(Symbol, u32, Vec<TyId>)> = en
          .variants
          .iter()
          .map(|(name, disc, fields)| {
            let pf: Vec<TyId> =
              fields.iter().map(|_| self.ty_checker.fresh_var()).collect();

            (*name, *disc, pf)
          })
          .collect();

        self.ty_checker.pop_scope();

        let ety_id = self
          .ty_checker
          .ty_table
          .intern_enum(en.name, &fresh_variants);
        let ty_id = self.ty_checker.intern_ty(zo_ty::Ty::Enum(ety_id));

        self.enum_defs.push((en.name, ety_id, ty_id));

        // Emit EnumDef so the codegen registers
        // enum_metas for match discriminant handling.
        self.sir.emit(Insn::EnumDef {
          name: en.name,
          ty_id,
          variants: fresh_variants,
          pubness: zo_value::Pubness::No,
        });

        entry = Some((en.name, ety_id, ty_id));
      }
    }

    if entry.is_none() {
      // Not an enum — try method call (apply).
      // Build mangled name: Type::method.
      let type_str = self.interner.get(enum_name).to_owned();
      let method_str = self.interner.get(member_name).to_owned();
      let mangled = format!("{type_str}::{method_str}");
      let mangled_sym = self.interner.intern(&mangled);

      // Check if mangled name is a known function.
      if self.funs.iter().any(|f| f.name == mangled_sym) {
        // Rewrite the function name for execute_call.
        // The next RParen will trigger execute_call
        // with this name.
        // Skip :: and member ident.
        self.skip_until = idx + 2;
        return;
      }

      return;
    }

    let (_, ety_id, ty_id) = entry.unwrap();
    let var_name = member_name;

    // Resolve variant.
    let enum_ty = match self.ty_checker.ty_table.enum_ty(ety_id) {
      Some(et) => *et,
      None => return,
    };

    let var_str = self.interner.get(var_name).to_owned();
    let variants = self.ty_checker.ty_table.enum_variants(&enum_ty);

    let found = variants
      .iter()
      .find(|v| self.interner.get(v.name) == var_str)
      .copied();

    let variant = match found {
      Some(v) => v,
      None => {
        // Not a variant — try method call (apply).
        let type_str = self.interner.get(enum_name).to_owned();
        let method_str = self.interner.get(member_name).to_owned();
        let mangled = format!("{type_str}::{method_str}");
        let mangled_sym = self.interner.intern(&mangled);

        if self.funs.iter().any(|f| f.name == mangled_sym) {
          self.skip_until = idx + 2;
        }

        return;
      }
    };

    // Skip the variant ident.
    self.skip_until = idx + 2;

    if variant.field_count == 0 {
      // Unit variant — emit immediately.
      let dst = ValueId(self.sir.next_value_id);
      self.sir.next_value_id += 1;

      let sv = self.sir.emit(Insn::EnumConstruct {
        dst,
        enum_name,
        variant: variant.discriminant,
        fields: Vec::new(),
        ty_id,
      });

      let rid = self.values.store_runtime(0);

      self.value_stack.push(rid);
      self.ty_stack.push(ty_id);
      self.sir_values.push(sv);
    } else {
      // Tuple variant — defer to RParen.
      self.pending_enum_construct =
        Some((enum_name, variant.discriminant, variant.field_count, ty_id));
    }
  }

  /// Checks if the current Dot is a method call rather
  /// than field access. Peeks at the stack without
  /// consuming.
  fn is_dot_method_call(&mut self, dot_idx: usize) -> bool {
    // Next token after Dot must be LParen for a call.
    if dot_idx + 1 >= self.tree.nodes.len()
      || self.tree.nodes[dot_idx + 1].token != Token::LParen
    {
      return false;
    }

    // Stack: [..., receiver, member_ident].
    // Peek at receiver type (second from top).
    if self.ty_stack.len() < 2 {
      return false;
    }

    let receiver_ty = self.ty_stack[self.ty_stack.len() - 2];

    // Get the member name from the top of the stack.
    // It's an ident that was pushed but NOT as a value.
    // Check by looking at the tree: the ident before
    // the Dot in postfix order.
    let member_idx = dot_idx - 1;

    if member_idx >= self.tree.nodes.len()
      || self.tree.nodes[member_idx].token != Token::Ident
    {
      return false;
    }

    let member_name = match self.node_value(member_idx) {
      Some(NodeValue::Symbol(s)) => s,
      _ => return false,
    };

    // Array builtin methods + user-defined `apply []T`.
    let resolved = self.ty_checker.kind_of(receiver_ty);

    if matches!(resolved, Ty::Array(_)) {
      let ms = self.interner.get(member_name).to_owned();

      if ms == "push" || ms == "pop" {
        return true;
      }

      // Fall through to the generic dispatch below so
      // `apply []int { fun sum(self) }` style methods
      // resolve via `array_ty_name_str` →
      // `arr_int::sum`.
    }

    // Generic type param: if the method is an abstract method,
    // this is a valid method call (resolved at mono time).
    if matches!(resolved, Ty::Infer(_)) {
      let ms = self.interner.get(member_name).to_owned();

      let is_abstract_method = self
        .abstract_defs
        .values()
        .any(|d| d.methods.iter().any(|m| self.interner.get(m.name) == ms));

      if is_abstract_method {
        return true;
      }
    }

    // Resolve receiver type name for struct/enum methods,
    // or map a primitive `Ty` onto its canonical keyword
    // string for `apply <Primitive> { ... }` methods.
    let type_name = match resolved {
      Ty::Struct(sid) => {
        self.ty_checker.ty_table.struct_ty(sid).map(|s| s.name)
      }
      Ty::Enum(eid) => self.ty_checker.ty_table.enum_ty(eid).map(|e| e.name),
      _ => Self::primitive_ty_name_str(&resolved)
        .map(|s| self.interner.intern(s))
        .or_else(|| {
          // Array receiver: `[]int.method()` dispatches to
          // the `apply []int { fun method }` body — mangled
          // as `arr_int::method`.
          self
            .array_ty_name_str(&resolved)
            .map(|s| self.interner.intern(&s))
        }),
    };

    let type_name = match type_name {
      Some(n) => n,
      None => return false,
    };

    // Build mangled name and check if it's a function.
    let ts = self.interner.get(type_name);
    let ms = self.interner.get(member_name);
    let mangled = format!("{ts}::{ms}");

    self
      .interner
      .symbol(&mangled)
      .is_some_and(|sym| self.funs.iter().any(|f| f.name == sym))
  }

  /// Resolves a dot-call `receiver.method(args)` to the
  /// mangled name `Type::method`. Returns the mangled
  /// symbol if found, or the original method name.
  fn resolve_dot_call(
    &mut self,
    method_idx: usize,
    method_name: Symbol,
  ) -> Symbol {
    // The receiver ident is at method_idx - 2
    // (method_idx - 1 is Dot).
    if method_idx < 2 {
      return method_name;
    }

    let receiver_idx = method_idx - 2;

    // Get receiver's type from the local.
    let receiver_sym = match self.node_value(receiver_idx) {
      Some(NodeValue::Symbol(s)) => s,
      _ => return method_name,
    };

    let local_ty = self.lookup_local(receiver_sym).map(|l| l.ty_id);

    let ty_id = match local_ty {
      Some(t) => t,
      None => return method_name,
    };

    // Resolve the type to get the type name.
    let resolved = self.ty_checker.kind_of(ty_id);

    // Generic type param: emit placeholder for mono rewrite.
    if matches!(resolved, Ty::Infer(_)) {
      let ms = self.interner.get(method_name).to_owned();
      let placeholder = format!("__abstract::{ms}");

      return self.interner.intern(&placeholder);
    }

    let type_name = match resolved {
      Ty::Struct(sid) => {
        self.ty_checker.ty_table.struct_ty(sid).map(|s| s.name)
      }
      Ty::Enum(eid) => self.ty_checker.ty_table.enum_ty(eid).map(|e| e.name),
      _ => Self::primitive_ty_name_str(&resolved)
        .map(|s| self.interner.intern(s))
        .or_else(|| {
          // Array receiver: `[]int.method()` dispatches to
          // the `apply []int { fun method }` body — mangled
          // as `arr_int::method`.
          self
            .array_ty_name_str(&resolved)
            .map(|s| self.interner.intern(&s))
        }),
    };

    let type_name = match type_name {
      Some(n) => n,
      None => return method_name,
    };

    // Build mangled name.
    let ts = self.interner.get(type_name).to_owned();
    let ms = self.interner.get(method_name).to_owned();
    let mangled = format!("{ts}::{ms}");
    let mangled_sym = self.interner.intern(&mangled);

    // Check if it exists as a function.
    if self.funs.iter().any(|f| f.name == mangled_sym) {
      mangled_sym
    } else {
      method_name
    }
  }

  /// Like `resolve_dot_call` but with an explicit receiver
  /// index. Used when tree order is `[recv, method, .]`
  /// instead of `[recv, ., method]`.
  fn resolve_dot_call_with_receiver(
    &mut self,
    receiver_idx: usize,
    method_name: Symbol,
  ) -> Symbol {
    let receiver_sym = match self.node_value(receiver_idx) {
      Some(NodeValue::Symbol(s)) => s,
      _ => return method_name,
    };

    let local_ty = self.lookup_local(receiver_sym).map(|l| l.ty_id);

    let ty_id = match local_ty {
      Some(t) => t,
      None => return method_name,
    };

    let resolved = self.ty_checker.kind_of(ty_id);

    if matches!(resolved, Ty::Infer(_)) {
      let ms = self.interner.get(method_name).to_owned();
      let placeholder = format!("__abstract::{ms}");

      return self.interner.intern(&placeholder);
    }

    let type_name = match resolved {
      Ty::Struct(sid) => {
        self.ty_checker.ty_table.struct_ty(sid).map(|s| s.name)
      }
      Ty::Enum(eid) => self.ty_checker.ty_table.enum_ty(eid).map(|e| e.name),
      _ => Self::primitive_ty_name_str(&resolved)
        .map(|s| self.interner.intern(s))
        .or_else(|| {
          // Array receiver: `[]int.method()` dispatches to
          // the `apply []int { fun method }` body — mangled
          // as `arr_int::method`.
          self
            .array_ty_name_str(&resolved)
            .map(|s| self.interner.intern(&s))
        }),
    };

    let type_name = match type_name {
      Some(n) => n,
      None => return method_name,
    };

    let ts = self.interner.get(type_name).to_owned();
    let ms = self.interner.get(method_name).to_owned();
    let mangled = format!("{ts}::{ms}");
    let mangled_sym = self.interner.intern(&mangled);

    if self.funs.iter().any(|f| f.name == mangled_sym) {
      mangled_sym
    } else {
      method_name
    }
  }

  /// Executes a dot-call `receiver.method(args)`.
  /// The receiver is already on the stack (left by the
  /// Dot handler). Injects it as the first argument.
  fn execute_dot_method_call(
    &mut self,
    mangled_name: Symbol,
    lparen_idx: usize,
    rparen_idx: usize,
  ) {
    // Placeholder calls (__abstract::method) don't have a
    // matching FunDef. Emit the Call directly — the mono
    // pass will rewrite the name to the concrete method.
    let name_str = self.interner.get(mangled_name);

    if name_str.starts_with("__abstract::") {
      // Pop explicit args first, then receiver — same
      // order as the non-abstract dot-call path.
      let has_content = lparen_idx + 1 < rparen_idx;
      let mut comma_count = 0;

      for i in (lparen_idx + 1)..rparen_idx {
        if self.tree.nodes[i].token == Token::Comma {
          comma_count += 1;
        }
      }

      let explicit_args = if has_content { comma_count + 1 } else { 0 };

      let mut arg_sirs = Vec::with_capacity(explicit_args + 1);

      // Pop explicit args (reverse order from stack).
      for _ in 0..explicit_args {
        if let Some(sir) = self.sir_values.pop() {
          arg_sirs.push(sir);
        }
        self.value_stack.pop();
        self.ty_stack.pop();
      }

      arg_sirs.reverse();

      // Pop receiver (self).
      if let Some(recv) = self.sir_values.pop() {
        arg_sirs.insert(0, recv);
      }
      self.value_stack.pop();
      self.ty_stack.pop();

      let dst = ValueId(self.sir.next_value_id);
      self.sir.next_value_id += 1;

      // Use a generic return type — resolved at mono time.
      let ret_ty = self.ty_checker.fresh_var();

      let result_sir = self.sir.emit(Insn::Call {
        dst,
        name: mangled_name,
        args: arg_sirs,
        ty_id: ret_ty,
      });

      let rid = self.values.store_runtime(0);

      self.value_stack.push(rid);
      self.ty_stack.push(ret_ty);
      self.sir_values.push(result_sir);

      return;
    }

    let func = self.funs.iter().find(|f| f.name == mangled_name).cloned();

    let func = match func {
      Some(f) => f,
      None => return,
    };

    // Count explicit args between parens.
    let has_content = lparen_idx + 1 < rparen_idx;
    let mut comma_count = 0;

    for i in (lparen_idx + 1)..rparen_idx {
      if self.tree.nodes[i].token == Token::Comma {
        comma_count += 1;
      }
    }

    let explicit_args = if has_content { comma_count + 1 } else { 0 };

    // Pop explicit args from stack.
    let mut arg_sirs = Vec::with_capacity(explicit_args + 1);

    for _ in 0..explicit_args {
      self.value_stack.pop();
      self.ty_stack.pop();

      if let Some(sir) = self.sir_values.pop() {
        arg_sirs.push(sir);
      }
    }

    arg_sirs.reverse();

    // Pop receiver (self) — it's before the explicit
    // args on the stack.
    let receiver_sir = self.sir_values.pop();

    self.value_stack.pop();
    self.ty_stack.pop();

    // Prepend receiver as first arg.
    let mut full_args = Vec::with_capacity(arg_sirs.len() + 1);

    if let Some(r) = receiver_sir {
      full_args.push(r);
    }

    full_args.extend(arg_sirs);

    // Emit call.
    let dst = ValueId(self.sir.next_value_id);
    self.sir.next_value_id += 1;

    let result_sir = self.sir.emit(Insn::Call {
      dst,
      name: mangled_name,
      args: full_args,
      ty_id: func.return_ty,
    });

    if func.return_ty != self.ty_checker.unit_type() {
      let result_val = self.values.store_runtime(0);

      self.value_stack.push(result_val);
      self.ty_stack.push(func.return_ty);
      self.sir_values.push(result_sir);
    }
  }

  /// Executes `arr.push(value)` — emits `ArrayPush` SIR.
  /// Stack: [..., receiver, value]. Pops both.
  fn execute_array_push(&mut self, lparen_idx: usize, rparen_idx: usize) {
    // Count explicit args (must be exactly 1).
    let has_content = lparen_idx + 1 < rparen_idx;

    if !has_content {
      let span = self.tree.spans[rparen_idx];

      report_error(Error::new(ErrorKind::ArgumentCountMismatch, span));

      return;
    }

    // Pop the value argument.
    let (_val, _val_ty, val_sir) = match (
      self.value_stack.pop(),
      self.ty_stack.pop(),
      self.sir_values.pop(),
    ) {
      (Some(v), Some(t), Some(s)) => (v, t, s),
      _ => return,
    };

    // Pop the receiver (array).
    let (_arr, arr_ty, arr_sir) = match (
      self.value_stack.pop(),
      self.ty_stack.pop(),
      self.sir_values.pop(),
    ) {
      (Some(v), Some(t), Some(s)) => (v, t, s),
      _ => return,
    };

    self.sir.emit(Insn::ArrayPush {
      array: arr_sir,
      value: val_sir,
      ty_id: arr_ty,
    });
  }

  /// Executes `val = arr.pop()` — emits `ArrayPop` SIR.
  /// Stack: [..., receiver]. No explicit args.
  fn execute_array_pop(&mut self, _lparen_idx: usize, _rparen_idx: usize) {
    // Pop the receiver (array).
    let (_arr, arr_ty, arr_sir) = match (
      self.value_stack.pop(),
      self.ty_stack.pop(),
      self.sir_values.pop(),
    ) {
      (Some(v), Some(t), Some(s)) => (v, t, s),
      _ => return,
    };

    // Resolve element type from array type.
    let elem_ty = if let Ty::Array(aid) = self.ty_checker.kind_of(arr_ty)
      && let Some(at) = self.ty_checker.ty_table.array(aid)
    {
      at.elem_ty
    } else {
      self.ty_checker.int_type()
    };

    let dst = ValueId(self.sir.next_value_id);

    self.sir.next_value_id += 1;

    let sv = self.sir.emit(Insn::ArrayPop {
      dst,
      array: arr_sir,
      ty_id: elem_ty,
    });

    let rid = self.values.store_runtime(0);

    self.value_stack.push(rid);
    self.ty_stack.push(elem_ty);
    self.sir_values.push(sv);
  }

  /// Desugars `expr?` into:
  ///   load discriminant → compare against Ok (0)
  ///   → if Ok: extract field[1], push value
  ///   → if Err: wrap in Err, return
  fn execute_try_operator(&mut self, _idx: usize) {
    // Pop the Result/Option value from stacks.
    let (_val, val_ty, val_sir) = match (
      self.value_stack.pop(),
      self.ty_stack.pop(),
      self.sir_values.pop(),
    ) {
      (Some(v), Some(t), Some(s)) => (v, t, s),
      _ => return,
    };

    let int_ty = self.ty_checker.int_type();
    let ok_label = self.sir.next_label();
    let done_label = self.sir.next_label();

    // Read discriminant: TupleIndex(scrutinee, 0).
    let disc_dst = ValueId(self.sir.next_value_id);
    self.sir.next_value_id += 1;

    let disc_sir = self.sir.emit(Insn::TupleIndex {
      dst: disc_dst,
      tuple: val_sir,
      index: 0,
      ty_id: int_ty,
    });

    // Compare against Ok/Some discriminant (0).
    let zero_dst = ValueId(self.sir.next_value_id);
    self.sir.next_value_id += 1;

    let zero_sir = self.sir.emit(Insn::ConstInt {
      dst: zero_dst,
      value: 0,
      ty_id: int_ty,
    });

    let cmp_dst = ValueId(self.sir.next_value_id);
    self.sir.next_value_id += 1;

    let cmp_sir = self.sir.emit(Insn::BinOp {
      dst: cmp_dst,
      op: zo_sir::BinOp::Eq,
      lhs: disc_sir,
      rhs: zero_sir,
      ty_id: int_ty,
    });

    // Branch: if NOT Ok → error path.
    self.sir.emit(Insn::BranchIfNot {
      cond: cmp_sir,
      target: ok_label, // reuse as "err" label
    });

    // Ok path: extract field[1] (the payload).
    // Resolve the Ok payload type from the enum.
    let payload_ty = if let Ty::Enum(eid) = self.ty_checker.kind_of(val_ty)
      && let Some(et) = self.ty_checker.ty_table.enum_ty(eid)
    {
      let et = *et;
      let variants = self.ty_checker.ty_table.enum_variants(&et).to_vec();

      // Ok/Some is variant 0, field 0 of that variant.
      if !variants.is_empty() && variants[0].field_count > 0 {
        let field_tys = self.ty_checker.ty_table.variant_fields(&variants[0]);

        if !field_tys.is_empty() {
          field_tys[0]
        } else {
          int_ty
        }
      } else {
        int_ty
      }
    } else {
      int_ty
    };

    let ok_dst = ValueId(self.sir.next_value_id);
    self.sir.next_value_id += 1;

    let ok_sir = self.sir.emit(Insn::TupleIndex {
      dst: ok_dst,
      tuple: val_sir,
      index: 1,
      ty_id: payload_ty,
    });

    // Jump past the error path.
    self.sir.emit(Insn::Jump { target: done_label });

    // Error path: return the original value as-is.
    // (It's already Err(...) or None — just return it.)
    self.sir.emit(Insn::Label { id: ok_label });
    self.sir.emit(Insn::Return {
      value: Some(val_sir),
      ty_id: val_ty,
    });

    // Done label: Ok path continues here.
    self.sir.emit(Insn::Label { id: done_label });

    // Push the unwrapped Ok payload onto stacks.
    let rid = self.values.store_runtime(0);

    self.value_stack.push(rid);
    self.ty_stack.push(payload_ty);
    self.sir_values.push(ok_sir);
  }

  /// Returns `(sink_name, outer_sink)` where:
  ///
  /// - `sink_name = Some(sym)` when the next branching
  ///   construct is about to be pushed in expression
  ///   position — i.e. its result will be consumed by
  ///   an enclosing `imu` init, function return, outer
  ///   branch sink, or similar. `None` for
  ///   statement-position branches.
  /// - `outer_sink = Some(sym)` echoes the enclosing
  ///   branch's sink when the new branch is a nested
  ///   expression producing into it. currently only
  ///   used for documentation; the arm-exit helper
  ///   reads from `branch_stack` directly.
  ///
  /// expression-position triggers:
  /// - a pending variable declaration (`imu y =
  ///   <branch>;`, `mut y = ...`, `val y = ...`).
  /// - the enclosing function expects a non-unit
  ///   return AND the branch is the tail expression.
  ///   we approximate this by the function's
  ///   declared return type being non-unit; false
  ///   positives are harmless (an unused sink emits
  ///   one extra Store + Load).
  /// - an enclosing `BranchCtx` that already has a
  ///   `value_sink` — nested branches produce into
  ///   the outer result position.
  fn mint_branch_sink_if_expr_position(
    &mut self,
  ) -> (Option<Symbol>, Option<Symbol>) {
    let outer_sink = self.branch_stack.iter().rev().find_map(|c| c.value_sink);

    let unit_ty = self.ty_checker.unit_type();

    let in_expr_pos = self.pending_decl.is_some()
      || outer_sink.is_some()
      || self
        .current_function
        .as_ref()
        .is_some_and(|f| f.return_ty != unit_ty);

    if !in_expr_pos {
      return (None, outer_sink);
    }

    let n = self.branch_result_counter;

    self.branch_result_counter += 1;

    let sym = self.interner.intern(&format!("__branch_result_{n}__"));

    (Some(sym), outer_sink)
  }

  /// Emit a `Store` from the current top-of-stack into
  /// a branch's value sink, consuming that top. no-op
  /// when the sink is absent or the stacks are empty
  /// (the arm exited via return / break / continue and
  /// already consumed its value).
  fn emit_branch_sink_store(&mut self, ctx_idx: usize) {
    let ctx = match self.branch_stack.get(ctx_idx) {
      Some(c) => c.clone(),
      None => return,
    };

    let sink = match ctx.value_sink {
      Some(s) => s,
      None => return,
    };

    // Only store if the arm actually produced a fresh
    // value on top of the stack. If `sir_values.len()`
    // is the SAME (or less) than when the branch was
    // entered, the arm ran purely for side effects
    // (`high = mid - 1;`, etc.) and the current top
    // belongs to an enclosing construct (e.g. the
    // parent while's condition). Popping it here would
    // silently corrupt the parent.
    if (self.sir_values.len() as u32) <= ctx.stack_depth_at_entry {
      return;
    }

    let top_sir = match self.sir_values.last().copied() {
      Some(s) => s,
      None => return,
    };

    let top_ty = match self.ty_stack.last().copied() {
      Some(t) => t,
      None => return,
    };

    // First arm to Store sets the sink type; later arms
    // unify against it. type mismatch reports through
    // the existing error path.
    let sink_ty = if let Some(prev) = ctx.value_sink_ty {
      let span = zo_span::Span::ZERO;
      self.ty_checker.unify(prev, top_ty, span).unwrap_or(prev)
    } else {
      if let Some(c) = self.branch_stack.get_mut(ctx_idx) {
        c.value_sink_ty = Some(top_ty);
      }

      top_ty
    };

    self.sir.emit(Insn::Store {
      name: sink,
      value: top_sir,
      ty_id: sink_ty,
    });

    self.value_stack.pop();
    self.ty_stack.pop();
    self.sir_values.pop();
  }

  /// Emit a `Load` from a branch's value sink at the
  /// merge point, pushing the loaded value onto the
  /// stacks as the branch expression's result. no-op
  /// when the sink is absent.
  fn emit_branch_sink_load(&mut self, ctx: &BranchCtx) {
    let sink = match ctx.value_sink {
      Some(s) => s,
      None => return,
    };

    // No arm ever stored — the branch was allocated a
    // sink eagerly (non-unit function, etc.) but the
    // arms never produced a value (pure statements).
    // Skip the Load entirely; emitting a Load against
    // an uninitialised slot pushes garbage onto the
    // stacks and corrupts downstream.
    let ty = match ctx.value_sink_ty {
      Some(t) => t,
      None => return,
    };

    let dst = ValueId(self.sir.next_value_id);

    self.sir.next_value_id += 1;

    let sv = self.sir.emit(Insn::Load {
      dst,
      src: LoadSource::Local(sink),
      ty_id: ty,
    });

    let rid = self.values.store_runtime(0);

    self.value_stack.push(rid);
    self.ty_stack.push(ty);
    self.sir_values.push(sv);
  }

  fn execute_if(&mut self, _start_idx: usize, _end_idx: usize) {
    let end_label = self.sir.next_label();
    let else_label = self.sir.next_label();

    let (value_sink, _existing_outer) =
      self.mint_branch_sink_if_expr_position();

    self.branch_stack.push(BranchCtx {
      kind: BranchKind::If,
      end_label,
      else_label: Some(else_label),
      loop_label: None,
      branch_emitted: false,
      for_var: None,
      scope_depth: self.scope_stack.len(),
      value_sink,
      value_sink_ty: None,
      stack_depth_at_entry: self.sir_values.len() as u32,
    });
  }

  /// Lower `match scrutinee { pat => body, ... }` to a
  /// cmp-chain of if-else-if-else lines in SIR. Slice 1 of
  /// `PLAN_MATCH` — literal int patterns + wildcard only, arm
  /// bodies run for side effects (no result-value
  /// unification). Enum patterns, string/bool literals, and
  /// match-as-expression are follow-up slices.
  ///
  /// Tree layout after `handle_match_keyword`:
  /// ```text
  ///   Match (idx)
  ///     <scrutinee expression nodes>
  ///     LBrace
  ///       <pat, FatArrow, body..., Comma>*
  ///     RBrace
  /// ```
  fn execute_match(&mut self, start_idx: usize, end_idx: usize) {
    // Provisional skip — the main loop must not re-visit the
    // match's nodes after we return. Tightened below to
    // `rbrace_idx + 1` once we locate the match's own `}`.
    // Without the tightening, parser trees where `Match`'s
    // child span reaches past the match's `}` (e.g. when a
    // guard arm's `If` sub-node inflates the sibling count)
    // would cause execute_match to swallow the enclosing
    // block's `}` — main would never emit its epilogue
    // `Return` and the binary SIGILL'd.
    self.skip_until = end_idx;

    // -- 1. Locate the LBrace that opens the arm block ------
    let lbrace_idx = match (start_idx + 1..end_idx)
      .find(|&j| self.tree.nodes[j].token == Token::LBrace)
    {
      Some(i) => i,
      None => return,
    };

    // -- 2. Locate the matching RBrace at depth 0 -----------
    let mut depth = 1_i32;
    let mut rbrace_idx = end_idx;

    for j in (lbrace_idx + 1)..end_idx {
      match self.tree.nodes[j].token {
        Token::LBrace => depth += 1,
        Token::RBrace => {
          depth -= 1;
          if depth == 0 {
            rbrace_idx = j;
            break;
          }
        }
        _ => {}
      }
    }

    // Tighten skip range: resume the outer loop AT the first
    // node past the match's own `}` — anything beyond that is
    // an outer sibling (the enclosing block's `}`, the next
    // statement, …) that we must NOT swallow.
    if rbrace_idx < end_idx {
      self.skip_until = rbrace_idx + 1;
    }

    // -- 3. Execute the scrutinee expression ----------------
    // Stream each scrutinee node through `execute_node` so
    // its sir_values top is the scrutinee's SIR value at the
    // end. Same pattern `execute_closure` already uses for
    // its body range.
    let saved_skip = self.skip_until;

    self.skip_until = 0;

    let stack_before = self.sir_values.len();

    for i in (start_idx + 1)..lbrace_idx {
      if i < self.skip_until {
        continue;
      }

      let node = self.tree.nodes[i];

      self.execute_node(&node, i);
    }

    self.skip_until = saved_skip;

    // Pop the scrutinee's value off the three stacks. We
    // capture its **symbol** (for re-loading per arm) and type,
    // NOT the single ValueId. Reusing one ValueId across all
    // arms breaks the register allocator's liveness tracking —
    // it frees the scrutinee's register after the first CMP,
    // and the second arm's pattern constant overwrites it.
    // Emitting a fresh `Insn::Load` per arm gives each a
    // dedicated ValueId with correct local liveness.
    let scrutinee_ty = self
      .ty_stack
      .last()
      .copied()
      .unwrap_or(self.ty_checker.int_type());

    // Determine the scrutinee's backing symbol for per-arm
    // reloads by inspecting what SIR the scrutinee walk left
    // behind. Two cases:
    //   1. The scrutinee is a BARE stored local — the walk's
    //      tail Insn is `Load { src: Local(sym) }` and a prior
    //      `Store { name: sym }` exists. Reuse `sym` directly
    //      so we don't redundantly spill.
    //   2. Everything else (literal, parameter, tuple literal,
    //      BinOp, call, index expr, ...) — materialize the
    //      top-of-stack SIR value into a synthetic local
    //      `__match_scrut__` so per-arm `Load`s have a
    //      well-defined source with the correct (possibly
    //      compound) scrutinee type.
    //
    // The earlier heuristic — "first Ident in the scrutinee
    // token range" — misfired on compound scrutinees like
    // `match (a, b)` where it picked `a` (a scalar int) as the
    // scrutinee symbol, then per-arm `TupleIndex` read the
    // wrong memory and every arm silently failed.
    let tail_load_sym = match self.sir.instructions.last() {
      Some(Insn::Load {
        src: LoadSource::Local(sym),
        ..
      }) => Some(*sym),
      _ => None,
    };

    let scrutinee_sym = if let Some(sym) = tail_load_sym
      && self
        .sir
        .instructions
        .iter()
        .any(|i| matches!(i, Insn::Store { name, .. } if *name == sym))
    {
      Some(sym)
    } else if let Some(sir_val) = self.sir_values.last().copied() {
      let scrut_sym = self.interner.intern("__match_scrut__");

      self.sir.emit(Insn::Store {
        name: scrut_sym,
        value: sir_val,
        ty_id: scrutinee_ty,
      });

      Some(scrut_sym)
    } else {
      None
    };

    while self.sir_values.len() > stack_before {
      self.sir_values.pop();
      self.value_stack.pop();
      self.ty_stack.pop();
    }

    // -- 4. Walk the arms ------------------------------------
    // Detect if the match is in expression position (result
    // will be consumed by a pending declaration or as a
    // function's implicit return).
    let is_expr_match = self.pending_decl.is_some()
      || self
        .current_function
        .as_ref()
        .is_some_and(|f| f.return_ty != self.ty_checker.unit_type());

    let end_label = self.sir.next_label();
    let mut arm_idx = lbrace_idx + 1;
    let mut match_result_ty: Option<TyId> = None;
    let mut match_result_sym: Option<Symbol> = None;

    while arm_idx < rbrace_idx {
      // Skip any stray comma from the previous arm.
      while arm_idx < rbrace_idx
        && self.tree.nodes[arm_idx].token == Token::Comma
      {
        arm_idx += 1;
      }

      if arm_idx >= rbrace_idx {
        break;
      }

      // Pattern is the first node; find the FatArrow that
      // separates it from the body.
      let pat_idx = arm_idx;
      let mut arrow_idx = None;

      for j in pat_idx..rbrace_idx {
        if self.tree.nodes[j].token == Token::FatArrow {
          arrow_idx = Some(j);
          break;
        }
      }

      let arrow_idx = match arrow_idx {
        Some(i) => i,
        None => break,
      };

      // Body range: arrow_idx + 1 .. next top-level Comma or
      // rbrace_idx. Top-level = depth 0 inside the arm block.
      let mut body_depth = 0_i32;
      let mut body_end = rbrace_idx;

      for j in (arrow_idx + 1)..rbrace_idx {
        let tok = self.tree.nodes[j].token;

        match tok {
          Token::LParen | Token::LBrace | Token::LBracket => body_depth += 1,
          Token::RParen | Token::RBrace | Token::RBracket => body_depth -= 1,
          Token::Comma if body_depth == 0 => {
            body_end = j;
            break;
          }
          _ => {}
        }
      }

      // -- Emit the arm ------------------------------------
      let pat_tok = self.tree.nodes[pat_idx].token;
      let is_wildcard = pat_tok == Token::Ident
        && matches!(
          self.node_value(pat_idx),
          Some(NodeValue::Symbol(s)) if s == Symbol::UNDERSCORE
        );

      let next_arm_label = self.sir.next_label();

      // Number of locals introduced by this arm's pattern.
      // Popped after the body executes.
      let mut arm_bindings: u32 = 0;

      // Detect enum variant pattern: Ident :: Ident [( bindings )]
      let is_enum_pat = !is_wildcard
        && pat_tok == Token::Ident
        && pat_idx + 2 < arrow_idx
        && self.tree.nodes[pat_idx + 1].token == Token::ColonColon
        && self.tree.nodes[pat_idx + 2].token == Token::Ident;

      // Detect tuple pattern: `(a, b, ..)`. LParen opens a
      // tuple pattern only at pattern position — parameter
      // lists, calls, and unary grouping never appear here.
      let is_tuple_pat = !is_wildcard && pat_tok == Token::LParen;

      if !is_wildcard
        && matches!(
          pat_tok,
          Token::Int
            | Token::Char
            | Token::Bytes
            | Token::Float
            | Token::True
            | Token::False
            | Token::String
        )
      {
        // Primitive/string literal pattern: emit reload,
        // const, compare, branch.
        let scrut_dst = ValueId(self.sir.next_value_id);
        self.sir.next_value_id += 1;

        let scrut_reload = if let Some(sym) = scrutinee_sym {
          self.sir.emit(Insn::Load {
            dst: scrut_dst,
            src: LoadSource::Local(sym),
            ty_id: scrutinee_ty,
          })
        } else {
          scrut_dst
        };

        let pat_dst = ValueId(self.sir.next_value_id);
        self.sir.next_value_id += 1;

        let pat_sir = match pat_tok {
          Token::Int => {
            let value = match self.node_value(pat_idx) {
              Some(NodeValue::Literal(lit)) => {
                self.literals.int_literals[lit as usize]
              }
              _ => 0,
            };

            self.sir.emit(Insn::ConstInt {
              dst: pat_dst,
              value,
              ty_id: scrutinee_ty,
            })
          }
          Token::Char => {
            let value = match self.node_value(pat_idx) {
              Some(NodeValue::Literal(lit)) => {
                self.literals.char_literals[lit as usize] as u64
              }
              _ => 0,
            };

            self.sir.emit(Insn::ConstInt {
              dst: pat_dst,
              value,
              ty_id: self.ty_checker.char_type(),
            })
          }
          Token::Bytes => {
            let value = match self.node_value(pat_idx) {
              Some(NodeValue::Literal(lit)) => {
                self.literals.bytes_literals[lit as usize] as u64
              }
              _ => 0,
            };

            self.sir.emit(Insn::ConstInt {
              dst: pat_dst,
              value,
              ty_id: self.ty_checker.bytes_type(),
            })
          }
          Token::Float => {
            let value = match self.node_value(pat_idx) {
              Some(NodeValue::Literal(lit)) => {
                self.literals.float_literals[lit as usize]
              }
              _ => 0.0,
            };

            self.sir.emit(Insn::ConstFloat {
              dst: pat_dst,
              value,
              ty_id: scrutinee_ty,
            })
          }
          Token::True => self.sir.emit(Insn::ConstBool {
            dst: pat_dst,
            value: true,
            ty_id: self.ty_checker.bool_type(),
          }),
          Token::False => self.sir.emit(Insn::ConstBool {
            dst: pat_dst,
            value: false,
            ty_id: self.ty_checker.bool_type(),
          }),
          Token::String => {
            let symbol = match self.node_value(pat_idx) {
              Some(NodeValue::Literal(lit)) => {
                self.literals.identifiers[lit as usize]
              }
              Some(NodeValue::Symbol(sym)) => sym,
              _ => self.interner.intern(""),
            };

            self.sir.emit(Insn::ConstString {
              dst: pat_dst,
              symbol,
              ty_id: self.ty_checker.str_type(),
            })
          }
          _ => pat_dst,
        };

        let cmp_dst = ValueId(self.sir.next_value_id);
        self.sir.next_value_id += 1;

        let cmp_sir = self.sir.emit(Insn::BinOp {
          dst: cmp_dst,
          op: zo_sir::BinOp::Eq,
          lhs: scrut_reload,
          rhs: pat_sir,
          ty_id: scrutinee_ty,
        });

        self.sir.emit(Insn::BranchIfNot {
          cond: cmp_sir,
          target: next_arm_label,
        });
      } else if is_enum_pat {
        // Resolve enum name + variant name.
        let enum_sym = match self.node_value(pat_idx) {
          Some(NodeValue::Symbol(s)) => s,
          _ => {
            arm_idx = body_end;
            continue;
          }
        };
        let var_sym = match self.node_value(pat_idx + 2) {
          Some(NodeValue::Symbol(s)) => s,
          _ => {
            arm_idx = body_end;
            continue;
          }
        };

        // Look up the enum definition and variant.
        // Trigger lazy import if this is the first
        // reference to an imported enum (e.g. Result).
        let enum_name_str = self.interner.get(enum_sym).to_owned();
        let mut entry = self
          .enum_defs
          .iter()
          .find(|e| {
            let n = self.interner.get(e.0);
            n == enum_name_str || n.starts_with(&format!("{enum_name_str}__"))
          })
          .copied();

        if entry.is_none()
          && let Some(pos) = self
            .pending_imported_enums
            .iter()
            .position(|e| self.interner.get(e.name) == enum_name_str)
        {
          let en = self.pending_imported_enums.remove(pos);

          self.ty_checker.push_scope();

          let fresh_variants: Vec<(Symbol, u32, Vec<TyId>)> = en
            .variants
            .iter()
            .map(|(name, disc, fields)| {
              let pf: Vec<TyId> =
                fields.iter().map(|_| self.ty_checker.fresh_var()).collect();
              (*name, *disc, pf)
            })
            .collect();

          self.ty_checker.pop_scope();

          let ety_id = self
            .ty_checker
            .ty_table
            .intern_enum(en.name, &fresh_variants);
          let ty_id = self.ty_checker.intern_ty(zo_ty::Ty::Enum(ety_id));

          self.enum_defs.push((en.name, ety_id, ty_id));

          self.sir.emit(Insn::EnumDef {
            name: en.name,
            ty_id,
            variants: fresh_variants,
            pubness: zo_value::Pubness::No,
          });

          entry = Some((en.name, ety_id, ty_id));
        }

        let entry = entry;
        let (_, ety_id, _) = match entry {
          Some(e) => e,
          None => {
            arm_idx = body_end;
            continue;
          }
        };

        let enum_ty = match self.ty_checker.ty_table.enum_ty(ety_id) {
          Some(et) => *et,
          None => {
            arm_idx = body_end;
            continue;
          }
        };

        let var_str = self.interner.get(var_sym).to_owned();
        let variants = self.ty_checker.ty_table.enum_variants(&enum_ty);
        let variant = match variants
          .iter()
          .find(|v| self.interner.get(v.name) == var_str)
        {
          Some(v) => *v,
          None => {
            arm_idx = body_end;
            continue;
          }
        };

        // Fresh Load of the scrutinee pointer.
        let scrut_dst = ValueId(self.sir.next_value_id);
        self.sir.next_value_id += 1;

        let scrut_reload = if let Some(sym) = scrutinee_sym {
          self.sir.emit(Insn::Load {
            dst: scrut_dst,
            src: LoadSource::Local(sym),
            ty_id: scrutinee_ty,
          })
        } else {
          scrut_dst
        };

        // Read discriminant from [scrutinee, 0].
        let int_ty = self.ty_checker.int_type();
        let disc_dst = ValueId(self.sir.next_value_id);
        self.sir.next_value_id += 1;

        let disc_sir = self.sir.emit(Insn::TupleIndex {
          dst: disc_dst,
          tuple: scrut_reload,
          index: 0,
          ty_id: int_ty,
        });

        // Compare against expected discriminant.
        let exp_dst = ValueId(self.sir.next_value_id);
        self.sir.next_value_id += 1;

        let exp_sir = self.sir.emit(Insn::ConstInt {
          dst: exp_dst,
          value: variant.discriminant as u64,
          ty_id: int_ty,
        });

        let cmp_dst = ValueId(self.sir.next_value_id);
        self.sir.next_value_id += 1;

        let cmp_sir = self.sir.emit(Insn::BinOp {
          dst: cmp_dst,
          op: zo_sir::BinOp::Eq,
          lhs: disc_sir,
          rhs: exp_sir,
          ty_id: int_ty,
        });

        self.sir.emit(Insn::BranchIfNot {
          cond: cmp_sir,
          target: next_arm_label,
        });

        // Tuple variant bindings: extract payload fields and
        // introduce them as locals so the arm body can use
        // them (e.g. `Loot::Gold(n) => showln(n)`).
        if variant.field_count > 0
          && pat_idx + 3 < arrow_idx
          && self.tree.nodes[pat_idx + 3].token == Token::LParen
        {
          // Use return_type_args from the ext function
          // if available (e.g. Result<str, int> → [str, int]),
          // otherwise fall back to the enum's generic field
          // types.
          // Look up return_type_args by variable symbol first,
          // then fall back to the enum's type name. User functions
          // store rta under the type name (e.g. "Result").
          let rta_key = scrutinee_sym
            .and_then(|s| self.var_return_type_args.get(&s.as_u32()))
            .or_else(|| self.var_return_type_args.get(&enum_ty.name.as_u32()));

          let field_tys = if let Some(rta) = rta_key {
            // Compute type arg offset for this variant:
            // sum of all preceding variants' field counts.
            let all_variants =
              self.ty_checker.ty_table.enum_variants(&enum_ty).to_vec();
            let var_offset: usize = all_variants
              .iter()
              .take_while(|v| v.discriminant != variant.discriminant)
              .map(|v| v.field_count as usize)
              .sum();

            (0..variant.field_count as usize)
              .map(|i| {
                let ty = rta.get(var_offset + i);

                match ty {
                  Some(Ty::Str) => self.ty_checker.str_type(),
                  Some(Ty::Bool) => self.ty_checker.bool_type(),
                  Some(Ty::Int { .. }) => self.ty_checker.int_type(),
                  Some(ty) => self.ty_checker.intern_ty(*ty),
                  None => self.ty_checker.int_type(),
                }
              })
              .collect::<Vec<_>>()
          } else {
            // Resolve through substitutions — generic enums
            // have inference variables that may have been
            // unified with concrete types during the call.
            let raw_fields =
              self.ty_checker.ty_table.variant_fields(&variant).to_vec();

            raw_fields
              .iter()
              .map(|ty_id| {
                let resolved = self.ty_checker.resolve_id(*ty_id);

                let ty = self.ty_checker.resolve_ty(resolved);

                match ty {
                  Ty::Str => self.ty_checker.str_type(),
                  Ty::Bool => self.ty_checker.bool_type(),
                  Ty::Char => self.ty_checker.char_type(),
                  Ty::Bytes => self.ty_checker.bytes_type(),
                  Ty::Int { .. } => self.ty_checker.int_type(),
                  Ty::Float(_) => self.ty_checker.intern_ty(ty),
                  _ => resolved,
                }
              })
              .collect::<Vec<_>>()
          };
          let mut bind_idx = pat_idx + 4;
          let mut field_i: u32 = 0;

          while bind_idx < arrow_idx && field_i < variant.field_count {
            let tok = self.tree.nodes[bind_idx].token;

            if tok == Token::RParen {
              break;
            }
            if tok == Token::Comma {
              bind_idx += 1;
              continue;
            }

            // Reject type keywords (str, int, bytes, etc.)
            // as binding names — the tokenizer turns them
            // into type tokens, making them unusable as
            // variable references in the arm body.
            if tok.is_ty() {
              let span = self.tree.spans[bind_idx];

              report_error(Error::new(ErrorKind::ExpectedIdentifier, span));

              bind_idx += 1;
              field_i += 1;

              continue;
            }

            let bind_sym = if tok == Token::Ident {
              match self.node_value(bind_idx) {
                Some(NodeValue::Symbol(s)) => Some(s),
                _ => None,
              }
            } else {
              None
            };

            if let Some(bind_sym) = bind_sym {
              let field_ty =
                field_tys.get(field_i as usize).copied().unwrap_or(int_ty);

              // TupleIndex to read the field (index = field_i + 1
              // because slot 0 is the discriminant).
              let field_dst = ValueId(self.sir.next_value_id);
              self.sir.next_value_id += 1;

              let field_sir = self.sir.emit(Insn::TupleIndex {
                dst: field_dst,
                tuple: scrut_reload,
                index: field_i + 1,
                ty_id: field_ty,
              });

              // Introduce as a local + emit VarDef + Store so
              // the codegen's Load handler can find it in
              // mutable_slots.
              self.sir.emit(Insn::VarDef {
                name: bind_sym,
                ty_id: field_ty,
                init: Some(field_sir),
                mutability: Mutability::No,
                pubness: Pubness::No,
              });

              self.sir.emit(Insn::Store {
                name: bind_sym,
                value: field_sir,
                ty_id: field_ty,
              });

              let rid = self.values.store_runtime(0);

              self.locals.push(Local {
                name: bind_sym,
                ty_id: field_ty,
                value_id: rid,
                pubness: Pubness::No,
                mutability: Mutability::No,
                sir_value: Some(field_sir),
                local_kind: LocalKind::Variable,
              });

              arm_bindings += 1;
              field_i += 1;
            }

            bind_idx += 1;
          }
        }
      } else if is_tuple_pat {
        // Tuple pattern `(p0, p1, ..)`. Emit per-field
        // compare + branch: any field mismatch skips to the
        // next arm. `_` fields don't contribute a compare
        // (they match anything). Non-literal / non-wildcard
        // fields are not yet supported — treat them as
        // wildcards so at worst the arm over-matches instead
        // of misbehaving.
        //
        // Find the matching RParen at depth 0 inside the
        // pattern so we can walk the top-level fields.
        let mut tp_depth = 1_i32;
        let mut tp_end = arrow_idx;

        for j in (pat_idx + 1)..arrow_idx {
          match self.tree.nodes[j].token {
            Token::LParen | Token::LBracket => tp_depth += 1,
            Token::RParen | Token::RBracket => {
              tp_depth -= 1;

              if tp_depth == 0 {
                tp_end = j;

                break;
              }
            }
            _ => {}
          }
        }

        // Collect field node indices at depth 0 between
        // pat_idx + 1 and tp_end. Each field is a single
        // token (literal or `_`) — patterns like `(1 + 2, 3)`
        // are not supported and will land on the fallback
        // wildcard below.
        let mut field_idxs: Vec<usize> = Vec::new();
        let mut f_depth = 0_i32;

        for j in (pat_idx + 1)..tp_end {
          let tok = self.tree.nodes[j].token;

          match tok {
            Token::LParen | Token::LBracket => {
              f_depth += 1;

              continue;
            }
            Token::RParen | Token::RBracket => {
              f_depth -= 1;

              continue;
            }
            Token::Comma if f_depth == 0 => continue,
            _ => {}
          }

          if f_depth == 0 {
            field_idxs.push(j);
          }
        }

        let int_ty = self.ty_checker.int_type();

        // Resolve the per-field scrutinee type from the
        // tuple's element list. Fall back to `int` if we
        // can't resolve (keeps old behaviour for untyped
        // paths).
        let elem_tys: Vec<TyId> = {
          let ty = self.ty_checker.resolve_ty(scrutinee_ty);

          if let zo_ty::Ty::Tuple(tt_id) = ty
            && let Some(tt) = self.ty_checker.ty_table.tuple(tt_id)
          {
            self.ty_checker.ty_table.tuple_elems(tt).to_vec()
          } else {
            Vec::new()
          }
        };

        for (slot, &f_idx) in field_idxs.iter().enumerate() {
          let f_tok = self.tree.nodes[f_idx].token;

          // Wildcard `_` or a plain Ident binding: no
          // comparison. Ident bindings other than `_` are
          // not yet wired through — treat them as wildcards
          // so arms don't misbehave. This mirrors how the
          // scalar pattern path handles `_`.
          let is_field_wildcard = f_tok == Token::Ident
            && matches!(
              self.node_value(f_idx),
              Some(NodeValue::Symbol(s)) if s == Symbol::UNDERSCORE
            );

          let is_field_ident = f_tok == Token::Ident && !is_field_wildcard;

          if is_field_wildcard || is_field_ident {
            continue;
          }

          // Only literal field kinds produce a compare.
          if !matches!(
            f_tok,
            Token::Int
              | Token::Char
              | Token::Bytes
              | Token::Float
              | Token::True
              | Token::False
              | Token::String
          ) {
            continue;
          }

          let field_ty = elem_tys.get(slot).copied().unwrap_or(int_ty);

          // Reload the scrutinee pointer fresh per field so
          // the register allocator sees a clean liveness.
          let scrut_dst = ValueId(self.sir.next_value_id);
          self.sir.next_value_id += 1;

          let scrut_reload = if let Some(sym) = scrutinee_sym {
            self.sir.emit(Insn::Load {
              dst: scrut_dst,
              src: LoadSource::Local(sym),
              ty_id: scrutinee_ty,
            })
          } else {
            scrut_dst
          };

          // Read the tuple field at `slot` — plain tuples
          // have no discriminant so slot maps 1:1.
          let field_dst = ValueId(self.sir.next_value_id);
          self.sir.next_value_id += 1;

          let field_sir = self.sir.emit(Insn::TupleIndex {
            dst: field_dst,
            tuple: scrut_reload,
            index: slot as u32,
            ty_id: field_ty,
          });

          // Build the pattern constant.
          let pat_dst = ValueId(self.sir.next_value_id);
          self.sir.next_value_id += 1;

          let pat_sir = match f_tok {
            Token::Int => {
              let value = match self.node_value(f_idx) {
                Some(NodeValue::Literal(lit)) => {
                  self.literals.int_literals[lit as usize]
                }
                _ => 0,
              };

              self.sir.emit(Insn::ConstInt {
                dst: pat_dst,
                value,
                ty_id: field_ty,
              })
            }
            Token::Char => {
              let value = match self.node_value(f_idx) {
                Some(NodeValue::Literal(lit)) => {
                  self.literals.char_literals[lit as usize] as u64
                }
                _ => 0,
              };

              self.sir.emit(Insn::ConstInt {
                dst: pat_dst,
                value,
                ty_id: self.ty_checker.char_type(),
              })
            }
            Token::Bytes => {
              let value = match self.node_value(f_idx) {
                Some(NodeValue::Literal(lit)) => {
                  self.literals.bytes_literals[lit as usize] as u64
                }
                _ => 0,
              };

              self.sir.emit(Insn::ConstInt {
                dst: pat_dst,
                value,
                ty_id: self.ty_checker.bytes_type(),
              })
            }
            Token::Float => {
              let value = match self.node_value(f_idx) {
                Some(NodeValue::Literal(lit)) => {
                  self.literals.float_literals[lit as usize]
                }
                _ => 0.0,
              };

              self.sir.emit(Insn::ConstFloat {
                dst: pat_dst,
                value,
                ty_id: field_ty,
              })
            }
            Token::True => self.sir.emit(Insn::ConstBool {
              dst: pat_dst,
              value: true,
              ty_id: self.ty_checker.bool_type(),
            }),
            Token::False => self.sir.emit(Insn::ConstBool {
              dst: pat_dst,
              value: false,
              ty_id: self.ty_checker.bool_type(),
            }),
            Token::String => {
              let symbol = match self.node_value(f_idx) {
                Some(NodeValue::Literal(lit)) => {
                  self.literals.identifiers[lit as usize]
                }
                Some(NodeValue::Symbol(sym)) => sym,
                _ => self.interner.intern(""),
              };

              self.sir.emit(Insn::ConstString {
                dst: pat_dst,
                symbol,
                ty_id: self.ty_checker.str_type(),
              })
            }
            _ => pat_dst,
          };

          // Compare and branch to next_arm on mismatch.
          let cmp_dst = ValueId(self.sir.next_value_id);
          self.sir.next_value_id += 1;

          let cmp_sir = self.sir.emit(Insn::BinOp {
            dst: cmp_dst,
            op: zo_sir::BinOp::Eq,
            lhs: field_sir,
            rhs: pat_sir,
            ty_id: field_ty,
          });

          self.sir.emit(Insn::BranchIfNot {
            cond: cmp_sir,
            target: next_arm_label,
          });
        }
      } else if !is_wildcard && !is_enum_pat && pat_tok == Token::Ident {
        // Ident-pattern `num => body` / `num if guard => body`.
        // Binds the scrutinee's value to `num` inside the arm's
        // scope, then (optionally) evaluates a guard expression
        // and skips the arm when false. Without this path, bare
        // idents fell through with no compare AND no binding —
        // the arm matched unconditionally but `num` was
        // undefined in the body / guard, which for guards meant
        // the `if` evaluated garbage → SIGILL at runtime.
        //
        // Guard detection: the parser emits `If` as the node
        // AFTER the pattern ident when the arm has `if expr`.
        // Its children carry the guard's postorder expression
        // tokens up to FatArrow.
        let has_guard = pat_idx + 1 < arrow_idx
          && self.tree.nodes[pat_idx + 1].token == Token::If;

        // Bind `num` to the scrutinee value. Load scrutinee, then
        // Store it under the pattern's ident name. Register as
        // a local so arm body references (and the guard, if any)
        // resolve to this value. Popped via `arm_bindings` at
        // arm end, so it doesn't leak to subsequent arms.
        if let Some(NodeValue::Symbol(bind_sym)) = self.node_value(pat_idx) {
          let scrut_dst = ValueId(self.sir.next_value_id);

          self.sir.next_value_id += 1;

          let scrut_reload = if let Some(sym) = scrutinee_sym {
            self.sir.emit(Insn::Load {
              dst: scrut_dst,
              src: LoadSource::Local(sym),
              ty_id: scrutinee_ty,
            })
          } else {
            scrut_dst
          };

          self.sir.emit(Insn::Store {
            name: bind_sym,
            value: scrut_reload,
            ty_id: scrutinee_ty,
          });

          let rid = self.values.store_runtime(0);

          self.locals.push(Local {
            name: bind_sym,
            ty_id: scrutinee_ty,
            value_id: rid,
            pubness: Pubness::No,
            mutability: Mutability::No,
            sir_value: Some(scrut_reload),
            local_kind: LocalKind::Variable,
          });

          arm_bindings += 1;
        }

        // Guard expression: sub-walk the tokens of the `If`
        // node's children range (everything between the `If`
        // token and the `FatArrow`). The existing binop /
        // ident machinery evaluates them; the top-of-stack
        // after the walk is the guard's boolean SIR value.
        // `BranchIfNot` to `next_arm_label` skips the body
        // when the guard is false.
        if has_guard {
          let if_idx = pat_idx + 1;
          let guard_start = if_idx + 1;
          let guard_end = arrow_idx;

          let saved_skip = self.skip_until;

          self.skip_until = 0;

          let stack_before = self.sir_values.len();

          for i in guard_start..guard_end {
            if i < self.skip_until {
              continue;
            }

            let node = self.tree.nodes[i];

            self.execute_node(&node, i);
          }

          self.apply_deferred_binop();

          self.skip_until = saved_skip;

          if self.sir_values.len() > stack_before {
            let guard_sir = self.sir_values.pop().unwrap();

            self.value_stack.pop();
            self.ty_stack.pop();

            // Drain extras — the guard must leave exactly one
            // value; anything more means the sub-walk pushed
            // stray state we shouldn't carry into the body.
            while self.sir_values.len() > stack_before {
              self.sir_values.pop();
              self.value_stack.pop();
              self.ty_stack.pop();
            }

            self.sir.emit(Insn::BranchIfNot {
              cond: guard_sir,
              target: next_arm_label,
            });
          }
        }
      }

      // Execute arm body nodes.
      let saved_skip = self.skip_until;

      self.skip_until = 0;

      let body_stack_before = self.sir_values.len();

      for i in (arrow_idx + 1)..body_end {
        if i < self.skip_until {
          continue;
        }

        let node = self.tree.nodes[i];

        self.execute_node(&node, i);
      }

      self.skip_until = saved_skip;

      // Match arms use commas, not semicolons. Manually
      // finalize pending operations that Semicolon handles.
      // Flush deferred binops first — without this, an arm
      // like `tape[0] = tape[0] + 1` defers the `+` (only 1
      // operand when it fires) and finalize_pending_array_assign
      // pops the raw constant instead of the BinOp result.
      self.apply_deferred_binop();

      if self.pending_assign.is_some() {
        self.finalize_pending_assign();
      }

      if self.pending_array_assign.is_some() {
        self.finalize_pending_array_assign();
      }

      self.finalize_pending_decl();

      // If the arm body produced a non-unit value, store it
      // to the match result slot. Once ANY arm produces unit,
      // the match is a statement and we stop capturing.
      let unit_ty = self.ty_checker.unit_type();
      let body_produced_value = self.sir_values.len() > body_stack_before;
      let body_ty = self.ty_stack.last().copied().unwrap_or(unit_ty);

      if is_expr_match
        && body_produced_value
        && body_ty != unit_ty
        && match_result_ty != Some(unit_ty)
      {
        let body_sir = self.sir_values.last().copied().unwrap();
        let result_sym = self.interner.intern("__match_result__");

        self.sir.emit(Insn::Store {
          name: result_sym,
          value: body_sir,
          ty_id: body_ty,
        });

        match_result_ty = Some(body_ty);
        match_result_sym = Some(result_sym);
      } else if body_ty == unit_ty || !body_produced_value {
        // Statement arm — abandon expression capture.
        match_result_ty = Some(unit_ty);
      }

      // Clean up arm body values from stacks.
      while self.sir_values.len() > body_stack_before {
        self.sir_values.pop();
        self.value_stack.pop();
        self.ty_stack.pop();
      }

      // Pop arm-local bindings (tuple variant payload fields)
      // so they don't leak into subsequent arms.
      for _ in 0..arm_bindings {
        self.locals.pop();
      }

      // Jump to the shared end label, then emit the
      // next-arm label for the failing branch above.
      self.sir.emit(Insn::Jump { target: end_label });
      self.sir.emit(Insn::Label { id: next_arm_label });

      // Advance past the arm's body and optional trailing
      // comma; the outer `while` handles the comma skip.
      arm_idx = body_end;
    }

    // -- 5. End label ----------------------------------------
    self.sir.emit(Insn::Label { id: end_label });

    // If the match was used as an expression, load the result
    // from the shared slot and push it to the stacks.
    let unit_ty = self.ty_checker.unit_type();

    if let (Some(ty), Some(sym)) = (match_result_ty, match_result_sym)
      && ty != unit_ty
    {
      let dst = ValueId(self.sir.next_value_id);

      self.sir.next_value_id += 1;

      let sv = self.sir.emit(Insn::Load {
        dst,
        src: LoadSource::Local(sym),
        ty_id: ty,
      });

      let rid = self.values.store_runtime(0);

      self.value_stack.push(rid);
      self.ty_stack.push(ty);
      self.sir_values.push(sv);
    }
  }

  /// Sets up a while loop context.
  fn execute_while(&mut self, _start_idx: usize, _end_idx: usize) {
    let loop_label = self.sir.next_label();
    let end_label = self.sir.next_label();

    self.sir.emit(Insn::Label { id: loop_label });

    self.branch_stack.push(BranchCtx {
      kind: BranchKind::While,
      end_label,
      else_label: None,
      loop_label: Some(loop_label),
      branch_emitted: false,
      for_var: None,
      scope_depth: self.scope_stack.len(),
      // Loops are always statement-position today — no
      // `loop { break value }` expression form yet.
      value_sink: None,
      value_sink_ty: None,
      stack_depth_at_entry: self.sir_values.len() as u32,
    });
  }

  /// Desugars `for i := start..end { body }` (or `..=`) into
  /// while-loop SIR:
  ///   mut i = start;
  ///   val __for_end__ = end;   -- evaluated once
  ///   while i {<|<=} __for_end__ { body; i = i + 1; }
  ///
  /// `start` and `end` are arbitrary expressions (literals,
  /// idents, calls, ...). We sub-walk each expression through
  /// `execute_node` to emit its SIR and collect the top of the
  /// stack as the range bound. `DotDotEq` selects `Lte`,
  /// `DotDot` selects `Lt`.
  fn execute_for(&mut self, start_idx: usize, end_idx: usize) {
    // Tree: For → [Ident(i), ColonEq, <start expr>,
    //              DotDot|DotDotEq, <end expr>, LBrace, ...]
    //
    // Step 1: locate the structural anchors at header depth 0.
    // We track paren/bracket depth so a range op inside e.g.
    // `f(1..=2)` (hypothetical) never gets mistaken for the
    // for-range op.
    let mut var_name: Option<Symbol> = None;
    let mut colon_eq_idx: Option<usize> = None;
    let mut range_idx: Option<usize> = None;
    let mut range_inclusive = false;
    let mut body_start_idx: Option<usize> = None;
    let mut body_is_fat_arrow = false;
    let mut paren_depth: i32 = 0;

    for j in (start_idx + 1)..end_idx {
      let tok = self.tree.nodes[j].token;

      match tok {
        Token::LParen | Token::LBracket => paren_depth += 1,
        Token::RParen | Token::RBracket => paren_depth -= 1,
        _ => {}
      }

      if paren_depth != 0 {
        continue;
      }

      match tok {
        Token::Ident if var_name.is_none() => {
          if let Some(NodeValue::Symbol(sym)) = self.node_value(j) {
            var_name = Some(sym);
          }
        }
        Token::ColonEq if colon_eq_idx.is_none() => {
          colon_eq_idx = Some(j);
        }
        Token::DotDot | Token::DotDotEq if range_idx.is_none() => {
          range_idx = Some(j);
          range_inclusive = tok == Token::DotDotEq;
        }
        Token::LBrace | Token::FatArrow => {
          body_start_idx = Some(j);
          body_is_fat_arrow = tok == Token::FatArrow;
          break;
        }
        _ => {}
      }
    }

    let var_name = match var_name {
      Some(n) => n,
      None => return,
    };

    let colon_eq_idx = match colon_eq_idx {
      Some(i) => i,
      None => return,
    };

    let range_idx = match range_idx {
      Some(i) => i,
      None => return,
    };

    let body_start_idx = match body_start_idx {
      Some(i) => i,
      None => return,
    };

    // For the sub-walk range below we need the last header
    // index (exclusive upper bound). Whether the body marker
    // is `LBrace` or `FatArrow`, header sub-walk goes up to
    // but not including it.
    let lbrace_idx = body_start_idx;

    let int_ty = self.ty_checker.int_type();

    // Step 2: evaluate both range operands. The tree is
    // POSTORDER — `1..3` lowers as `Int(1), Int(3), DotDot`,
    // i.e. operator AFTER operands. Sub-walk every node in
    // `(ColonEq, LBrace)` EXCEPT the range op itself (which
    // has no executor handler and would be a no-op anyway,
    // but skipping it is explicit and cheap). After the walk
    // stacks carry [start, end] with `end` on top.
    //
    // This runs BEFORE the loop var is introduced so
    // expressions like `for i := 1..i` see the OUTER `i`.
    let saved_skip = self.skip_until;

    self.skip_until = 0;

    let stack_before = self.sir_values.len();

    for i in (colon_eq_idx + 1)..lbrace_idx {
      if i == range_idx {
        continue;
      }

      if i < self.skip_until {
        continue;
      }

      let node = self.tree.nodes[i];

      self.execute_node(&node, i);
    }

    self.apply_deferred_binop();

    self.skip_until = saved_skip;

    // Pop in postorder: end first (top of stack), then start.
    let zero_const = |ex: &mut Self| -> ValueId {
      let dst = ValueId(ex.sir.next_value_id);

      ex.sir.next_value_id += 1;

      ex.sir.emit(Insn::ConstInt {
        dst,
        value: 0,
        ty_id: int_ty,
      })
    };

    let end_sir = if self.sir_values.len() > stack_before {
      let s = self.sir_values.pop().unwrap();

      self.value_stack.pop();
      self.ty_stack.pop();
      s
    } else {
      zero_const(self)
    };

    let start_sir = if self.sir_values.len() > stack_before {
      let s = self.sir_values.pop().unwrap();

      self.value_stack.pop();
      self.ty_stack.pop();
      s
    } else {
      zero_const(self)
    };

    // Drain any extras (shouldn't happen for well-formed
    // input — safety net).
    while self.sir_values.len() > stack_before {
      self.sir_values.pop();
      self.value_stack.pop();
      self.ty_stack.pop();
    }

    // Step 4: declare the iterator var and store `start`.
    let init_vid = self.values.store_runtime(0);

    self.sir.emit(Insn::VarDef {
      name: var_name,
      ty_id: int_ty,
      init: Some(start_sir),
      mutability: Mutability::Yes,
      pubness: Pubness::No,
    });

    self.locals.push(Local {
      name: var_name,
      ty_id: int_ty,
      value_id: init_vid,
      pubness: Pubness::No,
      mutability: Mutability::Yes,
      sir_value: Some(start_sir),
      local_kind: LocalKind::Variable,
    });

    if let Some(frame) = self.scope_stack.last_mut() {
      frame.count += 1;
    }

    self.sir.emit(Insn::Store {
      name: var_name,
      value: start_sir,
      ty_id: int_ty,
    });

    // Step 5: spill the end bound to a synthetic local so the
    // loop reloads it each iteration. This handles non-const
    // bounds (params, calls, arbitrary exprs) uniformly — no
    // special case for "const fold into the compare".
    let end_sym = {
      let name = format!("__for_end_{}__", self.sir.instructions.len());

      self.interner.intern(&name)
    };

    self.sir.emit(Insn::VarDef {
      name: end_sym,
      ty_id: int_ty,
      init: Some(end_sir),
      mutability: Mutability::No,
      pubness: Pubness::No,
    });

    self.sir.emit(Insn::Store {
      name: end_sym,
      value: end_sir,
      ty_id: int_ty,
    });

    // Register the synthetic end slot as a local so the
    // codegen-side Load targets its stack slot.
    let end_local_vid = self.values.store_runtime(0);

    self.locals.push(Local {
      name: end_sym,
      ty_id: int_ty,
      value_id: end_local_vid,
      pubness: Pubness::No,
      mutability: Mutability::No,
      sir_value: Some(end_sir),
      local_kind: LocalKind::Variable,
    });

    if let Some(frame) = self.scope_stack.last_mut() {
      frame.count += 1;
    }

    // Step 6: loop header.
    let loop_label = self.sir.next_label();
    let end_label = self.sir.next_label();

    self.sir.emit(Insn::Label { id: loop_label });

    let cond_dst = ValueId(self.sir.next_value_id);

    self.sir.next_value_id += 1;

    let load_sir = self.sir.emit(Insn::Load {
      dst: cond_dst,
      src: LoadSource::Local(var_name),
      ty_id: int_ty,
    });

    let end_reload_dst = ValueId(self.sir.next_value_id);

    self.sir.next_value_id += 1;

    let end_reload_sir = self.sir.emit(Insn::Load {
      dst: end_reload_dst,
      src: LoadSource::Local(end_sym),
      ty_id: int_ty,
    });

    let cmp_dst = ValueId(self.sir.next_value_id);

    self.sir.next_value_id += 1;

    let cmp_op = if range_inclusive {
      zo_sir::BinOp::Lte
    } else {
      zo_sir::BinOp::Lt
    };

    let cmp_sir = self.sir.emit(Insn::BinOp {
      dst: cmp_dst,
      op: cmp_op,
      lhs: load_sir,
      rhs: end_reload_sir,
      ty_id: int_ty,
    });

    self.sir.emit(Insn::BranchIfNot {
      cond: cmp_sir,
      target: end_label,
    });

    // Push branch context — RBrace will emit increment + jump.
    self.branch_stack.push(BranchCtx {
      kind: BranchKind::For,
      end_label,
      else_label: None,
      loop_label: Some(loop_label),
      branch_emitted: true,
      for_var: Some(var_name),
      scope_depth: self.scope_stack.len(),
      value_sink: None,
      value_sink_ty: None,
      stack_depth_at_entry: self.sir_values.len() as u32,
    });

    // `LBrace` form: hand off to the main loop. The LBrace
    // handler will push a scope and the body walks through
    // normally; its closing `RBrace` fires the For-loop
    // close path (increment + jump + end_label) and pops
    // the scope.
    if !body_is_fat_arrow {
      self.skip_until = body_start_idx;

      return;
    }

    // `=>` line form: `for x := start..end => expr;`.
    // The parser generates ONE synthetic `RBrace` that
    // tries to serve as both the loop terminator AND the
    // enclosing block's terminator — we can't let the
    // main loop re-use it, so finalize everything inline
    // here. Find the Semicolon that closes the single
    // body expression, sub-walk the body, emit loop close
    // (increment + jump + end_label), pop the branch ctx
    // + scope, and skip past the Semicolon. The outer
    // RBrace then cleanly closes the enclosing function /
    // block with no loop ambiguity.
    let semicolon_idx = ((body_start_idx + 1)..end_idx)
      .find(|&j| self.tree.nodes[j].token == Token::Semicolon)
      .unwrap_or(end_idx);

    // Scope for the body (mirrors LBrace form's push).
    self.push_scope();

    // Sub-walk body statements. The range includes the
    // terminating `Semicolon` — it's what triggers the
    // `finalize_pending_assign` / `finalize_pending_compound`
    // hooks, so dropping it silently swallows assignment-
    // style bodies like `n += 1` or `count = count + 1`.
    let saved_skip = self.skip_until;

    self.skip_until = 0;

    for i in (body_start_idx + 1)..=semicolon_idx {
      if i < self.skip_until {
        continue;
      }

      let node = self.tree.nodes[i];

      self.execute_node(&node, i);

      if self.tuple_ctx.is_empty() && self.pending_call_rparen.is_none() {
        self.apply_deferred_binop();
      }
    }

    self.skip_until = saved_skip;

    // Discard any leftover stack values — the body
    // expression's result isn't used.
    while self.sir_values.len() > stack_before {
      self.sir_values.pop();
      self.value_stack.pop();
      self.ty_stack.pop();
    }

    // Loop close: increment + jump + end label. Mirrors the
    // `BranchKind::For` arm of the RBrace handler (~line
    // 1475) — kept in sync by design.
    let ld = ValueId(self.sir.next_value_id);

    self.sir.next_value_id += 1;

    let ld_sir = self.sir.emit(Insn::Load {
      dst: ld,
      src: LoadSource::Local(var_name),
      ty_id: int_ty,
    });

    let one_dst = ValueId(self.sir.next_value_id);

    self.sir.next_value_id += 1;

    let one_sir = self.sir.emit(Insn::ConstInt {
      dst: one_dst,
      value: 1,
      ty_id: int_ty,
    });

    let add_dst = ValueId(self.sir.next_value_id);

    self.sir.next_value_id += 1;

    let add_sir = self.sir.emit(Insn::BinOp {
      dst: add_dst,
      op: zo_sir::BinOp::Add,
      lhs: ld_sir,
      rhs: one_sir,
      ty_id: int_ty,
    });

    self.sir.emit(Insn::Store {
      name: var_name,
      value: add_sir,
      ty_id: int_ty,
    });

    self.sir.emit(Insn::Jump { target: loop_label });
    self.sir.emit(Insn::Label { id: end_label });

    self.branch_stack.pop();
    self.pop_scope();

    // Skip past the `;` that terminated the body. The
    // synthetic `RBrace` that follows is the enclosing
    // block's — main loop handles it normally.
    self.skip_until = semicolon_idx + 1;
  }

  /// Begins compound assignment (+=, -=, etc).
  /// Tree order: target, CompoundOp, rhs_expr.
  /// We save the target + op, discard the LHS from the
  /// stack (it was pushed by the Ident handler), and let
  /// the main loop process the RHS. Finalized at
  /// Semicolon.
  fn execute_compound_assignment(&mut self, op: BinOp, node_idx: usize) {
    // Look back to find the target variable.
    if node_idx < 1 {
      return;
    }

    let target_idx = node_idx - 1;

    // Field compound assign: `receiver.field +=`.
    // In postfix the tree is: receiver, field, Dot, +=.
    // So target_idx points at Dot.
    if self.tree.nodes[target_idx].token == Token::Dot && target_idx >= 2 {
      // field is at target_idx - 1, receiver at - 2.
      let field_idx = target_idx - 1;
      let recv_idx = target_idx - 2;

      if let Some(NodeValue::Symbol(field_name)) = self.node_value(field_idx) {
        // Pop the dot result (or whatever is on the stack).
        self.value_stack.pop();
        self.ty_stack.pop();
        self.sir_values.pop();

        let span = self.tree.spans[field_idx];

        // Record receiver so finalize can check mutability.
        let recv_sym = match self.tree.nodes[recv_idx].token {
          Token::SelfLower => Some(zo_interner::Symbol::SELF_LOWER),
          Token::Ident => self.node_value(recv_idx).and_then(|v| match v {
            NodeValue::Symbol(s) => Some(s),
            _ => None,
          }),
          _ => None,
        };

        self.pending_compound_receiver = recv_sym;
        self.pending_compound = Some((field_name, op, span));
      }
      return;
    }

    // Direct variable compound assign: `x +=`.
    if let Token::Ident = self.tree.nodes[target_idx].token
      && let Some(NodeValue::Symbol(name)) = self.node_value(target_idx)
    {
      // Discard the LHS pushed by the Ident handler.
      self.value_stack.pop();
      self.ty_stack.pop();
      self.sir_values.pop();

      let span = self.tree.spans[target_idx];

      self.pending_compound_receiver = None;
      self.pending_compound = Some((name, op, span));
    }
  }

  /// Replace folded operand instructions with `Nop` in-place
  /// and remove their annotations. Indices stay stable.
  fn nop_folded_operands(&mut self, lhs_sir: ValueId, rhs_sir: ValueId) {
    for insn in self.sir.instructions.iter_mut() {
      let dst = match insn {
        Insn::ConstInt { dst, .. }
        | Insn::ConstFloat { dst, .. }
        | Insn::ConstBool { dst, .. }
        | Insn::ConstString { dst, .. }
        | Insn::BinOp { dst, .. }
        | Insn::UnOp { dst, .. }
        | Insn::Call { dst, .. }
        | Insn::Load { dst, .. }
        | Insn::ArrayLiteral { dst, .. }
        | Insn::ArrayIndex { dst, .. }
        | Insn::ArrayLen { dst, .. }
        | Insn::TupleLiteral { dst, .. }
        | Insn::TupleIndex { dst, .. }
        | Insn::EnumConstruct { dst, .. }
        | Insn::StructConstruct { dst, .. } => Some(*dst),
        Insn::Template { id, .. } => Some(*id),
        _ => None,
      };

      if dst == Some(lhs_sir) || dst == Some(rhs_sir) {
        *insn = Insn::Nop;
      }
    }

    // Remove annotations that pointed at the now-dead
    // operands. They are the most recent two.
    let len = self.annotations.len();

    if len >= 2 {
      self.annotations.truncate(len - 2);
    }
  }

  /// Finalize a pending compound assignment at Semicolon.
  fn finalize_pending_compound(&mut self) {
    let (name, op, span) = match self.pending_compound.take() {
      Some(c) => c,
      None => return,
    };

    // Pop the RHS value (processed by the main loop).
    let (Some(_rhs_value), Some(rhs_ty)) =
      (self.value_stack.pop(), self.ty_stack.pop())
    else {
      return;
    };
    let rhs_sir = self.sir_values.pop();

    // Find the mutable variable. For field access
    // (`self.x += 1`), `name` is the field — look up
    // the receiver (`self`) and check its mutability.
    let local = self.locals.iter_mut().rev().find(|l| l.name == name);

    let Some(local) = local else {
      // Not a direct local — field compound assign
      // (e.g., `self.x += 1`). Emit SIR for:
      //   TupleIndex (read) + BinOp + FieldStore (write).
      let recv_sym = match self.pending_compound_receiver.take() {
        Some(s) => s,
        None => return,
      };

      // Check receiver mutability and local kind.
      let recv_info = self
        .locals
        .iter()
        .rev()
        .find(|l| l.name == recv_sym)
        .map(|l| (l.ty_id, l.mutability, l.local_kind));

      let Some((recv_ty, recv_mut, recv_kind)) = recv_info else {
        return;
      };

      if recv_mut != Mutability::Yes {
        report_error(Error::new(ErrorKind::ImmutableVariable, span));
        return;
      }

      // Resolve field index from the struct type.
      let field_info = if let Ty::Struct(sid) = self.ty_checker.kind_of(recv_ty)
      {
        if let Some(st) = self.ty_checker.ty_table.struct_ty(sid) {
          let st = *st;
          let fields = self.ty_checker.ty_table.struct_fields(&st).to_vec();
          let fname_str = self.interner.get(name).to_owned();

          fields
            .iter()
            .enumerate()
            .find(|(_, f)| self.interner.get(f.name) == fname_str)
            .map(|(i, f)| (i as u32, f.ty_id))
        } else {
          None
        }
      } else {
        None
      };

      let Some((field_idx, field_ty)) = field_info else {
        return;
      };

      if let Some(rhs_s) = rhs_sir {
        // Load receiver pointer. Use Param source for
        // parameters (e.g., self) so the codegen reads
        // from the param spill slot, not mutable_slots.
        let recv_src = if recv_kind == LocalKind::Parameter {
          let param_idx =
            self
              .current_function
              .as_ref()
              .and_then(|ctx| {
                self.funs.iter().find(|f| f.name == ctx.name).and_then(|f| {
                  f.params.iter().position(|(n, _)| *n == recv_sym)
                })
              })
              .unwrap_or(0) as u32;

          LoadSource::Param(param_idx)
        } else {
          LoadSource::Local(recv_sym)
        };

        let recv_dst = ValueId(self.sir.next_value_id);
        self.sir.next_value_id += 1;

        self.sir.emit(Insn::Load {
          dst: recv_dst,
          src: recv_src,
          ty_id: recv_ty,
        });

        // Read current field value.
        let old_val = ValueId(self.sir.next_value_id);
        self.sir.next_value_id += 1;

        self.sir.emit(Insn::TupleIndex {
          dst: old_val,
          tuple: recv_dst,
          index: field_idx,
          ty_id: field_ty,
        });

        // Compute new value.
        let new_val = ValueId(self.sir.next_value_id);
        self.sir.next_value_id += 1;

        self.sir.emit(Insn::BinOp {
          dst: new_val,
          op,
          lhs: old_val,
          rhs: rhs_s,
          ty_id: field_ty,
        });

        // Write back to field.
        self.sir.emit(Insn::FieldStore {
          base: recv_dst,
          index: field_idx,
          value: new_val,
          ty_id: field_ty,
        });
      }
      return;
    };

    if local.mutability != Mutability::Yes {
      report_error(Error::new(ErrorKind::ImmutableVariable, span));
      return;
    }

    let Some(unified_ty) = self.ty_checker.unify(local.ty_id, rhs_ty, span)
    else {
      return;
    };

    // Emit Load(x) + BinOp(op, loaded, rhs) + Store(x).
    if let Some(rhs_s) = rhs_sir {
      let load_dst = ValueId(self.sir.next_value_id);
      self.sir.next_value_id += 1;

      self.sir.emit(Insn::Load {
        dst: load_dst,
        src: LoadSource::Local(name),
        ty_id: unified_ty,
      });

      let dst = ValueId(self.sir.next_value_id);
      self.sir.next_value_id += 1;

      let result_sir = self.sir.emit(Insn::BinOp {
        dst,
        op,
        lhs: load_dst,
        rhs: rhs_s,
        ty_id: unified_ty,
      });

      self.sir.emit(Insn::Store {
        name,
        value: result_sir,
        ty_id: unified_ty,
      });

      local.value_id = self.values.store_runtime(0);
    }
  }

  /// Executes return statement - acts as an introducer.
  fn execute_return(&mut self, _node_idx: usize) {
    // Only process return if we're in a function body
    if let Some(ref mut ctx) = self.current_function {
      // Mark that we're expecting a return value
      // The actual Return instruction will be emitted when we have the complete
      // value
      ctx.pending_return = true;
      ctx.has_explicit_return = true;
    }
  }

  /// Check if we have a pending return and emit it with the current stack value
  fn check_pending_return(&mut self) {
    // Inside a ternary, the Colon and RBrace handlers
    // emit per-arm Returns instead.
    if self
      .branch_stack
      .last()
      .is_some_and(|c| c.kind == BranchKind::Ternary)
    {
      return;
    }

    if let Some(ref mut ctx) = self.current_function
      && ctx.pending_return
    {
      // We have a pending return and a value on the stack
      let (return_value, return_ty) =
        if !self.sir_values.is_empty() && !self.ty_stack.is_empty() {
          let ty = self
            .ty_stack
            .last()
            .copied()
            .unwrap_or(self.ty_checker.unit_type());

          let sir_value = self.sir_values.last().copied();

          (sir_value, ty)
        } else {
          (None, self.ty_checker.unit_type())
        };

      // Emit the Return instruction
      self.sir.emit(Insn::Return {
        value: return_value,
        ty_id: return_ty,
      });

      // Pop the return value from stacks so it doesn't
      // leak into subsequent code (e.g. `if x <= 1 {
      // return 1; } return x * fact(x-1);` — the stale
      // `1` from the if body must not become an operand
      // for the `*` in the recursive case).
      if return_value.is_some() {
        self.value_stack.pop();
        self.ty_stack.pop();
        self.sir_values.pop();
      }

      // Clear the pending flag
      ctx.pending_return = false;
    }
  }

  /// Returns true if `dot_idx` is part of a pack-dotted
  /// chain (`pack.fn()`, `outer.inner.fn()`). Walks back
  /// through alternating `Ident`/`Dot` in postfix order
  /// until it reaches the root ident — if that root is a
  /// declared pack name (`self.pack_names`), this Dot is
  /// a namespace-qualification dot, not a field access.
  fn is_pack_chain_dot(&self, dot_idx: usize) -> bool {
    self.pack_chain_root(dot_idx).is_some()
  }

  /// Walk back the pack-dot chain ending at `dot_idx` and
  /// return the tree index of the root Ident (the pack
  /// name) if the chain roots in a declared pack. Used by
  /// `is_pack_chain_dot` and the dot-call resolver.
  fn pack_chain_root(&self, dot_idx: usize) -> Option<usize> {
    let mut i = dot_idx;

    loop {
      if i < 2 || self.tree.nodes[i].token != Token::Dot {
        return None;
      }

      if self.tree.nodes[i - 1].token != Token::Ident {
        return None;
      }

      // Nested chain: another Dot before the Ident.
      if i >= 3 && self.tree.nodes[i - 2].token == Token::Dot {
        i -= 2;
        continue;
      }

      // Reached the root.
      if i < 2 || self.tree.nodes[i - 2].token != Token::Ident {
        return None;
      }

      let root_sym = match self.node_value(i - 2) {
        Some(NodeValue::Symbol(s)) => s,
        _ => return None,
      };

      if self.pack_names.contains(&root_sym) {
        return Some(i - 2);
      }

      return None;
    }
  }

  /// Walks a postfix dot chain whose last token is at
  /// `end_idx` (which may be an `Ident` leaf or a `Dot`
  /// interior node) and appends every Ident symbol it
  /// passes into `parts`, in root-to-leaf order. Returns
  /// `false` if the shape is malformed.
  ///
  /// Postfix encoding:
  /// - `a.b`     → `a b .`     → [a, b]
  /// - `a.b.c`   → `a b . c .` → [a, b, c]
  /// - `a.b.c.d` → `a b . c . d .`
  fn collect_dot_chain(&self, end_idx: usize, parts: &mut Vec<Symbol>) -> bool {
    match self.tree.nodes[end_idx].token {
      Token::Ident => match self.node_value(end_idx) {
        Some(NodeValue::Symbol(s)) => {
          parts.push(s);

          true
        }
        _ => false,
      },
      Token::Dot => {
        if end_idx < 2 {
          return false;
        }

        let method_idx = end_idx - 1;
        let recv_end = end_idx - 2;

        if self.tree.nodes[method_idx].token != Token::Ident {
          return false;
        }

        // Recurse into receiver first so parts stay in
        // root-to-leaf order.
        if !self.collect_dot_chain(recv_end, parts) {
          return false;
        }

        match self.node_value(method_idx) {
          Some(NodeValue::Symbol(s)) => {
            parts.push(s);

            true
          }
          _ => false,
        }
      }
      _ => false,
    }
  }

  /// Resolves a pack-dotted call `pack.fn(...)` or
  /// `outer.inner.fn(...)` to its fully-qualified mangled
  /// callee symbol (`pack::fn`, `outer::inner::fn`) if
  /// the tree at `lparen_idx` has that shape AND the
  /// mangled name resolves to a declared function.
  fn resolve_pack_dotted_call(&self, lparen_idx: usize) -> Option<Symbol> {
    if lparen_idx < 3 {
      return None;
    }

    // Must be `... . ( ...`.
    if self.tree.nodes[lparen_idx - 1].token != Token::Dot {
      return None;
    }

    let leaf_dot = lparen_idx - 1;

    // Verify this dot's chain roots in a declared pack.
    self.pack_chain_root(leaf_dot)?;

    let mut parts: Vec<Symbol> = Vec::new();

    if !self.collect_dot_chain(leaf_dot, &mut parts) {
      return None;
    }

    if parts.len() < 2 {
      return None;
    }

    let raw_parts: Vec<String> = parts
      .iter()
      .map(|s| self.interner.get(*s).to_owned())
      .collect();

    // Resolution order mirrors name lookup in other
    // languages: first try the path as written (absolute
    // from any pack we've seen), then try prefixing with
    // the enclosing pack context (so `inner2.hello()`
    // inside `pack inner { fun h() { ... } }` binds to
    // `inner::inner2::hello`).
    let as_written = raw_parts.join("::");

    if let Some(sym) = self.interner.symbol(&as_written)
      && self.funs.iter().any(|f| f.name == sym)
    {
      return Some(sym);
    }

    for prefix_len in (1..=self.pack_context.len()).rev() {
      let mut prefixed: Vec<String> = self.pack_context[..prefix_len]
        .iter()
        .map(|(s, _)| self.interner.get(*s).to_owned())
        .collect();

      prefixed.extend(raw_parts.iter().cloned());

      let mangled = prefixed.join("::");

      if let Some(sym) = self.interner.symbol(&mangled)
        && self.funs.iter().any(|f| f.name == sym)
      {
        return Some(sym);
      }
    }

    None
  }

  /// Given a `LParen` tree index, returns the tree index of
  /// the function/closure Ident if this paren starts a call.
  ///
  /// Handles two layouts:
  ///   - `Ident LParen` → direct (idx - 1)
  ///   - `Ident Op LParen` → operator between (idx - 2),
  ///     validated by checking the Ident is a known function
  ///     or closure to avoid false positives like `a * (b)`.
  fn resolve_call_target(&self, lparen_idx: usize) -> Option<usize> {
    if lparen_idx == 0 {
      return None;
    }

    // Direct: Ident immediately before LParen.
    let prev = self.tree.nodes[lparen_idx - 1].token;

    if prev == Token::Ident {
      // A `::`-path (method/variant), `Dot`-path
      // (`pack.fn()`), or `@`-modifier (`check@eq(`)
      // before the Ident is always a call site — skip
      // the fun/closure validation.
      let has_path_prefix = lparen_idx >= 2
        && matches!(
          self.tree.nodes[lparen_idx - 2].token,
          Token::ColonColon | Token::Dot | Token::At
        );

      if has_path_prefix {
        return Some(lparen_idx - 1);
      }

      // Otherwise rule out variables: if the Ident
      // resolves to a known NON-closure local, the
      // adjacent `(` is a group, not a call (e.g. `low +
      // (high - low)` where the parser now emits `low`
      // adjacent to `(`). Everything else — named funs,
      // closures, and unknown idents (external/builtin
      // callees resolved at a later phase) — is treated
      // as a call site.
      if let Some(NodeValue::Symbol(sym)) = self.node_value(lparen_idx - 1) {
        let is_plain_local = self.lookup_local(sym).is_some_and(|l| {
          let vi = l.value_id.0 as usize;

          vi < self.values.kinds.len()
            && !matches!(self.values.kinds[vi], Value::Closure)
        });

        if is_plain_local {
          return None;
        }
      }

      return Some(lparen_idx - 1);
    }

    // Operator between: Ident Op LParen.
    if lparen_idx >= 2 {
      let prev2 = self.tree.nodes[lparen_idx - 2].token;

      if prev2 == Token::Ident {
        // Validate: the Ident must be a known function or
        // closure. Otherwise it's a variable and the paren
        // is grouping (e.g. `a * (b + c)`).
        if let Some(NodeValue::Symbol(sym)) = self.node_value(lparen_idx - 2) {
          let is_fun = self.funs.iter().any(|f| f.name == sym);

          let is_closure = self.lookup_local(sym).is_some_and(|l| {
            let vi = l.value_id.0 as usize;

            vi < self.values.kinds.len()
              && matches!(self.values.kinds[vi], Value::Closure)
          });

          if is_fun || is_closure {
            return Some(lparen_idx - 2);
          }
        }
      }
    }

    None
  }

  /// Returns true if the Ident at `ident_idx` is the callee
  /// of an imminent call — direct (`f(`), operator-separated
  /// (`a + f(`), or modifier (`check@eq(`). Mirrors
  /// `resolve_call_target`'s logic from the LParen side, but
  /// looking forward from the Ident. Used by the Ident
  /// handler to decide whether to push a `Value::Closure`
  /// (first-class fun reference) vs skip pushing (callee
  /// about to be consumed by RParen).
  fn ident_is_call_target(&self, ident_idx: usize) -> bool {
    let n = self.tree.nodes.len();

    if ident_idx + 1 >= n {
      return false;
    }

    // Direct: `Ident (`. Validate the Ident is a function
    // or closure — otherwise the adjacent `(` is a group
    // (`x + (1)` emits `x, LParen` but `x` is a variable,
    // not a callee). Mirrors `resolve_call_target`.
    if self.tree.nodes[ident_idx + 1].token == Token::LParen {
      // `::`/`.`/`@` before the ident is always a call
      // site (`Point::new(`, `pack.fn(`, `check@eq(`).
      let has_path_prefix = ident_idx >= 1
        && matches!(
          self.tree.nodes[ident_idx - 1].token,
          Token::ColonColon | Token::Dot | Token::At
        );

      if has_path_prefix {
        return true;
      }

      // Mirror `resolve_call_target`: only REJECT the
      // call interpretation if the ident is a known
      // non-closure local. Otherwise (fun, closure, or
      // external) treat as a call site.
      if let Some(NodeValue::Symbol(sym)) = self.node_value(ident_idx) {
        let is_plain_local = self.lookup_local(sym).is_some_and(|l| {
          let vi = l.value_id.0 as usize;

          vi < self.values.kinds.len()
            && !matches!(self.values.kinds[vi], Value::Closure)
        });

        return !is_plain_local;
      }

      return true;
    }

    // Operator-separated: `Ident Op (` — mirror the LParen
    // prev2 branch of `resolve_call_target`.
    if ident_idx + 2 < n
      && self.tree.nodes[ident_idx + 2].token == Token::LParen
    {
      return true;
    }

    // Modifier call: `Ident @ Ident (`. The base ident is
    // still the callee; the middle ident is the modifier.
    if ident_idx + 3 < n
      && self.tree.nodes[ident_idx + 1].token == Token::At
      && self.tree.nodes[ident_idx + 2].token == Token::Ident
      && self.tree.nodes[ident_idx + 3].token == Token::LParen
    {
      return true;
    }

    false
  }

  /// Returns true if `rparen_idx` closes an `(...)` whose
  /// LParen is a call site (i.e. `f(` / `Type::m(`). Used at
  /// the RParen dispatcher to pick the call path before the
  /// tuple/grouping path — otherwise a call's closing `)`
  /// inside a tuple literal (`(f(x), g(y))`) would pop the
  /// surrounding tuple_ctx and silently drop the call.
  fn rparen_closes_call(&self, rparen_idx: usize) -> bool {
    let mut depth = 1i32;
    let mut idx = rparen_idx;

    while idx > 0 && depth > 0 {
      idx -= 1;

      match self.tree.nodes[idx].token {
        Token::RParen => depth += 1,
        Token::LParen => depth -= 1,
        _ => {}
      }
    }

    if depth != 0 {
      return false;
    }

    self.resolve_call_target(idx).is_some()
      || self.resolve_pack_dotted_call(idx).is_some()
  }

  /// Checks if RParen closes a function call and executes it.
  fn execute_potential_call(&mut self, rparen_idx: usize) {
    // Look back to find matching LParen
    let mut depth = 1;
    let mut lparen_idx = None;
    let mut idx = rparen_idx;

    while idx > 0 && depth > 0 {
      idx -= 1;

      match self.tree.nodes[idx].token {
        Token::RParen => depth += 1,
        Token::LParen => {
          depth -= 1;

          if depth == 0 {
            lparen_idx = Some(idx);
          }
        }
        _ => {}
      }
    }

    if let Some(lparen_idx) = lparen_idx {
      // Pack-dotted call (`pack.fn(...)`,
      // `outer.inner.fn(...)`). Walk back through the
      // chain, mangle to `outer::inner::fn`, emit the
      // call directly — none of the receiver-as-`self`
      // paths apply, there is no implicit argument.
      if let Some(mangled) = self.resolve_pack_dotted_call(lparen_idx) {
        self.execute_call(mangled, lparen_idx, rparen_idx);

        return;
      }

      // Check if there's an identifier before LParen.
      // Also check idx-2 via resolve_call_target for the
      // `1 + f(x)` pattern where an operator intervenes.
      if lparen_idx > 0 {
        let fun_idx = self
          .resolve_call_target(lparen_idx)
          .unwrap_or(lparen_idx - 1);

        if let Token::Ident = self.tree.nodes[fun_idx].token {
          // Check for modifier pattern: Ident @ Ident LParen
          // e.g., check@lt(a, b)
          let (base_idx, modifier) = if fun_idx >= 2
            && self.tree.nodes[fun_idx - 1].token == Token::At
            && self.tree.nodes[fun_idx - 2].token == Token::Ident
          {
            // fun_idx-2 = base ident, fun_idx-1 = @, fun_idx = modifier
            let mod_sym = self.node_value(fun_idx).and_then(|v| match v {
              NodeValue::Symbol(s) => Some(s),
              _ => None,
            });

            (fun_idx - 2, mod_sym)
          } else {
            (fun_idx, None)
          };

          // Check if this is a function declaration (has 'fun'
          // before the identifier)
          let is_declaration = if base_idx > 0 {
            matches!(self.tree.nodes[base_idx - 1].token, Token::Fun)
          } else {
            false
          };

          // Only execute call if it's not a declaration
          if !is_declaration
            && let Some(NodeValue::Symbol(fun_name)) = self.node_value(base_idx)
          {
            if let Some(mod_sym) = modifier {
              self.execute_check_modifier(
                fun_name, mod_sym, lparen_idx, rparen_idx,
              );
            } else {
              // Check for Type::method() pattern.
              let call_name = if fun_idx >= 2
                && self.tree.nodes[fun_idx - 1].token == Token::ColonColon
                && self.tree.nodes[fun_idx - 2].token == Token::Ident
              {
                if let Some(NodeValue::Symbol(type_sym)) =
                  self.node_value(fun_idx - 2)
                {
                  let ts = self.interner.get(type_sym).to_owned();
                  let ms = self.interner.get(fun_name).to_owned();
                  let mangled = format!("{ts}::{ms}");

                  self.interner.intern(&mangled)
                } else {
                  fun_name
                }
              } else if fun_idx >= 2
                && self.tree.nodes[fun_idx - 1].token == Token::Dot
              {
                // Dot-call: tree [recv, ., method, (, )].
                let mangled = self.resolve_dot_call(fun_idx, fun_name);

                if mangled != fun_name {
                  self.execute_dot_method_call(mangled, lparen_idx, rparen_idx);

                  return;
                }

                fun_name
              } else if lparen_idx >= 2
                && self.tree.nodes[lparen_idx - 1].token == Token::Dot
              {
                // Dot-call: tree [recv, method, ., (, )].
                // receiver is at fun_idx - 1 (not fun_idx - 2).
                let mangled =
                  self.resolve_dot_call_with_receiver(fun_idx - 1, fun_name);

                if mangled != fun_name {
                  self.execute_dot_method_call(mangled, lparen_idx, rparen_idx);

                  return;
                }

                fun_name
              } else {
                fun_name
              };

              self.execute_call(call_name, lparen_idx, rparen_idx);
            }
          }
        } else if self.tree.nodes[fun_idx].token == Token::Dot && fun_idx >= 2 {
          // Postfix dot-call: `receiver method . ( )`
          // fun_idx is `.`, method is at fun_idx-1,
          // receiver before that.
          let method_idx = fun_idx;
          let method_name_idx = fun_idx - 1;

          if self.tree.nodes[method_name_idx].token == Token::Ident
            && let Some(NodeValue::Symbol(method_sym)) =
              self.node_value(method_name_idx)
          {
            // Array builtin methods.
            let ms = self.interner.get(method_sym).to_owned();

            if let Some(recv_ty) = self
              .ty_stack
              .get(self.ty_stack.len().saturating_sub(2))
              .copied()
              && matches!(self.ty_checker.kind_of(recv_ty), Ty::Array(_))
            {
              if ms == "push" {
                self.execute_array_push(lparen_idx, rparen_idx);

                return;
              }

              if ms == "pop" {
                self.execute_array_pop(lparen_idx, rparen_idx);

                return;
              }
            }

            let mangled = self.resolve_dot_call(method_idx, method_sym);

            if mangled != method_sym {
              self.execute_dot_method_call(mangled, lparen_idx, rparen_idx);
            }
          }
        }
      }
    }
  }

  /// Resolves a closure variable to its FunDef + capture count.
  /// Returns `(Some(func), capture_count)` if found, else `(None, 0)`.
  fn resolve_closure_call(&self, name: Symbol) -> (Option<FunDef>, u32) {
    let local = match self.lookup_local(name) {
      Some(l) => l,
      None => return (None, 0),
    };

    let idx = local.value_id.0 as usize;

    if idx >= self.values.kinds.len() {
      return (None, 0);
    }

    if !matches!(self.values.kinds[idx], Value::Closure) {
      return (None, 0);
    }

    let ci = self.values.indices[idx] as usize;
    let cv = &self.values.closures[ci];
    let maybe_fun = self.funs.iter().find(|f| f.name == cv.fun_name).cloned();

    match maybe_fun {
      Some(f) => {
        let cc = match f.kind {
          FunctionKind::Closure { capture_count } => capture_count,
          _ => 0,
        };

        (Some(f), cc)
      }
      None => (None, 0),
    }
  }

  /// Checks if the call has a single InterpString argument.
  fn is_single_interp_arg(&self, lparen_idx: usize, rparen_idx: usize) -> bool {
    // Single arg: exactly one non-comma token between parens.
    let arg_idx = lparen_idx + 1;

    arg_idx < rparen_idx
      && self.tree.nodes[arg_idx].token == Token::InterpString
  }

  /// Desugars `showln("{x}, {y}")` into a sequence of
  /// typed show() calls. Compile-time interpolation.
  ///
  /// Segments are pre-parsed by the tokenizer and stored
  /// in LiteralStore. The executor reads them and emits
  /// one show/showln Call per segment.
  fn execute_interp_call(
    &mut self,
    fun_name: Symbol,
    lparen_idx: usize,
    rparen_idx: usize,
  ) {
    let name_str = self.interner.get(fun_name);
    let wants_newline = name_str == "showln" || name_str == "eshowln";
    let is_stderr = name_str.starts_with('e');

    // Resolve the "show"/"eshow" symbol for intermediate
    // calls. Intern if not yet present.
    let base_name = if is_stderr { "eshow" } else { "show" };
    let show_sym = self.interner.intern(base_name);

    // Pop the already-pushed ConstString arg from stacks.
    self.value_stack.pop();
    self.ty_stack.pop();
    self.sir_values.pop();

    // Get pre-parsed segments from LiteralStore.
    // Tree node stores Literal(packed): low 16 = string
    // idx, high 16 = interp_ranges idx.
    let arg_idx = lparen_idx + 1;

    let packed = match self.tree.value(arg_idx as u32) {
      Some(NodeValue::Literal(p)) => p,
      _ => return,
    };

    let interp_id = packed >> 16;
    let segments = self.literals.interp_segs(interp_id);

    let unit_ty = self.ty_checker.unit_type();
    let str_ty = self.ty_checker.str_type();
    let n = segments.len();
    let span = self.tree.spans[rparen_idx];

    // Collect segments into a local vec to avoid borrow
    // issues with self.literals.
    let segments = segments.to_vec();

    for (si, seg) in segments.iter().enumerate() {
      let is_last = si == n - 1;

      let call_name = if is_last && wants_newline {
        fun_name
      } else {
        show_sym
      };

      match seg {
        InterpSegment::Literal(sym) => {
          let str_dst = ValueId(self.sir.next_value_id);
          self.sir.next_value_id += 1;

          let sir_val = self.sir.emit(Insn::ConstString {
            dst: str_dst,
            symbol: *sym,
            ty_id: str_ty,
          });

          let call_dst = ValueId(self.sir.next_value_id);
          self.sir.next_value_id += 1;

          self.sir.emit(Insn::Call {
            dst: call_dst,
            name: call_name,
            args: vec![sir_val],
            ty_id: unit_ty,
          });
        }
        InterpSegment::Variable(sym) => {
          // Resolve variable from scope.
          let local_info = self.lookup_local(*sym).map(|l| l.ty_id);

          if let Some(var_ty) = local_info {
            // Always emit a Load — the value may have
            // changed since init (e.g. after a for loop).
            // Use same src encoding as regular variable
            // references: 100 + symbol id.
            let dst = ValueId(self.sir.next_value_id);
            self.sir.next_value_id += 1;

            let sir_val = self.sir.emit(Insn::Load {
              dst,
              src: LoadSource::Local(*sym),
              ty_id: var_ty,
            });

            let call_dst = ValueId(self.sir.next_value_id);
            self.sir.next_value_id += 1;

            self.sir.emit(Insn::Call {
              dst: call_dst,
              name: call_name,
              args: vec![sir_val],
              ty_id: unit_ty,
            });
          } else {
            // Undefined variable in interpolation.
            report_error(Error::new(ErrorKind::UndefinedVariable, span));
          }
        }
      }
    }
  }

  /// Executes a function call.
  fn execute_call(
    &mut self,
    fun_name: Symbol,
    lparen_idx: usize,
    rparen_idx: usize,
  ) {
    // Interpolation desugaring: showln("{x}, {y}") →
    // show(x) + show(", ") + showln(y)
    let name_str = self.interner.get(fun_name);

    if matches!(name_str, "show" | "showln" | "eshow" | "eshowln")
      && self.is_single_interp_arg(lparen_idx, rparen_idx)
    {
      self.execute_interp_call(fun_name, lparen_idx, rparen_idx);

      return;
    }

    // Find the function definition — direct or via closure variable.
    let fun_def = self.funs.iter().find(|f| f.name == fun_name).cloned();
    let (func, capture_count) = if let Some(func) = fun_def {
      let cc = match func.kind {
        FunctionKind::Closure { capture_count } => capture_count,
        _ => 0,
      };

      (Some(func), cc)
    } else {
      // Check if fun_name is a local holding a closure value.
      self.resolve_closure_call(fun_name)
    };

    if let Some(func) = func {
      // Count arguments by commas at depth 0.
      // 0 commas + non-empty = 1 arg, N commas = N+1.
      let has_content = lparen_idx + 1 < rparen_idx;
      let mut comma_count = 0;
      let mut depth = 0;

      for i in (lparen_idx + 1)..rparen_idx {
        match self.tree.nodes[i].token {
          Token::LParen => depth += 1,
          Token::RParen => depth -= 1,
          Token::Comma if depth == 0 => comma_count += 1,
          _ => {}
        }
      }

      let arg_count = if has_content { comma_count + 1 } else { 0 };

      // Type check: correct number of arguments.
      // For closures, user args = total params - capture_count.
      let expected_args = func.params.len() - capture_count as usize;

      if func.kind != FunctionKind::Intrinsic && arg_count != expected_args {
        let span = self.tree.spans[rparen_idx];

        report_error(Error::new(ErrorKind::ArgumentCountMismatch, span));

        return;
      }

      // Pop user arguments from stack (they're in reverse order).
      let mut args = Vec::with_capacity(arg_count);
      let mut arg_types = Vec::with_capacity(arg_count);
      let mut arg_sirs = Vec::with_capacity(arg_count);

      for _ in 0..arg_count {
        if let (Some(val), Some(ty)) =
          (self.value_stack.pop(), self.ty_stack.pop())
        {
          args.push(val);
          arg_types.push(ty);

          if let Some(sir) = self.sir_values.pop() {
            arg_sirs.push(sir);
          }
        }
      }

      // Arguments were in reverse order, fix that.
      args.reverse();
      arg_types.reverse();
      arg_sirs.reverse();

      // For closures, prepend capture values before user args.
      // By-copy: use the SIR values stored at closure
      // creation time, not a fresh Load (which would read
      // the current/mutated value — by-reference).
      if capture_count > 0 {
        let mut full_sirs =
          Vec::with_capacity(capture_count as usize + arg_sirs.len());

        // Look up the ClosureValue for stored capture SIR.
        let closure_captures = self
          .lookup_local(fun_name)
          .and_then(|l| {
            let vi = l.value_id.0 as usize;

            if vi < self.values.kinds.len()
              && matches!(self.values.kinds[vi], Value::Closure)
            {
              let ci = self.values.indices[vi] as usize;

              Some(self.values.closures[ci].captures.clone())
            } else {
              None
            }
          })
          .unwrap_or_default();

        for i in 0..capture_count as usize {
          let (cap_name, cap_ty) = func.params[i];

          // Use stored SIR value if available (by-copy).
          // Fall back to Load (by-reference) if not stored.
          let sv = closure_captures
            .get(i)
            .filter(|c| c.sir_value.0 != u32::MAX)
            .map(|c| c.sir_value)
            .unwrap_or_else(|| {
              let dst = ValueId(self.sir.next_value_id);

              self.sir.next_value_id += 1;

              self.sir.emit(Insn::Load {
                dst,
                src: LoadSource::Local(cap_name),
                ty_id: cap_ty,
              })
            });

          full_sirs.push(sv);
        }

        full_sirs.extend_from_slice(&arg_sirs);

        arg_sirs = full_sirs;
      }

      // For generic functions, create fresh inference vars
      // at each call site so different calls can use
      // different types.
      let mut return_ty = func.return_ty;
      let mut param_types: Vec<TyId> =
        func.params.iter().map(|(_, ty)| *ty).collect();

      let mut subs: Vec<(TyId, TyId)> = Vec::new();

      if !func.type_params.is_empty() {
        // Build substitution: old var → fresh var.
        subs = func
          .type_params
          .iter()
          .map(|old| (*old, self.ty_checker.fresh_var()))
          .collect();

        // Substitute in param types.
        for pty in param_types.iter_mut() {
          for (old, new) in &subs {
            if *pty == *old {
              *pty = *new;
            }
          }
        }

        // Substitute in return type.
        for (old, new) in &subs {
          if return_ty == *old {
            return_ty = *new;
          }
        }
      }

      // Type check user arguments against user parameter types.
      // Skip captures (first capture_count params).
      if func.kind != FunctionKind::Intrinsic {
        let user_param_types = &param_types[capture_count as usize..];

        for (i, (param_ty, arg_ty)) in
          user_param_types.iter().zip(arg_types.iter()).enumerate()
        {
          let span = self.tree.spans[lparen_idx + 1 + i * 2];

          if self.ty_checker.unify(*param_ty, *arg_ty, span).is_none() {
            return;
          }
        }
      }

      // Resolve return type after unification.
      let resolved_ret = self.ty_checker.resolve_id(return_ty);

      // For generic functions, mangle the call name with
      // resolved types so each instantiation gets its own
      // function copy (monomorphization).
      let call_name = if !func.type_params.is_empty() {
        let base = self.interner.get(func.name).to_owned();
        let mut mangled = base;

        for (i, tp) in func.type_params.iter().enumerate() {
          // Use the substituted fresh var (unified with
          // the concrete type), not the original var.
          let actual = subs.get(i).map(|(_, new)| *new).unwrap_or(*tp);
          let resolved = self.ty_checker.resolve_id(actual);
          let ty = self.ty_checker.resolve_ty(resolved);
          let ty_name_owned: String;

          let ty_name = match ty {
            Ty::Int { .. } => "int",
            Ty::Float(_) => "float",
            Ty::Bool => "bool",
            Ty::Str => "str",
            Ty::Char => "char",
            Ty::Struct(sid) => {
              ty_name_owned = self
                .ty_checker
                .ty_table
                .struct_ty(sid)
                .map(|s| self.interner.get(s.name).to_owned())
                .unwrap_or_else(|| "unknown".into());
              &ty_name_owned
            }
            Ty::Enum(eid) => {
              ty_name_owned = self
                .ty_checker
                .ty_table
                .enum_ty(eid)
                .map(|e| self.interner.get(e.name).to_owned())
                .unwrap_or_else(|| "unknown".into());
              &ty_name_owned
            }
            _ => "unknown",
          };

          mangled = format!("{mangled}__{ty_name}");
        }

        let sym = self.interner.intern(&mangled);

        // Record instantiation for the instantiation pass.
        if !self.funs.iter().any(|f| f.name == sym) {
          let mut mono_def = func.clone();

          mono_def.name = sym;
          mono_def.type_params = Vec::new();

          self.funs.push(mono_def);

          // Resolve each fresh var (now unified with the
          // concrete argument type) to its concrete TyId.
          // Stored by `$T` position so the re-execution
          // pass can bind the freshly-parsed type params
          // directly. Snapshotting here — instead of at
          // mono time — insulates us from later mutations
          // of the ty_checker's substitution map.
          let concretes: Vec<TyId> = subs
            .iter()
            .map(|(_, fresh)| self.ty_checker.resolve_id(*fresh))
            .collect();

          self.pending_instantiations.push((
            sym,
            func.name,
            concretes,
            Vec::new(),
          ));
        }

        sym
      } else {
        func.name
      };

      // Closure param monomorphization: when a closure is
      // passed to a Fn-typed parameter, create a specialized
      // copy of the function where `Call { name: param }` is
      // replaced with the concrete closure function name.
      // This enables direct BL without indirect dispatch.
      let call_name = {
        // Build substitution: param_name → closure_fun_name.
        let mut closure_subs: Vec<(Symbol, Symbol)> = Vec::new();

        for (i, arg_val) in args.iter().enumerate() {
          let vi = arg_val.0 as usize;

          if vi < self.values.kinds.len()
            && matches!(self.values.kinds[vi], Value::Closure)
          {
            let ci = self.values.indices[vi] as usize;
            let cv = &self.values.closures[ci];
            let param_name =
              func.params.get(capture_count as usize + i).map(|(n, _)| *n);

            if let Some(name) = param_name {
              closure_subs.push((name, cv.fun_name));
            }
          }
        }

        if !closure_subs.is_empty() {
          // Mangle name with closure identifiers.
          let base = self.interner.get(call_name).to_owned();
          let mut mangled = base;

          for (_, closure_name) in &closure_subs {
            mangled = format!("{mangled}__cl{}", closure_name.as_u32());
          }

          let mono_sym = self.interner.intern(&mangled);

          // Register monomorphized FunDef if not already.
          if !self.funs.iter().any(|f| f.name == mono_sym) {
            let mut mono_def = func.clone();

            mono_def.name = mono_sym;

            // Replace Fn-typed params with the concrete
            // closure's params (keeping captures out).
            for (param_name, closure_fn) in &closure_subs {
              for p in mono_def.params.iter_mut() {
                if p.0 == *param_name {
                  // Change the param name to the closure
                  // function name so resolve_closure_call
                  // or direct lookup works.
                  p.0 = *closure_fn;
                }
              }
            }

            self.funs.push(mono_def);

            // Queue a re-execution request with closure
            // substitutions — the instantiation pass binds
            // each `param_sym` to a `Value::Closure`
            // pointing at the concrete `closure_fn_sym`
            // before the body runs, so emitted Calls name
            // the concrete closure directly (no post-hoc
            // Call-name rewrite). No type substitutions
            // here — closure-param mono is orthogonal to
            // generic-type mono.
            self.pending_instantiations.push((
              mono_sym,
              call_name,
              Vec::new(),
              closure_subs.clone(),
            ));
          }

          mono_sym
        } else {
          call_name
        }
      };

      // Template pretty-print: when showln/show is called
      // with a template argument, replace with a ConstString.
      let call_name_str = self.interner.get(call_name);

      if matches!(call_name_str, "showln" | "show" | "eshowln" | "eshow")
        && args.len() == 1
      {
        // Template pretty-print: trace the SIR Load arg
        // back to its local, check if it's a template, and
        // if so find the Template instruction and format it.
        if let Some(text) = self.resolve_template_text(arg_sirs.first()) {
          let sym = self.interner.intern(&text);
          let str_ty = self.ty_checker.str_type();

          // Use a fresh SIR value id that doesn't collide
          // with template ids (which use value storage
          // indices in a separate numbering space).
          let fresh_id =
            self.sir.next_value_id.max(self.values.kinds.len() as u32);

          let str_dst = ValueId(fresh_id);
          self.sir.next_value_id = fresh_id + 1;

          let str_sir = self.sir.emit(Insn::ConstString {
            dst: str_dst,
            symbol: sym,
            ty_id: str_ty,
          });

          arg_sirs = vec![str_sir];
        }
      }

      // Show abstract dispatch: when showln/show is called
      // with a struct arg that implements Show, insert a
      // Call to Type::show(arg) and use the returned string
      // as the showln argument.
      let call_name_str2 = self.interner.get(call_name);

      if matches!(call_name_str2, "showln" | "show" | "eshowln" | "eshow")
        && arg_types.len() == 1
      {
        let arg_ty = arg_types[0];
        let resolved = self.ty_checker.kind_of(arg_ty);

        let type_name = match resolved {
          Ty::Struct(sid) => {
            self.ty_checker.ty_table.struct_ty(sid).map(|s| s.name)
          }
          _ => None,
        };

        if let Some(tname) = type_name {
          let show_sym = self.interner.intern("Show");

          if self.abstract_impls.contains_key(&(show_sym, tname)) {
            let ts = self.interner.get(tname).to_owned();
            let mangled = format!("{ts}::show");
            let show_fn = self.interner.intern(&mangled);

            if self.funs.iter().any(|f| f.name == show_fn) {
              let show_dst = ValueId(self.sir.next_value_id);
              self.sir.next_value_id += 1;

              let show_result = self.sir.emit(Insn::Call {
                dst: show_dst,
                name: show_fn,
                args: arg_sirs.clone(),
                ty_id: self.ty_checker.str_type(),
              });

              arg_sirs = vec![show_result];
            }
          }
        }
      }

      let dst = ValueId(self.sir.next_value_id);
      self.sir.next_value_id += 1;

      let result_sir = self.sir.emit(Insn::Call {
        dst,
        name: call_name,
        args: arg_sirs,
        ty_id: resolved_ret,
      });

      // Push return value.
      if resolved_ret != self.ty_checker.unit_type() {
        let result_val = self.values.store_runtime(0);

        self.value_stack.push(result_val);
        self.ty_stack.push(resolved_ret);
        self.sir_values.push(result_sir);
      }

      // For ext functions with parameterized return types,
      // stash the type args so the match handler can use
      // them for binding types instead of the enum's
      // unresolved generic field vars.
      if !func.return_type_args.is_empty() {
        // Store under the variable name (if pending decl)
        // AND under the enum type name for match lookup.
        if let Some(ref decl) = self.pending_decl {
          self
            .var_return_type_args
            .insert(decl.name.as_u32(), func.return_type_args.clone());
        }

        // Also store under the return type's enum name
        // so `match` can find it when scrutinee_sym is None.
        if let Ty::Enum(eid) = self.ty_checker.kind_of(func.return_ty)
          && let Some(et) = self.ty_checker.ty_table.enum_ty(eid)
        {
          self
            .var_return_type_args
            .insert(et.name.as_u32(), func.return_type_args.clone());
        }
      }
    } else {
      // Function not found in definitions - might be external/builtin
      // Count arguments between LParen and RParen
      let mut arg_count = 0;
      let mut idx = lparen_idx + 1;

      while idx < rparen_idx {
        let token = &self.tree.nodes[idx].token;

        match token {
          Token::Comma => {}
          Token::LParen | Token::RParen => {
            // Skip nested parens
            let mut depth = 1;

            if *token == Token::LParen {
              idx += 1;

              while idx < rparen_idx && depth > 0 {
                match self.tree.nodes[idx].token {
                  Token::LParen => depth += 1,
                  Token::RParen => depth -= 1,
                  _ => {}
                }

                idx += 1;
              }

              continue;
            }
          }
          _ => {
            // This is an argument
            arg_count += 1;
          }
        }

        idx += 1;
      }

      // Pop arguments from stack
      let mut arg_sirs = Vec::with_capacity(arg_count);

      for _ in 0..arg_count {
        self.value_stack.pop();
        self.ty_stack.pop();

        if let Some(sir) = self.sir_values.pop() {
          arg_sirs.push(sir);
        }
      }

      arg_sirs.reverse();

      // Check if the unresolved function is a Fn-typed
      // parameter. If so, use its return type and push the
      // result for monomorphization at call sites.
      let return_ty = self
        .lookup_local(fun_name)
        .and_then(|l| {
          let ty = self.ty_checker.resolve_ty(l.ty_id);

          if let Ty::Fun(fid) = ty {
            self.ty_checker.ty_table.fun(&fid).map(|ft| ft.return_ty)
          } else {
            None
          }
        })
        .unwrap_or_else(|| self.ty_checker.unit_type());

      let dst = ValueId(self.sir.next_value_id);
      self.sir.next_value_id += 1;

      let result_sir = self.sir.emit(Insn::Call {
        dst,
        name: fun_name,
        args: arg_sirs,
        ty_id: return_ty,
      });

      // Push return value if non-unit.
      if return_ty != self.ty_checker.unit_type() {
        let result_val = self.values.store_runtime(0);

        self.value_stack.push(result_val);
        self.ty_stack.push(return_ty);
        self.sir_values.push(result_sir);
      }
    }
  }

  /// Executes a modified check call: check@op(lhs, rhs).
  /// Desugars to: BinOp(lhs, op, rhs) -> Call("check", [bool]).
  fn execute_check_modifier(
    &mut self,
    fun_name: Symbol,
    modifier: Symbol,
    lparen_idx: usize,
    rparen_idx: usize,
  ) {
    let base_name = self.interner.get(fun_name);

    if base_name != "check" {
      // Only check supports modifiers for now.
      self.execute_call(fun_name, lparen_idx, rparen_idx);

      return;
    }

    let mod_name = self.interner.get(modifier);

    let op = match mod_name {
      "lt" => zo_sir::BinOp::Lt,
      "le" => zo_sir::BinOp::Lte,
      "gt" => zo_sir::BinOp::Gt,
      "ge" => zo_sir::BinOp::Gte,
      "eq" => zo_sir::BinOp::Eq,
      "ne" => zo_sir::BinOp::Neq,
      _ => {
        let span = self.tree.spans[rparen_idx];

        report_error(Error::new(ErrorKind::UnexpectedToken, span));

        return;
      }
    };

    // Pop 2 arguments from stack (reversed order).
    let (rhs_val, rhs_ty, rhs_sir) = match (
      self.value_stack.pop(),
      self.ty_stack.pop(),
      self.sir_values.pop(),
    ) {
      (Some(v), Some(t), Some(s)) => (v, t, s),
      _ => return,
    };

    let (_lhs, lhs_ty, lhs_sir) = match (
      self.value_stack.pop(),
      self.ty_stack.pop(),
      self.sir_values.pop(),
    ) {
      (Some(v), Some(t), Some(s)) => (v, t, s),
      _ => {
        // Restore rhs if lhs pop failed.
        self.value_stack.push(rhs_val);
        self.ty_stack.push(rhs_ty);
        self.sir_values.push(rhs_sir);

        return;
      }
    };

    // If lhs is a template, resolve to string for comparison.
    let (lhs_ty, lhs_sir) = if let Some(text) =
      self.resolve_template_text(Some(&lhs_sir))
    {
      let sym = self.interner.intern(&text);
      let str_ty = self.ty_checker.str_type();

      let fresh_id = self.sir.next_value_id.max(self.values.kinds.len() as u32);
      let str_dst = ValueId(fresh_id);

      self.sir.next_value_id = fresh_id + 1;

      let str_sir = self.sir.emit(Insn::ConstString {
        dst: str_dst,
        symbol: sym,
        ty_id: str_ty,
      });

      (str_ty, str_sir)
    } else {
      (lhs_ty, lhs_sir)
    };

    // Narrow default-typed int literals to the other operand's
    // integer type. Handles `check@eq(x, 10)` where `x: uint`
    // but the literal `10` was parsed as the default `int`.
    let (lhs_ty, rhs_ty) = {
      let mut lt = lhs_ty;
      let mut rt = rhs_ty;

      if self.narrow_int_literal(rhs_sir, rt, lt) {
        rt = lt;
      } else if self.narrow_int_literal(lhs_sir, lt, rt) {
        lt = rt;
      }

      (lt, rt)
    };

    // Unify operand types.
    let span = self.tree.spans[lparen_idx];

    let ty_id = match self.ty_checker.unify(lhs_ty, rhs_ty, span) {
      Some(t) => t,
      None => return,
    };

    // Emit comparison BinOp.
    let cmp_dst = ValueId(self.sir.next_value_id);

    self.sir.next_value_id += 1;

    let cmp_sir = self.sir.emit(Insn::BinOp {
      dst: cmp_dst,
      op,
      lhs: lhs_sir,
      rhs: rhs_sir,
      ty_id,
    });

    // Emit Call("check", [cmp_result]).
    let check_func = self.funs.iter().find(|f| f.name == fun_name).cloned();

    let return_ty = check_func
      .map(|f| f.return_ty)
      .unwrap_or_else(|| self.ty_checker.unit_type());

    let dst = ValueId(self.sir.next_value_id);
    self.sir.next_value_id += 1;

    self.sir.emit(Insn::Call {
      dst,
      name: fun_name,
      args: vec![cmp_sir],
      ty_id: return_ty,
    });
  }

  /// Executes a `$: { ... }` or `pub $: { ... }` style block.
  ///
  /// Walks the tree children to extract style rules, builds
  /// a `zo_styler::StyleSheet`, compiles it to CSS, and pushes
  /// `UiCommand::StyleSheet` to `pending_styles`.
  fn execute_style_block(&mut self, start_idx: usize, end_idx: usize) {
    let scope = if self.is_pub(start_idx) {
      StyleScope::Global
    } else {
      StyleScope::Scoped
    };

    let mut rules = Vec::new();
    let mut idx = start_idx + 1; // skip Dollar

    // Skip Colon.
    if idx < end_idx && self.tree.nodes[idx].token == Token::Colon {
      idx += 1;
    }

    // Skip outer LBrace.
    if idx < end_idx && self.tree.nodes[idx].token == Token::LBrace {
      idx += 1;
    }

    // Parse rules until outer RBrace.
    while idx < end_idx {
      if self.tree.nodes[idx].token == Token::RBrace {
        break;
      }

      // Collect selector tokens until LBrace.
      let mut selector = String::new();

      while idx < end_idx {
        let t = self.tree.nodes[idx].token;

        if t == Token::LBrace {
          break;
        }

        match t {
          Token::Dot => selector.push('.'),
          Token::Hash => selector.push('#'),
          Token::Comma => selector.push_str(", "),
          Token::Ident => {
            if let Some(NodeValue::Symbol(sym)) = self.node_value(idx) {
              if !selector.is_empty()
                && !selector.ends_with('.')
                && !selector.ends_with('#')
                && !selector.ends_with(' ')
              {
                selector.push(' ');
              }

              selector.push_str(self.interner.get(sym));
            }
          }
          _ => {}
        }

        idx += 1;
      }

      // Skip inner LBrace.
      if idx < end_idx && self.tree.nodes[idx].token == Token::LBrace {
        idx += 1;
      }

      // Parse property declarations until RBrace.
      let mut props = Vec::new();

      while idx < end_idx {
        let t = self.tree.nodes[idx].token;

        if t == Token::RBrace {
          idx += 1;
          break;
        }

        if t == Token::Ident {
          let name = self.symbol_str(idx);

          idx += 1;

          // Colon.
          if idx < end_idx && self.tree.nodes[idx].token == Token::Colon {
            idx += 1;
          }

          // StyleValue.
          let value = if idx < end_idx
            && self.tree.nodes[idx].token == Token::StyleValue
          {
            let v = self.symbol_str(idx);

            idx += 1;
            v
          } else {
            String::new()
          };

          // Semicolon.
          if idx < end_idx && self.tree.nodes[idx].token == Token::Semicolon {
            idx += 1;
          }

          props.push(zo_styler::StyleProp { name, value });
        } else {
          idx += 1;
        }
      }

      rules.push(zo_styler::StyleRule { selector, props });
    }

    // Generate scope hash for scoped stylesheets.
    // Use the span start as a unique-enough seed — each
    // style block has a distinct position in the source.
    let scope_hash = if scope == StyleScope::Scoped {
      let span = self.tree.spans[start_idx];
      let mut seed = [0u8; 6];

      seed[0..4].copy_from_slice(&span.start.to_le_bytes());
      seed[4..6].copy_from_slice(&span.len.to_le_bytes());

      Some(zo_styler::scope_hash(&seed))
    } else {
      None
    };

    let sheet = zo_styler::StyleSheet {
      rules,
      scope_hash: scope_hash.clone(),
    };

    let css = zo_styler::compile(&sheet);

    // Emit to SIR for the canonical IR record.
    self.sir.emit(Insn::StyleSheet {
      css: css.clone(),
      scope: scope.clone(),
      scope_hash: scope_hash.clone(),
    });

    // Push to pending_styles for injection into the next
    // template's UiCommand list.
    self.pending_styles.push(UiCommand::StyleSheet {
      css,
      scope,
      scope_hash,
    });
  }

  fn execute_directive(&mut self, start_idx: usize, end_idx: usize) {
    // Directives: #identifier [expression]
    // Children come after Hash in the tree. We skip
    // them in the main loop (skip_until) and execute
    // the argument nodes here.

    if start_idx + 1 >= end_idx {
      return;
    }

    // First child is the directive name.
    let dir_idx = start_idx + 1;

    if dir_idx >= self.tree.nodes.len()
      || self.tree.nodes[dir_idx].token != Token::Ident
    {
      return;
    }

    let sym = match self.node_value(dir_idx) {
      Some(NodeValue::Symbol(s)) => s,
      _ => return,
    };

    let dir_name = self.interner.get(sym).to_owned();

    // `#dom <ident>`: resolve the identifier directly to
    // its local's `value_id`. For a template local, that
    // value IS the `Insn::Template { id, .. }` id, which
    // both codegen (`emit_render_call`) and the driver
    // (component-aware template selection) rely on.
    //
    // Executing the Ident through `execute_node` would
    // emit an `Insn::Load` whose fresh dst is unrelated
    // to the template id — the driver would then fail to
    // match the directive to a template, and would render
    // nothing (or, before this fix, silently fell back to
    // rendering every template in the SIR).
    if dir_name == "dom" {
      let target_idx = ((dir_idx + 1)..end_idx)
        .find(|&i| self.tree.nodes[i].token == Token::Ident);

      if let Some(ti) = target_idx
        && let Some(NodeValue::Symbol(target_sym)) = self.node_value(ti)
        && let Some(local) = self.lookup_local(target_sym)
      {
        self.sir.emit(Insn::Directive {
          name: sym,
          value: local.value_id,
          ty_id: local.ty_id,
        });
      }

      return;
    }

    // Execute argument children (after the name).
    // Skip Semicolon — it's syntactic, not a statement
    // terminator inside a directive.
    for i in (dir_idx + 1)..end_idx {
      let node = self.tree.nodes[i];

      if node.token == Token::Semicolon {
        continue;
      }

      self.execute_node(&node, i);
    }

    match dir_name.as_str() {
      "run" => {}
      "inline" => {}
      _ => {}
    }
  }

  /// Re-execute the Tree subtree of each generic function
  /// once per concrete-type instantiation.
  ///
  /// Instead of cloning the generic's already-emitted SIR
  /// and rewriting `ty_id` fields through substitutions,
  /// this pass replays the Tree range recorded in
  /// `generic_tree_ranges` with the freshly-parsed `$T`
  /// inference vars bound to concrete `TyId`s. The body
  /// then emits fresh SIR as if a hand-written `sum__int`
  /// had been declared — every `ty_id`, every `ValueId`,
  /// every abstract method resolution falls out naturally.
  ///
  /// Carbon-aligned: semantic analysis IS execution
  /// (manifesto line 176). Generics aren't a rewrite
  /// problem — they're a "run the body again with a
  /// different type environment" problem.
  fn reexecute_generic_instantiations(&mut self) {
    let pending = std::mem::take(&mut self.pending_instantiations);

    for (mangled, generic_name, concretes, closure_subs) in pending {
      // Skip duplicates (can arise from repeated call sites
      // with the same concrete types, or recursive generics).
      if self.reexecuted_instantiations.contains(&mangled) {
        continue;
      }

      let Some(&(range_start, range_end)) =
        self.generic_tree_ranges.get(&generic_name)
      else {
        // No recorded tree range — the generic was defined
        // in a context we don't track yet (e.g. imported
        // from another module's preload). Fall back to the
        // legacy clone path for this one by leaving
        // `reexecuted_instantiations` unmodified.
        continue;
      };

      let fun_idx = range_start as usize;
      let end_idx = range_end as usize;

      // --- Save executor state --------------------------------
      //
      // The re-execution runs `execute_fun` and the body
      // handlers against the SAME tree range that was already
      // executed in the outer pass. Every piece of mutable
      // state those handlers touch must be snapshotted so
      // the outer execution sees no side effects when we're
      // done.
      let saved_skip = self.skip_until;
      let saved_pending_decl = self.pending_decl.take();
      let saved_pending_function = self.pending_function.take();
      let saved_pending_fn_has_return = self.pending_fn_has_return_annotation;
      let saved_current_function = self.current_function.take();
      let saved_value_stack = std::mem::take(&mut self.value_stack);
      let saved_ty_stack = std::mem::take(&mut self.ty_stack);
      let saved_sir_values = std::mem::take(&mut self.sir_values);
      let saved_type_params = std::mem::take(&mut self.type_params);
      let saved_type_constraints = std::mem::take(&mut self.type_constraints);
      let saved_array_ctx = std::mem::take(&mut self.array_ctx);
      let saved_tuple_ctx = std::mem::take(&mut self.tuple_ctx);
      let saved_direct_call_depth = self.direct_call_depth;
      self.direct_call_depth = 0;
      let saved_branch_stack = std::mem::take(&mut self.branch_stack);
      let saved_locals_len = self.locals.len();
      let saved_scope_len = self.scope_stack.len();
      let saved_apply_ctx = self.apply_context;

      // Mark this instantiation as emitted BEFORE the body
      // runs — if the body contains a recursive call that
      // lands back in this pass, the duplicate-skip above
      // bails out instead of looping forever.
      self.reexecuted_instantiations.insert(mangled);

      // Remove the call-site's mono FunDef entry from
      // `self.funs` — re-execution will push a fresh one.
      self.funs.retain(|f| f.name != mangled);

      // Tell the next `execute_fun` to emit the mangled
      // name instead of the tree's literal generic name.
      self.mono_name_override = Some(mangled);

      // Snapshot the SIR boundary so we know which range to
      // rewrite-for-concretization after the body runs.
      let sir_boundary = self.sir.instructions.len();

      // --- Drive the sub-loop -------------------------------
      //
      // First node is `Fun`: execute it to parse the
      // signature. That populates `self.type_params` with
      // freshly-minted inference vars for each `$T`.
      self.skip_until = 0;

      let fun_header = self.tree.nodes[fun_idx];

      self.execute_node(&fun_header, fun_idx);

      // Bind each freshly-parsed `$T` fresh var to its
      // concrete type for this instantiation. `kind_of`
      // (and `resolve_id`) now return the concrete type for
      // any `ty_id` that flows through the body — no
      // placeholder `__abstract::` tricks needed.
      let installed: Vec<TyId> = self
        .type_params
        .iter()
        .map(|(_, ty)| *ty)
        .zip(concretes.iter())
        .map(|(fresh, concrete)| {
          self.ty_checker.install_substitution(fresh, *concrete);

          fresh
        })
        .collect();

      // Closure-param substitution: rewrite each
      // `Fn`-typed parameter local from its runtime
      // placeholder to a `Value::Closure` that references
      // the concrete closure function. When the body
      // later says `f(x)`, the Ident lookup picks up the
      // closure binding and the call emits
      // `Call { name: closure_fn }` directly — no
      // `Call { name: param_sym }` + post-hoc rewrite.
      for (param_sym, closure_fn_sym) in &closure_subs {
        if let Some(local) =
          self.locals.iter_mut().rev().find(|l| l.name == *param_sym)
        {
          let cv = ClosureValue {
            fun_name: *closure_fn_sym,
            captures: Vec::new(),
          };

          local.value_id = self.values.store_closure(cv);
        }
      }

      // Walk the rest of the tree range exactly as the
      // main execute loop does — `execute_fun` has already
      // set `skip_until` past the signature, so the next
      // idx that actually executes is the `LBrace`.
      let mut idx = fun_idx + 1;

      while idx < end_idx {
        if idx < self.skip_until {
          idx += 1;
          continue;
        }

        let header = self.tree.nodes[idx];

        self.execute_node(&header, idx);

        if self.pending_call_rparen == Some(idx) {
          self.pending_call_rparen = None;
        }

        if self.tuple_ctx.is_empty() && self.pending_call_rparen.is_none() {
          self.apply_deferred_binop();
        }

        idx += 1;
      }

      // Concretize `ty_id` fields in the emitted body
      // BEFORE the substitutions are cleared. The SIR
      // instructions carry fresh-var `TyId`s; after the
      // substitutions are gone, `resolve_id` on those
      // TyIds would again yield `Ty::Infer(..)`. Resolving
      // them in place now bakes concrete types into the
      // emitted SIR permanently — satisfying invariant 3
      // ("ty_ids are concrete at instantiation
      // boundaries") end-to-end.
      let end_sir = self.sir.instructions.len();

      for insn in &mut self.sir.instructions[sir_boundary..end_sir] {
        insn.visit_ty_ids_mut(&mut |id| {
          *id = self.ty_checker.resolve_id(*id);
        });
      }

      // --- Tear down substitutions + restore state -----------
      for fresh in installed.iter().rev() {
        self.ty_checker.clear_substitution(*fresh, None);
      }

      self.skip_until = saved_skip;
      self.pending_decl = saved_pending_decl;
      self.pending_function = saved_pending_function;
      self.pending_fn_has_return_annotation = saved_pending_fn_has_return;
      self.current_function = saved_current_function;
      self.value_stack = saved_value_stack;
      self.ty_stack = saved_ty_stack;
      self.sir_values = saved_sir_values;
      self.type_params = saved_type_params;
      self.type_constraints = saved_type_constraints;
      self.array_ctx = saved_array_ctx;
      self.tuple_ctx = saved_tuple_ctx;
      self.direct_call_depth = saved_direct_call_depth;
      self.branch_stack = saved_branch_stack;
      self.locals.truncate(saved_locals_len);
      self.scope_stack.truncate(saved_scope_len);
      self.apply_context = saved_apply_ctx;
    }
  }

  /// Converts a ValueId to its string representation.
  /// Used by template interpolation and showln.
  fn value_to_string(&self, value_id: ValueId) -> String {
    let vi = value_id.0 as usize;

    if vi >= self.values.kinds.len() {
      return String::new();
    }

    match self.values.kinds[vi] {
      Value::String => {
        let si = self.values.indices[vi] as usize;
        let sym = self.values.strings[si];

        self.interner.get(sym).to_string()
      }
      Value::Int => {
        let ii = self.values.indices[vi] as usize;

        self.values.ints[ii].to_string()
      }
      Value::Float => {
        let fi = self.values.indices[vi] as usize;

        self.values.floats[fi].to_string()
      }
      Value::Bool => {
        let bi = self.values.indices[vi] as usize;

        if self.values.bools[bi] {
          "true".to_string()
        } else {
          "false".to_string()
        }
      }
      Value::Char => {
        let ci = self.values.indices[vi] as usize;

        self.values.chars[ci].to_string()
      }
      Value::Template => {
        let ti = self.values.indices[vi] as usize;
        let template_ref = self.values.templates[ti];

        // Find the Template instruction in SIR and
        // pretty-print its commands.
        for insn in &self.sir.instructions {
          if let Insn::Template { id, commands, .. } = insn
            && id.0 == value_id.0
          {
            return Self::pretty_print_commands(commands);
          }
        }

        format!("<template #{template_ref}>")
      }
      _ => String::new(),
    }
  }

  /// Pretty-prints template UI commands as HTML-like text.
  /// Tracks a mini open-element stack so that `EndElement` can
  /// recover the matching tag name for the closing `</tag>`.
  fn pretty_print_commands(commands: &[UiCommand]) -> String {
    let mut out = String::new();
    let mut stack: Vec<&str> = Vec::new();

    for cmd in commands {
      match cmd {
        UiCommand::Element {
          tag,
          attrs,
          self_closing,
        } => {
          let name = tag.as_str();

          out.push('<');
          out.push_str(name);

          for attr in attrs {
            match attr {
              Attr::Prop { name, value } if !name.starts_with("data-") => {
                out.push_str(&format!(" {name}=\"{}\"", value.to_display()));
              }
              Attr::Dynamic { name, initial, .. }
                if !name.starts_with("data-") =>
              {
                out.push_str(&format!(" {name}=\"{}\"", initial.to_display()));
              }
              _ => {}
            }
          }

          if *self_closing {
            out.push_str(" />");
          } else {
            out.push('>');
            stack.push(name);
          }
        }
        UiCommand::EndElement => {
          if let Some(name) = stack.pop() {
            out.push_str(&format!("</{name}>"));
          }
        }
        UiCommand::Text(s) => {
          out.push_str(s);
        }
        UiCommand::Event { .. } | UiCommand::StyleSheet { .. } => {}
      }
    }

    out
  }

  /// Resolves a SIR argument to template text if it's a
  /// template variable. Traces Load → local → Value::Template
  /// → Insn::Template commands → pretty-print. Returns None
  /// if the argument is not a template.
  fn resolve_template_text(&self, sir_vid: Option<&ValueId>) -> Option<String> {
    let sir_vid = sir_vid?;

    // Find the Load instruction for this SIR value.
    let sym = self.sir.instructions.iter().find_map(|insn| {
      if let Insn::Load {
        dst,
        src: LoadSource::Local(sym),
        ..
      } = insn
        && dst == sir_vid
      {
        Some(*sym)
      } else {
        None
      }
    })?;

    // Check if the local's value is a template.
    let local = self.locals.iter().rev().find(|l| l.name == sym)?;
    let lvi = local.value_id.0 as usize;

    if lvi >= self.values.kinds.len()
      || !matches!(self.values.kinds[lvi], Value::Template)
    {
      return None;
    }

    // Find the Template instruction matching this local's
    // ValueId — not the last one globally.
    let target_id = local.value_id;

    self.sir.instructions.iter().find_map(|i| match i {
      Insn::Template { id, commands, .. }
        if *id == target_id && !commands.is_empty() =>
      {
        Some(Self::pretty_print_commands(commands))
      }
      _ => None,
    })
  }

  fn execute_template_assign(&mut self, _start_idx: usize, _end_idx: usize) {
    // Template assignment: ::= switches parser to template mode.
    // Find the TemplateFragmentStart forward in the flat tree
    // (it's a sibling, not a child of ::=) and execute it.
    for idx in (_start_idx + 1)..self.tree.nodes.len() {
      let tok = self.tree.nodes[idx].token;

      if tok == Token::TemplateFragmentStart {
        let header = self.tree.nodes[idx];

        self.execute_node(&header, idx);

        break;
      }

      // Stop if we hit a statement boundary.
      if tok == Token::Semicolon || tok == Token::RBrace {
        break;
      }
    }
  }

  fn execute_template_fragment(&mut self, start_idx: usize, end_idx: usize) {
    let mut commands = Vec::new();

    // Walk the flat token stream with a cursor, building
    // UiCommands via tag registry + attribute extraction.
    let mut idx = start_idx + 1;

    while idx < end_idx {
      let node = &self.tree.nodes[idx];

      match node.token {
        Token::TemplateText => {
          if let Some(NodeValue::Symbol(sym)) = self.node_value(idx) {
            let text = self.interner.get(sym).to_string();

            if !text.trim().is_empty() {
              commands.push(UiCommand::Text(text));
            }
          }

          idx += 1;
        }
        Token::TemplateFragmentEnd => break,
        Token::LAngle => {
          // Opening tag or closing tag.
          idx += 1;
          if idx >= end_idx {
            break;
          }

          let next = &self.tree.nodes[idx];

          if next.token == Token::Slash2 {
            // Closing tag: </ ident >
            // Skip slash, tag name, and closing >
            idx += 1; // skip ident
            if idx < end_idx && self.tree.nodes[idx].token == Token::Ident {
              idx += 1; // skip past ident
            }
            if idx < end_idx && self.tree.nodes[idx].token == Token::RAngle {
              idx += 1;
            }
            self.close_template_tag(&mut commands);
          } else if next.token == Token::Ident {
            // Opening tag: < ident [attrs...] > or
            //              < ident [attrs...] / >
            let tag_name = self
              .node_value(idx)
              .and_then(|v| match v {
                NodeValue::Symbol(s) => Some(s),
                _ => None,
              })
              .map(|s| self.interner.get(s).to_string())
              .unwrap_or_default();

            idx += 1;

            // Extract typed attributes until > or />
            let mut attrs = Vec::with_capacity(4);
            let mut self_closing = false;

            while idx < end_idx {
              let n = &self.tree.nodes[idx];
              match n.token {
                Token::RAngle => {
                  idx += 1;
                  break;
                }
                Token::Slash2 => {
                  self_closing = true;
                  idx += 1;
                  if idx < end_idx
                    && self.tree.nodes[idx].token == Token::RAngle
                  {
                    idx += 1;
                  }
                  break;
                }
                Token::Ident => {
                  let attr_name = self
                    .node_value(idx)
                    .and_then(|v| match v {
                      NodeValue::Symbol(s) => Some(s),
                      _ => None,
                    })
                    .map(|s| self.interner.get(s).to_string())
                    .unwrap_or_default();

                  idx += 1;

                  // name="value" pair
                  if idx < end_idx && self.tree.nodes[idx].token == Token::Eq {
                    idx += 1;
                  }

                  if idx < end_idx
                    && self.tree.nodes[idx].token == Token::String
                  {
                    let raw = self
                      .node_value(idx)
                      .and_then(|v| match v {
                        NodeValue::Symbol(s) => Some(s),
                        _ => None,
                      })
                      .map(|s| self.interner.get(s).to_string())
                      .unwrap_or_default();
                    idx += 1;
                    attrs.push(Attr::parse_prop(&attr_name, &raw));
                  } else if idx < end_idx
                    && self.tree.nodes[idx].token == Token::InterpString
                  {
                    // Attribute value is an interpolated string
                    // literal, e.g. `alt="a picture of '{name}'"`.
                    // Walk the pre-parsed InterpSegment list from
                    // the tokenizer side-table, resolving each
                    // `Variable(sym)` segment against the local
                    // scope, and concatenate the result.
                    let resolved = self.resolve_interp_string_attr(idx);

                    idx += 1;
                    attrs.push(Attr::parse_prop(&attr_name, &resolved));
                  } else if idx < end_idx
                    && self.tree.nodes[idx].token == Token::LBrace
                  {
                    // Attribute interpolation: attr={expr}.
                    idx += 1;

                    // Fast path: single-identifier expression
                    // resolves directly to the local for both
                    // its compile-time value AND its reactive
                    // metadata (mut → `Attr::Dynamic`).
                    let single_ident_sym = if idx < end_idx
                      && self.tree.nodes[idx].token == Token::Ident
                      && idx + 1 < end_idx
                      && self.tree.nodes[idx + 1].token == Token::RBrace
                      && let Some(NodeValue::Symbol(sym)) = self.node_value(idx)
                    {
                      Some(sym)
                    } else {
                      None
                    };

                    if let Some(sym) = single_ident_sym {
                      attrs.push(self.make_attr_from_local(&attr_name, sym));
                      idx += 1; // past ident
                    } else {
                      // General expression — eager-only, no
                      // reactive tracking.
                      while idx < end_idx
                        && self.tree.nodes[idx].token != Token::RBrace
                      {
                        let n = self.tree.nodes[idx];

                        self.execute_node(&n, idx);
                        idx += 1;
                      }

                      let val = if let Some(vid) = self.value_stack.pop() {
                        self.ty_stack.pop();
                        self.sir_values.pop();

                        self.value_to_string(vid)
                      } else {
                        String::new()
                      };

                      attrs.push(Attr::parse_prop(&attr_name, &val));
                    }

                    if idx < end_idx
                      && self.tree.nodes[idx].token == Token::RBrace
                    {
                      idx += 1;
                    }
                  } else {
                    // Boolean attribute: <input disabled />
                    attrs.push(Attr::Prop {
                      name: attr_name,
                      value: PropValue::Bool(true),
                    });
                  }
                }
                Token::At => {
                  // @click={handler} — event binding
                  idx += 1;

                  if idx < end_idx && self.tree.nodes[idx].token == Token::Ident
                  {
                    let event_name = self
                      .node_value(idx)
                      .and_then(|v| match v {
                        NodeValue::Symbol(s) => Some(s),
                        _ => None,
                      })
                      .map(|s| self.interner.get(s).to_string())
                      .unwrap_or_default();

                    idx += 1;

                    // Expect ={handler}
                    if idx < end_idx && self.tree.nodes[idx].token == Token::Eq
                    {
                      idx += 1;
                    }

                    // { handler_ident }
                    if idx < end_idx
                      && self.tree.nodes[idx].token == Token::LBrace
                    {
                      idx += 1;
                    }

                    let handler = if idx < end_idx
                      && self.tree.nodes[idx].token == Token::Ident
                    {
                      let h = self
                        .node_value(idx)
                        .and_then(|v| match v {
                          NodeValue::Symbol(s) => Some(s),
                          _ => None,
                        })
                        .map(|s| self.interner.get(s).to_string())
                        .unwrap_or_default();
                      idx += 1;
                      h
                    } else if idx < end_idx
                      && self.tree.nodes[idx].token == Token::Fn
                    {
                      // Inline closure as event handler:
                      // @click={fn() => expr}
                      let header = self.tree.nodes[idx];
                      let children_end =
                        (header.child_start + header.child_count) as usize;

                      self.execute_closure(idx, children_end);

                      // Pop the closure value and extract its
                      // generated function name.
                      let h = self
                        .value_stack
                        .pop()
                        .and_then(|vid| {
                          let vi = vid.0 as usize;

                          if vi < self.values.kinds.len()
                            && matches!(self.values.kinds[vi], Value::Closure)
                          {
                            let ci = self.values.indices[vi] as usize;
                            let name = self.values.closures[ci].fun_name;

                            Some(self.interner.get(name).to_string())
                          } else {
                            None
                          }
                        })
                        .unwrap_or_default();

                      // Pop the type and SIR value that
                      // execute_closure pushed.
                      self.ty_stack.pop();
                      self.sir_values.pop();

                      idx = children_end;
                      h
                    } else {
                      String::new()
                    };

                    if idx < end_idx
                      && self.tree.nodes[idx].token == Token::RBrace
                    {
                      idx += 1;
                    }
                    let event_kind = match event_name.as_str() {
                      "click" => EventKind::Click,
                      "hover" => EventKind::Hover,
                      "change" => EventKind::Change,
                      "input" => EventKind::Input,
                      "focus" => EventKind::Focus,
                      "blur" => EventKind::Blur,
                      _ => EventKind::Click,
                    };

                    attrs.push(Attr::Event {
                      name: event_name,
                      event_kind,
                      handler,
                    });
                  }
                }
                Token::Eq => {
                  idx += 1;
                }
                // Shorthand attribute: `<tag {ident} />` — sugar
                // for `<tag ident={ident}>`. Resolves the local
                // for both its compile-time value AND reactive
                // metadata (mut → `Attr::Dynamic`).
                Token::LBrace => {
                  idx += 1;

                  if idx < end_idx
                    && self.tree.nodes[idx].token == Token::Ident
                    && let Some(NodeValue::Symbol(sym)) = self.node_value(idx)
                  {
                    let shorthand_name = self.interner.get(sym).to_string();

                    idx += 1;

                    if idx < end_idx
                      && self.tree.nodes[idx].token == Token::RBrace
                    {
                      idx += 1;
                    }

                    attrs.push(self.make_attr_from_local(&shorthand_name, sym));
                  } else {
                    // Malformed shorthand — skip to matching
                    // close brace to stay in sync with the
                    // template token stream.
                    while idx < end_idx
                      && self.tree.nodes[idx].token != Token::RBrace
                    {
                      idx += 1;
                    }

                    if idx < end_idx {
                      idx += 1;
                    }
                  }
                }
                _ => {
                  idx += 1;
                }
              }
            }

            self.emit_opening_tag(
              &tag_name,
              &attrs,
              self_closing,
              &mut commands,
            );
          } else {
            idx += 1;
          }
        }
        // Template interpolation: {expr}.
        // Execute tokens between { and } as a normal zo
        // expression, convert the result to string, and
        // append as UiCommand::Text.
        Token::LBrace => {
          let brace_span = self.tree.spans[idx];

          idx += 1;

          // Detect empty braces {}.
          if idx < end_idx && self.tree.nodes[idx].token == Token::RBrace {
            report_error(Error::new(ErrorKind::ExpectedExpression, brace_span));
            idx += 1;
          } else if self.try_handle_html_directive(
            &mut idx,
            end_idx,
            &mut commands,
            brace_span,
          ) {
            // `{#html expr}` consumed through matching `}`.
          } else {
            // Execute expression tokens until matching }.
            // For simple identifiers, resolve the local's
            // original value directly (not the runtime Load).
            let mut interp_text = None;

            while idx < end_idx && self.tree.nodes[idx].token != Token::RBrace {
              let n = self.tree.nodes[idx];

              // Simple identifier — resolve the local's
              // compile-time value for template embedding.
              if n.token == Token::Ident
                && interp_text.is_none()
                && let Some(NodeValue::Symbol(sym)) = self.node_value(idx)
                && let Some(local) =
                  self.locals.iter().rev().find(|l| l.name == sym)
              {
                let text = self.value_to_string(local.value_id);

                if !text.is_empty() {
                  interp_text = Some(text);
                }

                // Track reactive text binding for mut vars.
                if local.mutability == Mutability::Yes {
                  self.template_bindings.text.push((commands.len(), sym));
                }
              }

              if interp_text.is_none() {
                self.execute_node(&n, idx);
              }

              idx += 1;
            }

            // Skip the closing }.
            if idx < end_idx && self.tree.nodes[idx].token == Token::RBrace {
              idx += 1;
            }

            // Use resolved text, or fall back to executed
            // expression result.
            let text = if let Some(t) = interp_text {
              // Clean up stacks if execute_node didn't run.
              t
            } else if let Some(value_id) = self.value_stack.pop() {
              self.ty_stack.pop();
              self.sir_values.pop();
              self.value_to_string(value_id)
            } else {
              String::new()
            };

            if !text.is_empty() {
              commands.push(UiCommand::Text(text));
            }
          }
        }
        _ => {
          idx += 1;
        }
      }
    }

    if !commands.is_empty() {
      let optimizer = TemplateOptimizer::new();

      commands = optimizer.optimize(commands);
    }

    // Prepend any pending stylesheets so they reach the
    // runtime alongside the template's UI commands.
    if !self.pending_styles.is_empty() {
      let mut styled = std::mem::take(&mut self.pending_styles);

      styled.append(&mut commands);
      commands = styled;
    }

    let template_id = self.values.store_template(self.template_counter);

    self.template_counter += 1;

    self.value_stack.push(template_id);
    self.ty_stack.push(self.ty_checker.template_ty());

    let sir_value = self.sir.emit(Insn::Template {
      id: template_id,
      name: None,
      ty_id: self.ty_checker.template_ty(),
      commands,
      bindings: std::mem::take(&mut self.template_bindings),
    });

    self.sir_values.push(sir_value);

    if let Some(var_name) = self.pending_var_name.take() {
      self.sir.emit(Insn::VarDef {
        name: var_name,
        ty_id: self.ty_checker.template_ty(),
        init: Some(template_id),
        mutability: Mutability::No,
        pubness: Pubness::No,
      });

      // Register in locals so later references
      // (e.g., `#dom view`) can find the variable.
      self.locals.push(Local {
        name: var_name,
        ty_id: self.ty_checker.template_ty(),
        value_id: template_id,
        pubness: Pubness::No,
        mutability: Mutability::No,
        sir_value: Some(sir_value),
        local_kind: LocalKind::Variable,
      });

      if let Some(frame) = self.scope_stack.last_mut() {
        frame.count += 1;
      }

      // Pop the template value from the stacks — it's now
      // stored in the local. Leaving it on the stack would
      // corrupt subsequent function call arg counts.
      self.value_stack.pop();
      self.ty_stack.pop();
      self.sir_values.pop();
    }
  }

  /// Tag registry: maps HTML tag names to UiCommand emissions.
  fn emit_opening_tag(
    &mut self,
    tag: &str,
    attrs: &[Attr],
    self_closing: bool,
    commands: &mut Vec<UiCommand>,
  ) {
    // Component resolution: if the tag name is a local template
    // variable, inline its commands directly (no wrapping
    // element). Short-circuit before any classification.
    if let Some(resolved) = self.try_resolve_template_component(tag) {
      commands.extend(resolved);
      return;
    }

    let element_tag = tag_to_element(tag);
    let mut out_attrs: Vec<Attr> = Vec::with_capacity(attrs.len() + 1);

    // Interactive elements get an auto-assigned widget id for
    // event routing; the id goes into the attrs vec as a
    // `data-id` Prop so the DOM bridge (bridge.js) can read it
    // via `e.target.dataset.id` and forward clicks/input
    // events back to the runtime.
    let widget_id: String = match &element_tag {
      ElementTag::Button | ElementTag::Input | ElementTag::Textarea => {
        let wid = self.next_widget_id();

        out_attrs.push(Attr::parse_prop("data-id", &wid.to_string()));

        wid.to_string()
      }
      ElementTag::Img => {
        let id = format!("img_{}", self.template_counter);

        out_attrs.push(Attr::str_prop("data-id", &id));
        id
      }
      _ => {
        let id = format!("{tag}_{}", self.template_counter);

        out_attrs.push(Attr::str_prop("data-id", &id));
        id
      }
    };

    // Copy through Prop/Style/Dynamic attributes. Events are
    // pulled into separate UiCommand::Event commands below.
    for attr in attrs {
      match attr {
        Attr::Prop { .. } | Attr::Style { .. } | Attr::Dynamic { .. } => {
          out_attrs.push(attr.clone());
        }
        Attr::Event { .. } => {}
      }
    }

    // Force self-closing for HTML5 void elements so the renderer
    // doesn't expect an EndElement for them.
    let final_self_closing =
      self_closing || element_tag.is_self_closing_default();

    // Record reactive attribute bindings against the element's
    // command index BEFORE pushing the command (so cmd_idx =
    // commands.len() points at the Element we're about to push).
    let element_cmd_idx = commands.len();

    for attr in &out_attrs {
      if matches!(attr, Attr::Dynamic { .. }) {
        self
          .template_bindings
          .attrs
          .push((element_cmd_idx, attr.clone()));
      }
    }

    commands.push(UiCommand::Element {
      tag: element_tag,
      attrs: out_attrs,
      self_closing: final_self_closing,
    });

    // Emit UiCommand::Event for each @event attribute, routed
    // to the widget id we just assigned.
    for attr in attrs {
      if let Attr::Event {
        event_kind,
        handler,
        ..
      } = attr
      {
        commands.push(UiCommand::Event {
          widget_id: widget_id.clone(),
          event_kind: event_kind.clone(),
          handler: handler.clone(),
        });
      }
    }
  }

  /// If `tag` names a local variable bound to a template value,
  /// return that template's commands for inlining. Otherwise
  /// returns None. Preserves the component-resolution behavior
  /// from the legacy `TagKind::Unknown` path.
  fn try_resolve_template_component(
    &self,
    tag: &str,
  ) -> Option<Vec<UiCommand>> {
    let sym = self.interner.symbol(tag)?;
    let local = self.locals.iter().rev().find(|l| l.name == sym)?;
    let vi = local.value_id.0 as usize;

    if vi >= self.values.kinds.len()
      || !matches!(self.values.kinds[vi], Value::Template)
    {
      return None;
    }

    let ti = self.values.indices[vi] as usize;
    let tpl_ref = self.values.templates[ti];

    for insn in &self.sir.instructions {
      if let Insn::Template {
        id,
        commands: child_cmds,
        ..
      } = insn
        && id.0 == tpl_ref
      {
        return Some(child_cmds.clone());
      }
    }

    None
  }

  /// Handle closing tag: emit `UiCommand::EndElement`. The
  /// legacy sentinel-rewriting hack is gone — the Element
  /// model's children are just inline `TextNode` / `Element`
  /// commands between the open and close markers.
  fn close_template_tag(&mut self, commands: &mut Vec<UiCommand>) {
    commands.push(UiCommand::EndElement);
  }

  /// Resolve a local variable to its stringified compile-time
  /// value for eager template embedding. Used by attribute
  /// shorthand (`<img {src} />`) and by attribute string
  /// interpolation (`alt="a picture of '{name}'"`). Returns an
  /// empty string if the symbol does not resolve to a local —
  /// matches the silently-empty semantics of the existing text
  /// interpolation path.
  fn resolve_local_for_template(&self, sym: Symbol) -> String {
    if let Some(local) = self.locals.iter().rev().find(|l| l.name == sym) {
      self.value_to_string(local.value_id)
    } else {
      String::new()
    }
  }

  /// Detect and handle a `{#html expr}` raw HTML splice inside
  /// a template interpolation. Returns `true` when the directive
  /// was recognized and consumed through its closing `}`, in
  /// which case the caller must NOT fall through to the regular
  /// `{expr}` interpolation path. Returns `false` when the
  /// leading tokens do not form a `#html` directive, leaving
  /// `idx` unchanged so the caller can try the normal path.
  ///
  /// Shape (MVP): `#` `Ident("html")` `Ident(src)` where `src`
  /// is an immutable local bound to a string value. The source
  /// string is resolved at compile time, parsed by
  /// `html_inline::parse_raw_html`, and spliced into `commands`
  /// at the interpolation site. Malformed directives emit a
  /// diagnostic and still return `true` so the walker advances
  /// past the closing brace.
  fn try_handle_html_directive(
    &mut self,
    idx: &mut usize,
    end_idx: usize,
    commands: &mut Vec<UiCommand>,
    brace_span: Span,
  ) -> bool {
    // Check the leading shape without consuming — if it doesn't
    // match, we return false and the caller uses the normal
    // interpolation path.
    if *idx + 1 >= end_idx
      || self.tree.nodes[*idx].token != Token::Hash
      || self.tree.nodes[*idx + 1].token != Token::Ident
    {
      return false;
    }

    let name_sym = match self.node_value(*idx + 1) {
      Some(NodeValue::Symbol(s)) => s,
      _ => return false,
    };

    if self.interner.get(name_sym) != "html" {
      return false;
    }

    // From here on, we've committed to the directive — any
    // error still returns `true` to advance past the closing
    // brace.
    *idx += 2; // past `#html`

    // Expect a single identifier naming the source string.
    let src_sym =
      if *idx < end_idx && self.tree.nodes[*idx].token == Token::Ident {
        match self.node_value(*idx) {
          Some(NodeValue::Symbol(s)) => {
            *idx += 1;
            Some(s)
          }
          _ => None,
        }
      } else {
        None
      };

    // Walk to the matching closing brace regardless of what's
    // inside — we don't want to leave the walker stranded.
    while *idx < end_idx && self.tree.nodes[*idx].token != Token::RBrace {
      *idx += 1;
    }

    if *idx < end_idx {
      *idx += 1; // past `}`
    }

    let Some(sym) = src_sym else {
      report_error(Error::new(ErrorKind::ExpectedExpression, brace_span));

      return true;
    };

    // Resolve the source — must be an immutable local bound to
    // a string.
    let Some(local) = self.locals.iter().rev().find(|l| l.name == sym) else {
      report_error(Error::new(ErrorKind::UndefinedVariable, brace_span));

      return true;
    };

    if local.mutability == Mutability::Yes {
      // MVP: dynamic #html is not yet supported. The error
      // kind is a close-enough semantic fit — a dedicated
      // variant can be added later.
      report_error(Error::new(ErrorKind::TypeMismatch, brace_span));

      return true;
    }

    let html_source = self.value_to_string(local.value_id);
    let html_commands = crate::html_inline::parse_raw_html(&html_source);

    commands.extend(html_commands);

    true
  }

  /// `true` when `sym` names a mutable local. Used by the
  /// attribute emitters to decide between `Attr::Prop` and
  /// `Attr::Dynamic`.
  fn local_is_mut(&self, sym: Symbol) -> bool {
    self
      .locals
      .iter()
      .rev()
      .find(|l| l.name == sym)
      .map(|l| l.mutability == Mutability::Yes)
      .unwrap_or(false)
  }

  /// Build an `Attr` for a single-identifier attribute source.
  /// Immutable locals produce `Attr::Prop` (eager only);
  /// mutable locals produce `Attr::Dynamic` carrying the
  /// reactive binding metadata alongside the initial value.
  fn make_attr_from_local(&self, name: &str, sym: Symbol) -> Attr {
    let value_str = self.resolve_local_for_template(sym);
    let initial = PropValue::parse(&value_str);

    if self.local_is_mut(sym) {
      Attr::Dynamic {
        name: name.to_string(),
        var: sym.0,
        initial,
      }
    } else {
      Attr::Prop {
        name: name.to_string(),
        value: initial,
      }
    }
  }

  /// Resolve a `Token::InterpString` template attribute value
  /// by walking its pre-parsed `InterpSegment` list. Each
  /// `Literal(sym)` segment contributes its raw text; each
  /// `Variable(sym)` segment contributes the stringified
  /// compile-time value of the matching local.
  fn resolve_interp_string_attr(&self, node_idx: usize) -> String {
    let packed = match self.node_value(node_idx) {
      Some(NodeValue::Literal(p)) => p,
      _ => return String::new(),
    };

    let interp_id = packed >> 16;
    let segments = self.literals.interp_segs(interp_id).to_vec();

    let mut out = String::new();

    for seg in segments {
      match seg {
        InterpSegment::Literal(sym) => {
          out.push_str(self.interner.get(sym));
        }
        InterpSegment::Variable(sym) => {
          out.push_str(&self.resolve_local_for_template(sym));
        }
      }
    }

    out
  }

  fn next_widget_id(&mut self) -> u32 {
    let id = self.widget_counter.get();

    self.widget_counter.set(id + 1);

    id
  }
}

/// The kind of control flow branch.
#[derive(Clone, Copy, PartialEq)]
enum BranchKind {
  /// An `if/else` branch.
  If,
  /// A `while` branch.
  While,
  /// A `for` branch.
  For,
  /// A `when ? :` ternary expression.
  Ternary,
  // Short-circuit logical `&&` / `||` intentionally does
  // NOT live on `branch_stack` — its state is on
  // `deferred_short_circuits`. Keeping this enum minimal
  // avoids dead match arms.
}

/// Tracks context for a pending control flow branch.
#[derive(Clone)]
struct BranchCtx {
  /// The kind of branch.
  kind: BranchKind,
  /// The label id for the end of the construct.
  end_label: u32,
  /// The label id for the else block (if only).
  else_label: Option<u32>,
  /// The label for loop start (while only).
  loop_label: Option<u32>,
  /// Whether the branch instruction has been emitted.
  branch_emitted: bool,
  /// For-loop variable name (For only).
  for_var: Option<Symbol>,
  /// Scope depth when this context was pushed. RBrace
  /// only closes control flow at the matching depth to
  /// prevent inner blocks (e.g. `_ => {}` in match arms)
  /// from accidentally consuming outer while/if contexts.
  scope_depth: usize,
  /// Synthetic local receiving each arm's result when
  /// the branch is in expression position. `None` for
  /// statement-position branches. emulates an SSA φ:
  /// each reachable arm `Store`s into this local; the
  /// merge point `Load`s the result and pushes it as
  /// the branch expression's value. see
  /// `PLAN_BRANCH_EXPR_PHI.md`.
  value_sink: Option<Symbol>,
  /// Unified type of all arm producers. set by the
  /// first arm to `Store` into the sink; subsequent
  /// arms unify against it.
  value_sink_ty: Option<TyId>,
  /// `sir_values.len()` captured when the branch was
  /// pushed. `emit_branch_sink_store` uses this to tell
  /// "this arm produced a new value on top of the stack"
  /// (depth grew) from "this arm was purely statements
  /// and the stack top is stale outer state from BEFORE
  /// the branch" (depth unchanged). Without this guard,
  /// a statement-position if inside a non-unit function
  /// (which `mint_branch_sink_if_expr_position`
  /// eagerly allocates a sink for) pops whatever sat on
  /// the stack — typically the enclosing while's
  /// condition bool — and silently corrupts the parent
  /// construct.
  stack_depth_at_entry: u32,
}

/// Tracks context when compiling inside a function
#[derive(Clone)]
struct FunCtx {
  pub(crate) name: Symbol,
  pub(crate) return_ty: TyId,
  pub(crate) body_start: u32,
  pub(crate) fundef_idx: usize,
  pub(crate) has_explicit_return: bool,
  /// True when the function declaration has `-> Type`.
  pub(crate) has_return_type_annotation: bool,
  /// Set when we see 'return' keyword, cleared when we emit Return insn.
  pub(crate) pending_return: bool,
  /// Scope depth when the function body was entered.
  /// Only close the function at this depth's RBrace.
  pub(crate) scope_depth: usize,
}

/// Snapshot of the outer fun's state when a nested `fun`
/// takes over. Restored at the nested fun's closing `}`
/// so the outer function body can keep running.
struct SavedOuterFun {
  function: Option<FunCtx>,
  value_stack: Vec<ValueId>,
  ty_stack: Vec<TyId>,
  sir_values: Vec<ValueId>,
  sir: Sir,
  pending_decl: Option<PendingDecl>,
}

/// Static tag registry — maps HTML tag names directly to
/// `ElementTag`. Unknown tags fall through to
/// `ElementTag::Custom` so the renderer can still stamp them
/// verbatim (and component resolution is attempted one layer up).
fn tag_to_element(tag: &str) -> ElementTag {
  match tag {
    "div" => ElementTag::Div,
    "section" => ElementTag::Section,
    "main" => ElementTag::Main,
    "article" => ElementTag::Article,
    "aside" => ElementTag::Aside,
    "header" => ElementTag::Header,
    "footer" => ElementTag::Footer,
    "nav" => ElementTag::Nav,
    "form" => ElementTag::Form,
    "ul" => ElementTag::Ul,
    "ol" => ElementTag::Ol,
    "li" => ElementTag::Li,
    "span" => ElementTag::Span,
    "h1" => ElementTag::H1,
    "h2" => ElementTag::H2,
    "h3" => ElementTag::H3,
    "p" => ElementTag::P,
    "img" => ElementTag::Img,
    "button" => ElementTag::Button,
    "input" => ElementTag::Input,
    "textarea" => ElementTag::Textarea,
    other => ElementTag::Custom(other.to_string()),
  }
}
