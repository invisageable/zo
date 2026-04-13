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
use std::collections::HashMap;

/// Scope frame for variable tracking
pub struct ScopeFrame {
  // Start index in locals array
  start: u32,
  // Number of locals in this scope
  count: u32,
}

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
  /// The type checker instance.
  ty_checker: TyChecker,
  /// Type annotations for HIR nodes
  annotations: Vec<Annotation>,
  /// Maps value_stack indices to SIR ValueIds for operands
  sir_values: Vec<ValueId>,
  /// Function definitions
  funs: Vec<FunDef>,
  /// Current function context (if we're inside a function)
  current_function: Option<FunCtx>,
  /// Pending function definition (waiting for LBrace)
  pending_function: Option<FunDef>,
  /// Counter for generating unique template IDs
  template_counter: u32,
  /// Pending variable name from imu/mut for template assignment
  pending_var_name: Option<Symbol>,
  /// Counter for unique widget IDs (buttons, inputs)
  widget_counter: Cell<u32>,
  /// The pending branch contexts for control flow.
  branch_stack: Vec<BranchCtx>,
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
  /// Global compile-time constants (`val` at module level).
  /// Visible from all functions.
  global_constants: Vec<Local>,
  /// Active type parameters: `$T → TyId`. Set during
  /// generic function definition, cleared after.
  type_params: Vec<(Symbol, TyId)>,
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
  /// Pending stylesheet commands collected from `$:` blocks.
  /// Injected into the next `Insn::Template`'s commands.
  pending_styles: Vec<UiCommand>,
  /// Reactive bindings collected during template execution,
  /// consumed when emitting `Insn::Template`. Split into text
  /// and attribute bindings so the runtime can dispatch each
  /// kind to the right patch path.
  template_bindings: zo_sir::TemplateBindings,
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
  ) -> Self {
    let capacity = tree.nodes.len();

    Self {
      tree,
      interner,
      literals,
      value_stack: Vec::with_capacity(capacity / 4), // Estimate stack depth
      ty_stack: Vec::with_capacity(capacity / 4),
      values: ValueStorage::new(capacity),
      scope_stack: Vec::with_capacity(32), // Typical nesting depth
      locals: Vec::with_capacity(capacity / 10), // Estimate variables
      sir: Sir::new(),
      ty_checker: TyChecker::new(),
      annotations: Vec::with_capacity(capacity),
      sir_values: Vec::with_capacity(capacity / 4),
      funs: Vec::with_capacity(capacity / 100), // Estimate function count
      current_function: None,
      pending_function: None,
      template_counter: 0,
      pending_var_name: None,
      widget_counter: Cell::new(0),
      branch_stack: Vec::with_capacity(8),
      skip_until: 0,
      pending_decl: None,
      pending_assign: None,
      pending_compound: None,
      pending_compound_receiver: None,
      array_ctx: Vec::new(),
      pending_array_assign: None,
      tuple_ctx: Vec::new(),
      deferred_binops: Vec::new(),
      closure_counter: 0,
      enum_defs: Vec::new(),
      pending_imported_enums: Vec::new(),
      var_return_type_args: HashMap::default(),
      pending_enum_construct: None,
      apply_context: None,
      global_constants: Vec::new(),
      type_params: Vec::new(),
      deferred_closures: Vec::new(),
      pending_call_rparen: None,
      pending_styles: Vec::new(),
      template_bindings: TemplateBindings::default(),
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
  ) -> Self {
    self.funs = funs;
    self.locals.extend(vars);

    // Defer enum interning to first use to avoid TyId counter
    // shifts that would pollute HM unification. Raw export
    // data is stored here and resolved lazily in
    // `execute_enum_access` / the Ident handler.
    self.pending_imported_enums = enums;

    self
  }

  /// Executes a parse tree in one pass to build semantic IR.
  pub fn execute(mut self) -> (Sir, Vec<Annotation>, TyChecker, Vec<FunDef>) {
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

    // Monomorphization: duplicate generic function bodies
    // for each instantiation.
    self.monomorphize();

    (self.sir, self.annotations, self.ty_checker, self.funs)
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

      Token::Ext => {
        let children_end = (header.child_start + header.child_count) as usize;

        self.execute_ext(idx, children_end);
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

        self.execute_pack(idx, children_end);

        self.skip_until = children_end;
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

        self.branch_stack.push(BranchCtx {
          kind: BranchKind::Ternary,
          end_label,
          else_label: Some(else_label),
          // Store stack depth at When for deferred
          // branch detection.
          loop_label: Some(self.sir_values.len() as u32),
          branch_emitted: false,
          for_var: None,
        });
      }

      Token::Question => {
        // Condition is now on the stack — emit branch.
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
        if let Some(ctx) = self.branch_stack.last_mut()
          && ctx.kind == BranchKind::If
        {
          // Jump over else block (from if-body).
          self.sir.emit(Insn::Jump {
            target: ctx.end_label,
          });

          // Emit else label.
          if let Some(else_label) = ctx.else_label.take() {
            self.sir.emit(Insn::Label { id: else_label });
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
        let is_call = self.resolve_call_target(idx).is_some();

        if is_call {
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
        // Check if this closes an enum variant constructor.
        if let Some((enum_name, disc, field_count, ty_id)) =
          self.pending_enum_construct.take()
        {
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
          self.apply_deferred_binop();
        } else {
          // No tuple context → function call.
          self.execute_potential_call(idx);
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
            pending_return: false,
            scope_depth: self.scope_stack.len(),
          });

          // Update body_start in the pending function
          pending_func.body_start = body_start;

          // Store function definition for later calls
          self.funs.push(pending_func);

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
              if has_value && body_ty != unit_ty {
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
        }

        // Close control flow block.
        if let Some(ctx) = self.branch_stack.last() {
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
                // No else — emit the label that
                // BranchIfNot targets (else_label),
                // plus end_label, then pop.
                if let Some(el) = ctx.else_label {
                  self.sir.emit(Insn::Label { id: el });
                }

                self.sir.emit(Insn::Label { id: ctx.end_label });
                self.branch_stack.pop();
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

            if is_dot_member {
              let placeholder = self.values.store_runtime(0);

              self.value_stack.push(placeholder);
              self.ty_stack.push(self.ty_checker.unit_type());
              self.sir_values.push(ValueId(u32::MAX));
            } else if !is_fun && !is_enum && !is_struct {
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
        // the preceding Ident. For literals: stacks
        // have whatever was there before.
        let is_indexing = idx > 0
          && matches!(
            self.tree.nodes[idx - 1].token,
            Token::Ident | Token::RBracket
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

            // Infer element type from the first element
            // before popping the stacks.
            let elem_ty = if depth < self.ty_stack.len() {
              self.ty_stack[depth]
            } else {
              int_ty
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
      Token::AmpAmp => self.execute_binop(BinOp::And, idx),
      Token::PipePipe => self.execute_binop(BinOp::Or, idx),

      // === BITWISE OPERATORS ===
      Token::Amp => self.execute_binop(BinOp::BitAnd, idx),
      Token::Pipe => self.execute_binop(BinOp::BitOr, idx),
      Token::Caret => self.execute_binop(BinOp::BitXor, idx),
      Token::LShift => self.execute_binop(BinOp::Shl, idx),
      Token::RShift => self.execute_binop(BinOp::Shr, idx),

      // === UNARY OPERATORS ===
      Token::Bang => self.execute_unop(UnOp::Not, idx),

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
          let ctx = self.branch_stack.last().unwrap();
          let end_label = ctx.end_label;
          let else_label = ctx.else_label.unwrap();

          // Emit Return for the true arm — handles both
          // explicit `return when ...` and implicit return
          // (when as last expression in non-void function).
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

              fun_ctx.has_explicit_return = true;
            }
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
          let ctx = self.branch_stack.pop().unwrap();
          self.sir.emit(Insn::Label { id: ctx.end_label });
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
    }
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
  fn execute_pack(&mut self, _start_idx: usize, end_idx: usize) {
    let mut name = None;

    for child_idx in (_start_idx + 1)..end_idx {
      if let Some(node) = self.tree.nodes.get(child_idx)
        && node.token == Token::Ident
        && let Some(NodeValue::Symbol(sym)) = self.node_value(child_idx)
      {
        name = Some(sym);
        break;
      }
    }

    if let Some(name) = name {
      self.sir.emit(Insn::PackDecl {
        name,
        pubness: if self.is_pub(_start_idx) {
          Pubness::Yes
        } else {
          Pubness::No
        },
      });
    }
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

      if tok.is_ty() {
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

    // Mangle name in apply context: Type::method.
    let name = if let Some(type_name) = self.apply_context {
      let type_str = self.interner.get(type_name).to_owned();
      let method_str = self.interner.get(name.unwrap()).to_owned();
      let mangled = format!("{type_str}::{method_str}");

      self.interner.intern(&mangled)
    } else {
      name.unwrap()
    };

    // Parse parameters: (name, type, mutability).
    let mut params: Vec<(Symbol, TyId, Mutability)> = Vec::new();
    let mut return_ty = self.ty_checker.unit_type();
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
          }
        }

        idx += 1;
      }
    }

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
            // the applied type.
            if let Some(type_name) = self.apply_context {
              let self_sym = zo_interner::Symbol::SELF_LOWER;
              let self_ty = self
                .ty_checker
                .resolve_ty_name(type_name)
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

    // main() must return unit — no other return type.
    let unit_ty = self.ty_checker.unit_type();

    if self.interner.get(name) == "main" && return_ty != unit_ty {
      // Point the span at the return type token (after ->).
      let span = self
        .find_return_type_span(start_idx, _end_idx)
        .unwrap_or(self.tree.spans[start_idx]);

      report_error(Error::new(ErrorKind::InvalidReturnType, span));

      return_ty = unit_ty;
    }

    // FunDef stores (name, ty) — strip mutability.
    let sir_params =
      params.iter().map(|(n, t, _)| (*n, *t)).collect::<Vec<_>>();

    self.pending_function = Some(FunDef {
      name,
      params: sir_params,
      return_ty,
      body_start: 0,
      kind: FunctionKind::UserDefined,
      pubness,
      type_params: self.type_params.iter().map(|(_, ty)| *ty).collect(),
      return_type_args: Vec::new(),
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

  /// Called at Semicolon after the init expression has been
  /// evaluated and its value is on the stacks.
  fn finalize_pending_decl(&mut self) {
    let decl = match self.pending_decl.take() {
      Some(d) => d,
      None => return,
    };

    if let (Some(init_value), Some(init_ty)) =
      (self.value_stack.pop(), self.ty_stack.pop())
    {
      let sir_init = self.sir_values.pop();

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

  /// Executes an `ext` declaration — an intrinsic function
  /// with no body. Emits `FunDef { is_intrinsic: true }`.
  fn execute_ext(&mut self, start_idx: usize, end_idx: usize) {
    // Parse signature: ext name(params) -> return_ty;
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
              // Skip the default value expression.
              if idx < end_idx {
                idx += 1;
              }
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
      fields,
      pubness,
    });

    self.skip_until = end_idx;
  }

  /// Executes `apply Type { fun_defs... }`.
  ///
  /// Sets the apply context so child function definitions
  /// get mangled names (`Type::method`). `Self` resolves
  /// to the applied type.
  fn execute_apply(&mut self, start_idx: usize, end_idx: usize) {
    // Parse type name.
    let type_name = self
      .tree
      .nodes
      .get(start_idx + 1)
      .filter(|n| n.token == Token::Ident)
      .and_then(|_| self.node_value(start_idx + 1))
      .and_then(|v| match v {
        NodeValue::Symbol(s) => Some(s),
        _ => None,
      });

    let type_name = match type_name {
      Some(n) => n,
      None => {
        self.skip_until = end_idx;
        return;
      }
    };

    // Set apply context.
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

    // Array builtin methods: push.
    let resolved = self.ty_checker.kind_of(receiver_ty);

    if matches!(resolved, Ty::Array(_)) {
      let ms = self.interner.get(member_name);

      return ms == "push";
    }

    // Resolve receiver type name for struct/enum methods.
    let type_name = match resolved {
      Ty::Struct(sid) => {
        self.ty_checker.ty_table.struct_ty(sid).map(|s| s.name)
      }
      Ty::Enum(eid) => self.ty_checker.ty_table.enum_ty(eid).map(|e| e.name),
      _ => None,
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

    let type_name = match resolved {
      Ty::Struct(sid) => {
        self.ty_checker.ty_table.struct_ty(sid).map(|s| s.name)
      }
      Ty::Enum(eid) => self.ty_checker.ty_table.enum_ty(eid).map(|e| e.name),
      _ => None,
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

  /// Executes a dot-call `receiver.method(args)`.
  /// The receiver is already on the stack (left by the
  /// Dot handler). Injects it as the first argument.
  fn execute_dot_method_call(
    &mut self,
    mangled_name: Symbol,
    lparen_idx: usize,
    rparen_idx: usize,
  ) {
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

  fn execute_if(&mut self, _start_idx: usize, _end_idx: usize) {
    let end_label = self.sir.next_label();
    let else_label = self.sir.next_label();

    self.branch_stack.push(BranchCtx {
      kind: BranchKind::If,
      end_label,
      else_label: Some(else_label),
      loop_label: None,
      branch_emitted: false,
      for_var: None,
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
    // Everything between `start_idx` and `end_idx` is ours;
    // the main loop must not re-visit these nodes after we
    // return.
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

    // Extract the scrutinee's local symbol for per-arm reloads.
    // For Slice 1 the scrutinee is always a single Ident node.
    let scrutinee_sym = (start_idx + 1..lbrace_idx)
      .find(|&i| self.tree.nodes[i].token == Token::Ident)
      .and_then(|i| match self.node_value(i) {
        Some(NodeValue::Symbol(s)) => Some(s),
        _ => None,
      });

    while self.sir_values.len() > stack_before {
      self.sir_values.pop();
      self.value_stack.pop();
      self.ty_stack.pop();
    }

    // -- 4. Walk the arms ------------------------------------
    let end_label = self.sir.next_label();
    let mut arm_idx = lbrace_idx + 1;

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

      if !is_wildcard && pat_tok == Token::Int {
        let value = match self.node_value(pat_idx) {
          Some(NodeValue::Literal(lit)) => {
            self.literals.int_literals[lit as usize]
          }
          _ => 0,
        };

        // Fresh Load per arm so the register allocator
        // sees an independent liveness range for the
        // scrutinee in each test.
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

        let pat_sir = self.sir.emit(Insn::ConstInt {
          dst: pat_dst,
          value,
          ty_id: scrutinee_ty,
        });

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
          let rta_key = scrutinee_sym
            .and_then(|s| self.var_return_type_args.get(&s.as_u32()));

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
            self.ty_checker.ty_table.variant_fields(&variant).to_vec()
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
      }

      // Execute arm body nodes. Each call to `execute_node`
      // emits SIR for one tree node; expression values pile
      // up on the sir_values stack but since Slice 1 bodies
      // are statement expressions (e.g. `showln(...)`), the
      // stack returns to its baseline once the call
      // completes.
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

      // Drop any residual expression values left on the
      // stack by the arm body. Unit-returning calls leave
      // nothing, but we defensively normalize.
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
    });
  }

  /// Desugars `for i := start..end { body }` into
  /// while-loop SIR:
  ///   mut i = start;
  ///   while i < end { body; i = i + 1; }
  fn execute_for(&mut self, start_idx: usize, end_idx: usize) {
    // Tree: For → [Ident(i), ColonEq, start, DotDot, end,
    //              LBrace, ...body..., RBrace]
    // Scan children for the variable name, start, and end.
    let mut var_name = None;
    let mut range_start = None;
    let mut range_end = None;
    let mut i = start_idx + 1;

    while i < end_idx {
      match self.tree.nodes[i].token {
        Token::Ident if var_name.is_none() => {
          if let Some(NodeValue::Symbol(sym)) = self.node_value(i) {
            var_name = Some(sym);
          }
        }
        Token::Int => {
          if let Some(NodeValue::Literal(lit)) = self.node_value(i) {
            let val = self.literals.int_literals[lit as usize];
            if range_start.is_none() {
              range_start = Some(val);
            } else {
              range_end = Some(val);
            }
          }
        }
        Token::LBrace => break,
        _ => {}
      }

      i += 1;
    }

    let var_name = match var_name {
      Some(n) => n,
      None => return,
    };

    let start_val = range_start.unwrap_or(0);
    let end_val = range_end.unwrap_or(0);
    let int_ty = self.ty_checker.int_type();

    // --- Emit: mut i = start ---
    let init_dst = ValueId(self.sir.next_value_id);
    self.sir.next_value_id += 1;

    let init_sir = self.sir.emit(Insn::ConstInt {
      dst: init_dst,
      value: start_val,
      ty_id: int_ty,
    });

    let init_vid = self.values.store_int(start_val);

    self.sir.emit(Insn::VarDef {
      name: var_name,
      ty_id: int_ty,
      init: Some(init_sir),
      mutability: Mutability::Yes,
      pubness: Pubness::No,
    });

    self.locals.push(Local {
      name: var_name,
      ty_id: int_ty,
      value_id: init_vid,
      pubness: Pubness::No,
      mutability: Mutability::Yes,
      sir_value: Some(init_sir),
      local_kind: LocalKind::Variable,
    });

    if let Some(frame) = self.scope_stack.last_mut() {
      frame.count += 1;
    }

    // Emit initial Store (mutable lives on stack).
    self.sir.emit(Insn::Store {
      name: var_name,
      value: init_sir,
      ty_id: int_ty,
    });

    // --- Emit: loop header ---
    let loop_label = self.sir.next_label();
    let end_label = self.sir.next_label();

    self.sir.emit(Insn::Label { id: loop_label });

    // Condition: Load i < end
    let cond_dst = ValueId(self.sir.next_value_id);

    self.sir.next_value_id += 1;

    let load_sir = self.sir.emit(Insn::Load {
      dst: cond_dst,
      src: LoadSource::Local(var_name),
      ty_id: int_ty,
    });

    let end_dst = ValueId(self.sir.next_value_id);
    self.sir.next_value_id += 1;

    let end_sir = self.sir.emit(Insn::ConstInt {
      dst: end_dst,
      value: end_val,
      ty_id: int_ty,
    });

    let cmp_dst = ValueId(self.sir.next_value_id);

    self.sir.next_value_id += 1;

    let cmp_sir = self.sir.emit(Insn::BinOp {
      dst: cmp_dst,
      op: zo_sir::BinOp::Lt,
      lhs: load_sir,
      rhs: end_sir,
      ty_id: int_ty,
    });

    self.sir.emit(Insn::BranchIfNot {
      cond: cmp_sir,
      target: end_label,
    });

    // Push branch context — RBrace will emit increment
    // + jump.
    self.branch_stack.push(BranchCtx {
      kind: BranchKind::For,
      end_label,
      else_label: None,
      loop_label: Some(loop_label),
      branch_emitted: true,
      for_var: Some(var_name),
    });

    // Skip header tokens (Ident, ColonEq, start, DotDot,
    // end) — let the main loop process from LBrace onward.
    let lbrace_idx = (start_idx + 1..end_idx)
      .find(|&j| self.tree.nodes[j].token == Token::LBrace)
      .unwrap_or(end_idx);

    self.skip_until = lbrace_idx;
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

      // Clear the pending flag
      ctx.pending_return = false;
    }
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
                // Dot-call: receiver.method(args).
                let mangled = self.resolve_dot_call(fun_idx, fun_name);

                if mangled != fun_name {
                  // Inject receiver as first arg.
                  // The receiver is on the stack
                  // (left by the Dot handler).
                  // execute_call will count explicit
                  // args between parens. We need to
                  // include the receiver in arg_sirs.
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

            if ms == "push"
              && let Some(recv_ty) = self
                .ty_stack
                .get(self.ty_stack.len().saturating_sub(2))
                .copied()
              && matches!(self.ty_checker.kind_of(recv_ty), Ty::Array(_))
            {
              self.execute_array_push(lparen_idx, rparen_idx);

              return;
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

      if !func.type_params.is_empty() {
        // Build substitution: old var → fresh var.
        let subs: Vec<(TyId, TyId)> = func
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

        for tp in &func.type_params {
          let resolved = self.ty_checker.resolve_id(*tp);
          let ty = self.ty_checker.resolve_ty(resolved);
          let ty_name = match ty {
            Ty::Int { .. } => "int",
            Ty::Float(_) => "float",
            Ty::Bool => "bool",
            Ty::Str => "str",
            Ty::Char => "char",
            _ => "unknown",
          };

          mangled = format!("{mangled}__{ty_name}");
        }

        let sym = self.interner.intern(&mangled);

        // Record instantiation for the mono pass.
        if !self.funs.iter().any(|f| f.name == sym) {
          let mut mono_def = func.clone();

          mono_def.name = sym;
          mono_def.type_params = Vec::new();

          self.funs.push(mono_def);
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
          }

          // Record SIR substitutions for the monomorphize
          // pass: replace Call { name: param } with
          // Call { name: closure_fn } in the duplicated body.
          // Store in a field for monomorphize() to use.
          // For now, do inline SIR patching after body dup.

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
      if !func.return_type_args.is_empty()
        && let Some(ref decl) = self.pending_decl
      {
        self
          .var_return_type_args
          .insert(decl.name.as_u32(), func.return_type_args.clone());
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
      "dom" if !self.value_stack.is_empty() => {
        let template_value = self.value_stack.pop().unwrap();
        let template_ty = self.ty_stack.pop().unwrap();

        self.sir.emit(Insn::Directive {
          name: sym,
          value: template_value,
          ty_id: template_ty,
        });
      }
      "inline" => {}
      _ => {}
    }
  }

  /// Duplicates generic function SIR bodies for each
  /// monomorphized instantiation.
  ///
  /// Scans `self.funs` for entries whose `body_start`
  /// matches an existing generic function but whose name
  /// is a mangled variant. For each, copies the SIR
  /// instructions from `FunDef..Return` and appends them
  /// with the mangled name.
  fn monomorphize(&mut self) {
    // Find generic function SIR ranges: (name, start_idx, end_idx).
    let mut generic_ranges: Vec<(Symbol, usize, usize)> = Vec::new();

    for (i, insn) in self.sir.instructions.iter().enumerate() {
      if let Insn::FunDef { name, .. } = insn {
        // Check if this function is generic by looking
        // up its FunDef in self.funs.
        let is_generic = self
          .funs
          .iter()
          .any(|f| f.name == *name && !f.type_params.is_empty());

        if is_generic {
          // Find the matching Return to get the range.
          let end = (i + 1..self.sir.instructions.len())
            .find(|&j| matches!(self.sir.instructions[j], Insn::Return { .. }))
            .unwrap_or(self.sir.instructions.len() - 1);

          generic_ranges.push((*name, i, end));
        }
      }
    }

    // For each mangled instantiation, duplicate the body.
    let mono_funs: Vec<(Symbol, Symbol)> = self
      .funs
      .iter()
      .filter(|f| f.type_params.is_empty())
      .filter_map(|f| {
        // Find which generic this is an instance of.
        let name_str = self.interner.get(f.name);

        for (gen_name, _, _) in &generic_ranges {
          let gen_str = self.interner.get(*gen_name);

          if name_str.starts_with(gen_str)
            && name_str.len() > gen_str.len()
            && name_str.as_bytes()[gen_str.len()..].starts_with(b"__")
          {
            return Some((*gen_name, f.name));
          }
        }

        None
      })
      .collect();

    for (gen_name, mono_name) in &mono_funs {
      let range = generic_ranges.iter().find(|(n, _, _)| n == gen_name);

      let Some((_, start, end)) = range else {
        continue;
      };

      // Clone the SIR range.
      let mut cloned: Vec<Insn> = self.sir.instructions[*start..=*end].to_vec();

      // Replace the FunDef name with the mangled name.
      if let Some(Insn::FunDef { name, .. }) = cloned.first_mut() {
        *name = *mono_name;
      }

      // Insert BEFORE the last function (main) so DCE
      // treats main as the entry point, not the mono'd fn.
      let main_idx = self
        .sir
        .instructions
        .iter()
        .rposition(|i| matches!(i, Insn::FunDef { .. }))
        .unwrap_or(self.sir.instructions.len());

      // Splice the cloned instructions before main.
      let pos = main_idx;

      for (j, insn) in cloned.into_iter().enumerate() {
        self.sir.instructions.insert(pos + j, insn);
      }
    }

    // Closure-param monomorphization: duplicate function
    // bodies for `__cl` mangled names, replacing internal
    // `Call { name: param }` with the concrete closure name.
    // Collect closure-param monomorphization candidates.
    let cl_indices: Vec<usize> = self
      .funs
      .iter()
      .enumerate()
      .filter(|(_, f)| self.interner.get(f.name).contains("__cl"))
      .map(|(i, _)| i)
      .collect();

    #[allow(clippy::type_complexity)]
    let mut cl_funs: Vec<(Symbol, Symbol, Vec<(Symbol, Symbol)>)> = Vec::new();

    for idx in cl_indices {
      let name_str = self.interner.get(self.funs[idx].name).to_owned();
      let base_end = name_str.find("__cl").unwrap_or(name_str.len());
      let base_str = &name_str[..base_end];
      let base_sym = self.interner.intern(base_str);
      let mono_name = self.funs[idx].name;
      let mono_params = self.funs[idx].params.clone();

      let mut subs: Vec<(Symbol, Symbol)> = Vec::new();

      if let Some(orig) = self.funs.iter().find(|g| g.name == base_sym) {
        for (op, mp) in orig.params.iter().zip(mono_params.iter()) {
          if op.0 != mp.0 {
            subs.push((op.0, mp.0));
          }
        }
      }

      cl_funs.push((base_sym, mono_name, subs));
    }

    for (base_sym, mono_name, subs) in &cl_funs {
      // Find base function's SIR range.
      let range = self
        .sir
        .instructions
        .iter()
        .enumerate()
        .find(|(_, insn)| {
          matches!(insn, Insn::FunDef { name, .. } if *name == *base_sym)
        })
        .map(|(i, _)| {
          let end = (i + 1..self.sir.instructions.len())
            .find(|&j| {
              matches!(
                self.sir.instructions[j],
                Insn::Return { .. }
              )
            })
            .unwrap_or(self.sir.instructions.len() - 1);

          (i, end)
        });

      let Some((start, end)) = range else {
        continue;
      };

      let mut cloned: Vec<Insn> = self.sir.instructions[start..=end].to_vec();

      // Replace FunDef name.
      if let Some(Insn::FunDef { name, .. }) = cloned.first_mut() {
        *name = *mono_name;
      }

      // Replace Call names: param → closure.
      for insn in cloned.iter_mut() {
        if let Insn::Call { name, .. } = insn {
          for (from, to) in subs {
            if *name == *from {
              *name = *to;
            }
          }
        }
      }

      // Insert before main.
      let main_idx = self
        .sir
        .instructions
        .iter()
        .rposition(|i| matches!(i, Insn::FunDef { .. }))
        .unwrap_or(self.sir.instructions.len());

      for (j, insn) in cloned.into_iter().enumerate() {
        self.sir.instructions.insert(main_idx + j, insn);
      }
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
          tag, self_closing, ..
        } => {
          let name = tag.as_str();

          if *self_closing {
            out.push_str(&format!("<{name} />"));
          } else {
            out.push_str(&format!("<{name}>"));
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
}

/// Tracks context when compiling inside a function
#[derive(Clone)]
struct FunCtx {
  pub(crate) name: Symbol,
  pub(crate) return_ty: TyId,
  pub(crate) body_start: u32,
  pub(crate) fundef_idx: usize,
  pub(crate) has_explicit_return: bool,
  /// Set when we see 'return' keyword, cleared when we emit Return insn.
  pub(crate) pending_return: bool,
  /// Scope depth when the function body was entered.
  /// Only close the function at this depth's RBrace.
  pub(crate) scope_depth: usize,
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
