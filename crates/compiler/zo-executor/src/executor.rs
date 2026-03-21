use zo_constant_folding::{ConstFold, FoldResult, Operand};
use zo_error::{Error, ErrorKind};
use zo_interner::{Interner, Symbol};
use zo_reporter::report_error;
use zo_sir::{BinOp, Insn, Sir, UnOp};
use zo_span::Span;
use zo_template_optimizer::TemplateOptimizer;
use zo_token::{LiteralStore, Token};
use zo_tree::{NodeHeader, NodeValue, Tree};
use zo_ty::{Annotation, TyId};
use zo_ty_checker::TyChecker;
use zo_ui_protocol::{
  Attr, ContainerDirection, EventKind, PropValue, TextStyle, UiCommand,
};
use zo_value::{
  FunDef, Local, Mutability, Pubness, Value, ValueId, ValueStorage,
};

use std::cell::Cell;

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
  /// String interner (mostly read-only - symbols already interned during
  /// parsing)
  interner: &'a Interner,
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
  pending_assign: Option<Symbol>,
}

/// Deferred variable declaration, finalized at Semicolon.
struct PendingDecl {
  name: Symbol,
  is_mutable: bool,
  is_pub: bool,
}
impl<'a> Executor<'a> {
  /// Creates a new [`Executor`] instance.
  pub fn new(
    tree: &'a Tree,
    interner: &'a Interner,
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
  pub fn with_imports(mut self, funs: Vec<FunDef>, vars: Vec<Local>) -> Self {
    self.funs = funs;
    self.locals.extend(vars);

    self
  }

  /// Executes a parse tree in one pass to build semantic IR.
  pub fn execute(mut self) -> (Sir, Vec<Annotation>, TyChecker) {
    for idx in 0..self.tree.nodes.len() {
      if idx < self.skip_until {
        continue;
      }
      let header = self.tree.nodes[idx];
      self.execute_node(&header, idx);
    }

    (self.sir, self.annotations, self.ty_checker)
  }

  /// Executes a single node from the parse tree.
  /// This is the core of the execution-based compilation model
  fn execute_node(&mut self, header: &NodeHeader, idx: usize) {
    match header.token {
      Token::Fun => {
        let children_end = (header.child_start + header.child_count) as usize;

        self.execute_fun(idx, children_end);
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
      Token::Imu | Token::Val => {
        self.begin_decl(idx, header, false);
      }

      Token::Mut => {
        self.begin_decl(idx, header, true);
      }

      // === CONTROL FLOW ===
      Token::If => {
        let children_end = (header.child_start + header.child_count) as usize;

        self.execute_if(idx, children_end);
      }

      Token::While => {
        let children_end = (header.child_start + header.child_count) as usize;

        self.execute_while(idx, children_end);
      }

      Token::For => {
        let children_end = (header.child_start + header.child_count) as usize;

        self.execute_for(idx, children_end);
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

      // === DIRECTIVES ===
      Token::Hash => {
        let children_end = (header.child_start + header.child_count) as usize;

        self.execute_directive(idx, children_end);
      }

      // === FUNCTION CALLS ===
      Token::RParen => {
        // Check if this is a function call by looking back
        // Function calls have the pattern: Ident, LParen, [args...], RParen
        self.execute_potential_call(idx);
      }

      // === SCOPE BOUNDARIES ===
      Token::LBrace => {
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
            is_intrinsic: false,
            is_pub: pending_func.is_pub,
          });

          // Now set the context with the correct body start.
          // scope_depth tracks where we are so only the
          // function body's RBrace triggers function-close.
          self.current_function = Some(FunCtx {
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

        // Emit branch instruction for control flow.
        if let Some(ctx) = self.branch_stack.last_mut()
          && !ctx.branch_emitted
        {
          if let Some(cond_sir) = self.sir_values.last().copied() {
            let target = match ctx.kind {
              BranchKind::If => ctx.else_label.unwrap_or(ctx.end_label),
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

            let (return_value, return_ty) = if func_return_ty == unit_ty {
              // Void function with implicit non-unit return
              // → user likely forgot `-> T` annotation.
              if has_value && body_ty != unit_ty {
                let span = self.tree.spans[idx];

                report_error(Error::new(ErrorKind::TypeMismatch, span));
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
              // Non-void function but no value — type error.
              let span = self.tree.spans[idx];

              report_error(Error::new(ErrorKind::TypeMismatch, span));

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
            if let Some(Insn::FunDef { is_intrinsic, .. }) =
              self.sir.instructions.get_mut(fun_ctx.fundef_idx)
            {
              *is_intrinsic = true;
            }
          }

          // Clear function context
          self.current_function = None;
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
                let load_src = 100 + var_name.as_u32();
                let ld = ValueId(self.sir.next_value_id);

                self.sir.next_value_id += 1;

                let ld_sir = self.sir.emit(Insn::Load {
                  dst: ld,
                  src: load_src,
                  ty_id: int_ty,
                });

                let one_sir = self.sir.emit(Insn::ConstInt {
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
          }
        }

        self.pop_scope();
      }

      // === LITERALS (push compile-time constants) ===
      Token::Int => {
        // Get the integer value from the node
        if let Some(NodeValue::Literal(lit_idx)) = self.node_value(idx) {
          // Get actual value from literal store (already u64, no cast needed)
          let value = self.literals.int_literals[lit_idx as usize];

          // Infer type based on value
          let ty_id = self.ty_checker.int_type();

          let sir_value = self.sir.emit(Insn::ConstInt { value, ty_id });
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

          let sir_value = self.sir.emit(Insn::ConstFloat { value, ty_id });
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
        let sir_value = self.sir.emit(Insn::ConstBool { value: true, ty_id });
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
        let sir_value = self.sir.emit(Insn::ConstBool {
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

      Token::String | Token::RawString => {
        // String literals are already interned during tokenization
        if let Some(NodeValue::Symbol(symbol)) = self.node_value(idx) {
          let ty_id = self.ty_checker.str_type();

          // Emit SIR instruction for string constant
          let sir_value = self.sir.emit(Insn::ConstString { symbol, ty_id });

          // Store in value storage and push to stack
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

      // === IDENTIFIERS ===
      Token::Ident => {
        if let Some(NodeValue::Symbol(sym)) = self.node_value(idx) {
          // Copy fields to avoid borrow issues.
          let local_info = self.lookup_local(sym).map(|l| {
            (l.value_id, l.ty_id, l.sir_value, l.is_param, l.mutability)
          });

          if let Some((value_id, ty_id, sir_value, is_param, mutability)) =
            local_info
          {
            if self.current_function.is_some() {
              let is_mut = mutability == Mutability::Yes;

              if is_param || is_mut {
                // Parameter or mutable local: emit Load.
                // Params use src=param_index (0-7).
                // Mutables use src=100+slot so codegen
                // can distinguish and read from stack.
                let dst = ValueId(self.sir.next_value_id);

                self.sir.next_value_id += 1;

                let src = if is_param {
                  value_id.0
                } else {
                  100 + sym.as_u32()
                };

                let sv = self.sir.emit(Insn::Load { dst, src, ty_id });

                let rid = self.values.store_runtime(0);

                self.value_stack.push(rid);
                self.ty_stack.push(ty_id);
                self.sir_values.push(sv);
              } else if let Some(sv) = sir_value {
                // Immutable local: reuse SIR ValueId.
                let rid = self.values.store_runtime(0);

                self.value_stack.push(rid);
                self.ty_stack.push(ty_id);
                self.sir_values.push(sv);
              }
            } else {
              self.value_stack.push(value_id);
              self.ty_stack.push(ty_id);
              self.sir_values.push(value_id);
            }
          } else {
            // Check if this identifier is a known function
            // — call handling happens at RParen, not here.
            let name = self.interner.get(sym);
            let is_builtin =
              matches!(name, "show" | "showln" | "eshow" | "eshowln" | "flush");
            let is_fun = is_builtin || self.funs.iter().any(|f| f.name == sym);

            if !is_fun {
              let span = self.tree.spans[idx];

              report_error(Error::new(ErrorKind::UndefinedVariable, span));
            }

            // Push error values for stack consistency.
            let error_id = self.values.store_runtime(u32::MAX);

            self.value_stack.push(error_id);
            self.ty_stack.push(self.ty_checker.error_type());
          }
        }
      }

      // === TYPE LITERALS ===
      _ if header.token.is_ty() => {
        let ty_id = self.resolve_type_token(idx);
        let value_id = self.values.store_type(ty_id);

        self.value_stack.push(value_id);
        self.ty_stack.push(self.ty_checker.type_type());
      }

      // === BINARY OPERATORS ===
      Token::Plus => self.execute_binop(BinOp::Add, idx),
      Token::Minus => {
        if self.value_stack.len() >= 2 {
          self.execute_binop(BinOp::Sub, idx);
        } else {
          self.execute_unop(UnOp::Neg, idx);
        }
      }
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

      // === TYPE ANNOTATION ===
      Token::Colon => self.execute_ty_annotation(),

      // === TEMPLATE TOKENS ===
      Token::TemplateAssign => {
        let children_end = (header.child_start + header.child_count) as usize;
        self.execute_template_assign(idx, children_end);
      }

      Token::TemplateFragmentStart => {
        let children_end = (header.child_start + header.child_count) as usize;
        self.execute_template_fragment(idx, children_end);
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
              src: 100 + var_name.as_u32(),
              ty_id: int_ty,
            });

            let one_sir = self.sir.emit(Insn::ConstInt {
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
        // Finalize pending assignment (x = expr;).
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

        // If nothing consumed the stacks, discard the
        // expression value so it doesn't leak to `}`.
        if !had_assign && !had_decl && !had_return {
          self.value_stack.pop();
          self.ty_stack.pop();
          self.sir_values.pop();
        }
      }

      // === ASSIGNMENT ===
      Token::Eq => {
        // Defer: the RHS hasn't been processed yet.
        // Pop the target identifier's value (it was pushed
        // as a variable reference but it's actually the
        // assignment target). Record the target name.
        // The Semicolon will finalize after the RHS.
        if idx >= 1 {
          let target_idx = idx - 1;
          if let Token::Ident = self.tree.nodes[target_idx].token
            && let Some(NodeValue::Symbol(name)) = self.node_value(target_idx)
          {
            // Pop the target's value from stacks
            // (it was a spurious "use").
            self.value_stack.pop();
            self.ty_stack.pop();
            self.sir_values.pop();

            self.pending_assign = Some(name);
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

      // Skip other tokens for now
      _ => {}
    }
  }

  /// Executes a binary operator.
  fn execute_binop(&mut self, op: BinOp, node_idx: usize) {
    // Pop operands (postfix order: left then right)
    if self.value_stack.len() < 2
      || self.ty_stack.len() < 2
      || self.sir_values.len() < 2
    {
      // Error: not enough operands
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
        // Try constant folding using the ConstFold module
        let constprop = ConstFold::new(&self.values);
        let resolved_ty = self.ty_checker.resolve_ty(ty_id);

        if let Some(folded) =
          constprop.fold_binop(op, lhs, rhs, span, resolved_ty)
        {
          match folded {
            FoldResult::Int(value) => {
              let sir_value = self.sir.emit(Insn::ConstInt { value, ty_id });
              let value_id = self.values.store_int(value);

              self.value_stack.push(value_id);
              self.ty_stack.push(ty_id);
              self.sir_values.push(sir_value);
              self.annotations.push(Annotation { node_idx, ty_id });

              return;
            }
            FoldResult::Float(value) => {
              let sir_value = self.sir.emit(Insn::ConstFloat { value, ty_id });
              let value_id = self.values.store_float(value);

              self.value_stack.push(value_id);
              self.ty_stack.push(ty_id);
              self.sir_values.push(sir_value);
              self.annotations.push(Annotation { node_idx, ty_id });

              return;
            }
            FoldResult::Bool(value) => {
              let ty_id = self.ty_checker.bool_type();

              let sir_value = self.sir.emit(Insn::ConstBool { value, ty_id });
              let value_id = self.values.store_bool(value);

              self.value_stack.push(value_id);
              self.ty_stack.push(ty_id);
              self.sir_values.push(sir_value);
              self.annotations.push(Annotation { node_idx, ty_id });

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
              let rhs_sir_val = self.sir.emit(Insn::ConstInt {
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

        // Runtime operation - emit SIR
        // The destination is the new SSA value being created
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
        self.ty_stack.push(ty_id);
        self.sir_values.push(sir_value);
        self.annotations.push(Annotation { node_idx, ty_id });
      }
      None => {
        // Type error - push error values
        let error_id = self.values.store_runtime(u32::MAX);

        self.value_stack.push(error_id);
        self.ty_stack.push(self.ty_checker.error_type()); // Error type
      }
    }
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
    let constprop = ConstFold::new(&self.values);
    let resolved_ty = self.ty_checker.resolve_ty(ty_id);

    if let Some(folded) = constprop.fold_unop(op, rhs_id, span, resolved_ty) {
      match folded {
        FoldResult::Int(value) => {
          let sir_value = self.sir.emit(Insn::ConstInt { value, ty_id });
          let value_id = self.values.store_int(value);

          self.value_stack.push(value_id);
          self.ty_stack.push(ty_id);
          self.sir_values.push(sir_value);
          self.annotations.push(Annotation { node_idx, ty_id });

          return;
        }
        FoldResult::Float(value) => {
          let sir_value = self.sir.emit(Insn::ConstFloat { value, ty_id });
          let value_id = self.values.store_float(value);

          self.value_stack.push(value_id);
          self.ty_stack.push(ty_id);
          self.sir_values.push(sir_value);
          self.annotations.push(Annotation { node_idx, ty_id });

          return;
        }
        FoldResult::Bool(value) => {
          let sir_value = self.sir.emit(Insn::ConstBool { value, ty_id });
          let value_id = self.values.store_bool(value);

          self.value_stack.push(value_id);
          self.ty_stack.push(ty_id);
          self.sir_values.push(sir_value);
          self.annotations.push(Annotation { node_idx, ty_id });

          return;
        }
        // note: Forward/Strength are unreachable for unary ops,
        // but handle for exhaustiveness.
        FoldResult::Forward(_) | FoldResult::Strength(..) => {
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
    let sir_value = self.sir.emit(Insn::UnOp {
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
  fn execute_ty_annotation(&mut self) {
    if self.value_stack.len() >= 2 && self.ty_stack.len() >= 2 {
      // Pop type value
      let ty_value = self.value_stack.pop().unwrap();
      let _ty_ty = self.ty_stack.pop().unwrap(); // Should be Type type

      if let Some(unified) = self
        .ty_value(ty_value)
        .and_then(|ty| self.ty_stack.last().map(|&var_ty| (ty, var_ty)))
        .and_then(|(ty, var_ty)| self.ty_checker.unify(var_ty, ty, Span::ZERO))
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
        is_pub: self.is_pub(_start_idx),
      });
    }
  }

  /// Resolves a type token at `idx` to a [`TyId`].
  fn resolve_type_token(&mut self, idx: usize) -> TyId {
    match self.tree.nodes[idx].token {
      Token::IntType => self.ty_checker.int_type(),
      Token::S8Type => self.ty_checker.intern_ty(zo_ty::Ty::Int {
        signed: true,
        width: zo_ty::IntWidth::S8,
      }),
      Token::S16Type => self.ty_checker.intern_ty(zo_ty::Ty::Int {
        signed: true,
        width: zo_ty::IntWidth::S16,
      }),
      Token::S32Type => self.ty_checker.s32_type(),
      Token::S64Type => self.ty_checker.intern_ty(zo_ty::Ty::Int {
        signed: true,
        width: zo_ty::IntWidth::S64,
      }),
      Token::UintType => self.ty_checker.intern_ty(zo_ty::Ty::Int {
        signed: false,
        width: zo_ty::IntWidth::U32,
      }),
      Token::U8Type => self.ty_checker.intern_ty(zo_ty::Ty::Int {
        signed: false,
        width: zo_ty::IntWidth::U8,
      }),
      Token::U16Type => self.ty_checker.intern_ty(zo_ty::Ty::Int {
        signed: false,
        width: zo_ty::IntWidth::U16,
      }),
      Token::U32Type => self.ty_checker.u32_type(),
      Token::U64Type => self.ty_checker.intern_ty(zo_ty::Ty::Int {
        signed: false,
        width: zo_ty::IntWidth::U64,
      }),
      Token::FloatType => self.ty_checker.f64_type(),
      Token::F32Type => self.ty_checker.f32_type(),
      Token::F64Type => self.ty_checker.f64_type(),
      Token::BoolType => self.ty_checker.bool_type(),
      Token::CharType => self.ty_checker.char_type(),
      Token::StrType => self.ty_checker.str_type(),
      Token::BytesType => self.ty_checker.intern_ty(zo_ty::Ty::Bytes),
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
      _ => self.ty_checker.unit_type(),
    }
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

    let name = name.unwrap();

    // Parse parameters
    let mut params = Vec::new();
    let mut return_ty = self.ty_checker.unit_type();
    let mut idx = start_idx + 2; // Skip Fun and name

    // Skip past LParen
    if idx < _end_idx && self.tree.nodes[idx].token == Token::LParen {
      idx += 1;

      // Parse parameters until we hit RParen
      while idx < _end_idx {
        let token = &self.tree.nodes[idx].token;

        match token {
          Token::RParen => {
            idx += 1;

            break;
          }
          Token::Ident => {
            // Get parameter name
            if let Some(NodeValue::Symbol(param_name)) = self.node_value(idx) {
              idx += 1;

              // Next should be the type (no colon token)
              if idx < _end_idx {
                let param_ty = self.resolve_type_token(idx);
                params.push((param_name, param_ty));
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
            return_ty = self.resolve_type_token(idx);
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
    let is_pub = self.is_pub(start_idx);

    self.pending_function = Some(FunDef {
      name,
      params: params.clone(),
      return_ty,
      body_start: 0, // Will be set when we emit FunDef
      is_intrinsic: false,
      is_pub,
    });

    // Push a scope for the function parameters
    self.push_scope();

    // Add parameters as local variables
    for (param_name, param_ty) in &params {
      // Parameters are immutable by default
      let value_id = self.values.store_runtime(self.locals.len() as u32);

      self.locals.push(Local {
        name: *param_name,
        ty_id: *param_ty,
        value_id,
        pubness: Pubness::No,
        mutability: Mutability::No,
        sir_value: None,
        is_param: true,
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
  fn begin_decl(&mut self, idx: usize, header: &NodeHeader, is_mutable: bool) {
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
      let is_pub = self.is_pub(idx);

      self.pending_decl = Some(PendingDecl {
        name,
        is_mutable,
        is_pub,
      });

      // Skip: name ident, type annotation, colon, eq.
      // Find the Eq token — init expression starts after it.
      let mut skip_to = idx + 1; // at least skip the Imu
      for i in (idx + 1)..children_end {
        skip_to = i + 1;
        if self.tree.nodes[i].token == Token::Eq {
          break;
        }
      }
      self.skip_until = skip_to;
    }
  }

  /// Finalize a pending variable declaration.
  ///
  /// Finalize a pending assignment (x = expr;).
  fn finalize_pending_assign(&mut self) {
    let name = match self.pending_assign.take() {
      Some(n) => n,
      None => return,
    };

    if let (Some(value), Some(value_ty)) =
      (self.value_stack.pop(), self.ty_stack.pop())
    {
      let value_sir = self.sir_values.pop();

      if let Some(local) = self.locals.iter_mut().rev().find(|l| l.name == name)
      {
        if local.mutability != Mutability::Yes {
          report_error(Error::new(ErrorKind::ImmutableVariable, Span::ZERO));

          return;
        }

        if let Some(unified_ty) =
          self.ty_checker.unify(local.ty_id, value_ty, Span::ZERO)
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

      let mutability = if decl.is_mutable {
        Mutability::Yes
      } else {
        Mutability::No
      };

      let _sir_value = self.sir.emit(Insn::VarDef {
        name: decl.name,
        ty_id: init_ty,
        init: sir_init,
        mutability,
        is_pub: decl.is_pub,
      });

      self.locals.push(Local {
        name: decl.name,
        ty_id: init_ty,
        value_id: init_value,
        pubness: if decl.is_pub {
          Pubness::Yes
        } else {
          Pubness::No
        },
        mutability,
        sir_value: sir_init,
        is_param: false,
      });

      // For mutable variables, emit an initial Store so
      // the value is on the stack frame. Loop iterations
      // will read it via Load and write it via Store.
      if decl.is_mutable
        && let Some(sv) = sir_init
      {
        self.sir.emit(Insn::Store {
          name: decl.name,
          value: sv,
          ty_id: init_ty,
        });
      }

      if let Some(frame) = self.scope_stack.last_mut() {
        frame.count += 1;
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
      // Don't create VarDef here - the template will handle it
      // Store the variable name for the template to use
      if let Some(name) = self.get_var_name(start_idx, end_idx) {
        // Store in a temporary location for the template to pick up
        self.pending_var_name = Some(name);
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
        let is_pub = self.is_pub(start_idx);

        let sir_value = self.sir.emit(Insn::VarDef {
          name,
          ty_id: init_ty,
          init: sir_init,
          mutability: Mutability::No,
          is_pub,
        });

        self.locals.push(Local {
          name,
          ty_id: init_ty,
          value_id: init_value,
          pubness: if is_pub { Pubness::Yes } else { Pubness::No },
          mutability: Mutability::No,
          sir_value: sir_init,
          is_param: false,
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
        let is_pub = self.is_pub(_start_idx);

        let sir_value = self.sir.emit(Insn::VarDef {
          name,
          ty_id: init_ty,
          init: sir_init,
          mutability: Mutability::Yes,
          is_pub,
        });

        self.locals.push(Local {
          name,
          ty_id: init_ty,
          value_id: init_value,
          mutability: Mutability::Yes,
          pubness: if is_pub { Pubness::Yes } else { Pubness::No },
          sir_value: sir_init,
          is_param: false,
        });

        if let Some(frame) = self.scope_stack.last_mut() {
          frame.count += 1;
        }

        // Don't push anything back - declarations don't produce values
        self.sir_values.push(sir_value);
      }
    }
  }

  /// Sets up an if branch context. The actual branch instruction
  /// is emitted when LBrace is hit (condition is on the stack
  /// by then).
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
    let init_sir = self.sir.emit(Insn::ConstInt {
      value: start_val,
      ty_id: int_ty,
    });

    let init_vid = self.values.store_int(start_val);

    self.sir.emit(Insn::VarDef {
      name: var_name,
      ty_id: int_ty,
      init: Some(init_sir),
      mutability: Mutability::Yes,
      is_pub: false,
    });

    self.locals.push(Local {
      name: var_name,
      ty_id: int_ty,
      value_id: init_vid,
      pubness: Pubness::No,
      mutability: Mutability::Yes,
      sir_value: Some(init_sir),
      is_param: false,
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

    let load_src = 100 + var_name.as_u32();

    let load_sir = self.sir.emit(Insn::Load {
      dst: cond_dst,
      src: load_src,
      ty_id: int_ty,
    });

    let end_sir = self.sir.emit(Insn::ConstInt {
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

  /// Executes compound assignment (+=, -=, etc).
  fn execute_compound_assignment(&mut self, op: BinOp, node_idx: usize) {
    // In postorder: target, value, CompoundOp
    // So when we hit CompoundOp, we have value on top of stack
    if let (Some(rhs_value), Some(rhs_ty)) =
      (self.value_stack.pop(), self.ty_stack.pop())
    {
      let rhs_sir = self.sir_values.pop();

      // Look back to find the target variable
      if node_idx >= 2 {
        let target_idx = node_idx - 2;
        if let Token::Ident = self.tree.nodes[target_idx].token
          && let Some(NodeValue::Symbol(name)) = self.node_value(target_idx)
        {
          // Find the variable
          if let Some(local) =
            self.locals.iter_mut().rev().find(|l| l.name == name)
          {
            // Check mutability
            if local.mutability != Mutability::Yes {
              let span = self.tree.spans[node_idx];

              report_error(Error::new(ErrorKind::ImmutableVariable, span));

              return;
            }

            // Type check and perform operation
            let span = self.tree.spans[node_idx];

            if let Some(unified_ty) =
              self.ty_checker.unify(local.ty_id, rhs_ty, span)
            {
              // Try constant folding if both values are compile-time known
              let constprop = ConstFold::new(&self.values);
              let resolved_ty = self.ty_checker.resolve_ty(unified_ty);

              if let Some(folded) = constprop.fold_binop(
                op,
                local.value_id,
                rhs_value,
                span,
                resolved_ty,
              ) {
                match folded {
                  FoldResult::Int(value) => {
                    let new_value = self.values.store_int(value);

                    local.value_id = new_value;

                    let sir_value = self.sir.emit(Insn::ConstInt {
                      value,
                      ty_id: unified_ty,
                    });

                    self.sir.emit(Insn::Store {
                      name,
                      value: sir_value,
                      ty_id: unified_ty,
                    });

                    return;
                  }
                  FoldResult::Float(value) => {
                    let new_value = self.values.store_float(value);

                    local.value_id = new_value;

                    let sir_value = self.sir.emit(Insn::ConstFloat {
                      value,
                      ty_id: unified_ty,
                    });

                    self.sir.emit(Insn::Store {
                      name,
                      value: sir_value,
                      ty_id: unified_ty,
                    });

                    return;
                  }
                  FoldResult::Bool(value) => {
                    let new_value = self.values.store_bool(value);

                    local.value_id = new_value;

                    let sir_value = self.sir.emit(Insn::ConstBool {
                      value,
                      ty_id: unified_ty,
                    });

                    self.sir.emit(Insn::Store {
                      name,
                      value: sir_value,
                      ty_id: unified_ty,
                    });

                    return;
                  }
                  FoldResult::Forward(operand) => {
                    match operand {
                      Operand::Lhs => {
                        // identity: value unchanged (e.g. x += 0).
                        return;
                      }
                      Operand::Rhs => {
                        // absorbing: result is rhs.
                        local.value_id = rhs_value;

                        if let Some(sir_val) = rhs_sir {
                          self.sir.emit(Insn::Store {
                            name,
                            value: sir_val,
                            ty_id: unified_ty,
                          });
                        }

                        return;
                      }
                    }
                  }
                  FoldResult::Strength(new_op, const_rhs) => {
                    // strength reduction for compound assign.
                    // e.g. x *= 8 → x <<= 3.
                    let rhs_sir_val = self.sir.emit(Insn::ConstInt {
                      value: const_rhs,
                      ty_id: unified_ty,
                    });

                    let lhs_sir_val = ValueId(local.value_id.0);
                    let dst = ValueId(self.sir.next_value_id);

                    self.sir.next_value_id += 1;

                    let result_sir = self.sir.emit(Insn::BinOp {
                      dst,
                      op: new_op,
                      lhs: lhs_sir_val,
                      rhs: rhs_sir_val,
                      ty_id: unified_ty,
                    });

                    let runtime_id = self.values.store_runtime(0);

                    local.value_id = runtime_id;

                    self.sir.emit(Insn::Store {
                      name,
                      value: result_sir,
                      ty_id: unified_ty,
                    });

                    return;
                  }
                  FoldResult::Error(error) => {
                    report_error(error);
                    return;
                  }
                }
              }

              // Runtime operation - emit BinOp then Store
              // We need to load the current value first (but we don't have
              // Load yet) For now, we'll emit the BinOp with
              // a placeholder
              if let Some(rhs_sir) = rhs_sir {
                // Create a placeholder for the LHS (the current variable
                // value) This is a simplification - proper
                // SSA would need Load instruction
                let lhs_sir = ValueId(local.value_id.0); // Use the variable's value ID as placeholder
                let dst = ValueId(self.sir.next_value_id);

                self.sir.next_value_id += 1;

                let result_sir = self.sir.emit(Insn::BinOp {
                  dst,
                  op,
                  lhs: lhs_sir,
                  rhs: rhs_sir,
                  ty_id: unified_ty,
                });

                self.sir.emit(Insn::Store {
                  name,
                  value: result_sir,
                  ty_id: unified_ty,
                });

                // Update local's value to runtime
                local.value_id = self.values.store_runtime(0);
              }
            }
          } else {
            let span = self.tree.spans[target_idx];

            report_error(Error::new(ErrorKind::UndefinedVariable, span));
          }
        }
      }
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
      // Check if there's an identifier before LParen
      if lparen_idx > 0 {
        let fun_idx = lparen_idx - 1;

        if let Token::Ident = self.tree.nodes[fun_idx].token {
          // Check if this is a function declaration (has 'fun' before the
          // identifier)
          let is_declaration = if fun_idx > 0 {
            matches!(self.tree.nodes[fun_idx - 1].token, Token::Fun)
          } else {
            false
          };

          // Only execute call if it's not a declaration
          if !is_declaration
            && let Some(NodeValue::Symbol(fun_name)) = self.node_value(fun_idx)
          {
            // This is a function call!
            self.execute_call(fun_name, lparen_idx, rparen_idx);
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
    // Find the function definition
    let fun_def = self.funs.iter().find(|f| f.name == fun_name).cloned();

    if let Some(func) = fun_def {
      // Count arguments between LParen and RParen
      let mut arg_count = 0;
      let mut idx = lparen_idx + 1;

      // Arguments are already evaluated and on the stack in postorder
      // We just need to count them
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

      // Type check: correct number of arguments
      if arg_count != func.params.len() {
        let span = self.tree.spans[rparen_idx];

        report_error(Error::new(ErrorKind::ArgumentCountMismatch, span));

        return;
      }

      // Pop arguments from stack (they're in reverse order)
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

      // Arguments were in reverse order, fix that
      args.reverse();
      arg_types.reverse();
      arg_sirs.reverse();

      // Type check arguments against parameter types
      for (i, ((_, param_ty), arg_ty)) in
        func.params.iter().zip(arg_types.iter()).enumerate()
      {
        let span = self.tree.spans[lparen_idx + 1 + i * 2]; // Approximate span

        if self.ty_checker.unify(*param_ty, *arg_ty, span).is_none() {
          // Type error already reported by unify
          return;
        }
      }

      // Emit Call instruction
      let result_sir = self.sir.emit(Insn::Call {
        name: fun_name,
        args: arg_sirs,
        ty_id: func.return_ty,
      });

      // Push return value
      if func.return_ty != self.ty_checker.unit_type() {
        let result_val = self.values.store_runtime(0);
        self.value_stack.push(result_val);
        self.ty_stack.push(func.return_ty);
        self.sir_values.push(result_sir);
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

      // For external funs, assume they return unit type
      // In a real compiler, we'd have external function declarations
      let return_ty = self.ty_checker.unit_type();

      // Emit Call instruction for external function
      self.sir.emit(Insn::Call {
        name: fun_name,
        args: arg_sirs,
        ty_id: return_ty,
      });

      // External funs typically return unit
      // Don't push anything to the stack for unit returns
    }
  }

  fn execute_directive(&mut self, start_idx: usize, end_idx: usize) {
    // Directives follow pattern: #identifier expression
    // Children: identifier, [expression nodes], semicolon

    // Skip the Hash token itself
    if start_idx + 1 >= end_idx {
      return;
    }

    // Get the directive name
    let dir_idx = start_idx + 1;

    if dir_idx < self.tree.nodes.len()
      && self.tree.nodes[dir_idx].token == Token::Ident
      && let Some(NodeValue::Symbol(sym)) = self.node_value(dir_idx)
    {
      let dir_name = self.interner.get(sym);

      // Handle different directives
      match dir_name {
        "run" => {
          // #run executes code at compile time
          // For now, just note it was encountered
          // Future: execute the expression and store result
        }
        "dom"
          // #dom renders a template to the DOM
          // Pop the template value from the stack
          if !self.value_stack.is_empty() => {
            let template_value = self.value_stack.pop().unwrap();
            let template_ty = self.ty_stack.pop().unwrap();

            // Emit DOM rendering instruction
            self.sir.emit(Insn::Directive {
              name: sym,
              value: template_value,
              ty_id: template_ty,
            });
          }
        "inline" => {
          // #inline hints for inlining
          // Store as metadata for optimization pass
        }
        _ => {
          // Unknown directive - could be user-defined
        }
      }
    }
  }

  fn execute_template_assign(&mut self, start_idx: usize, end_idx: usize) {
    // Template assignment: ::= switches parser to template mode
    // The template fragment should be the next token after ::=
    // Process children which should include the template fragment

    // Execute children (the template fragment)
    for idx in (start_idx + 1)..end_idx {
      let node = &self.tree.nodes[idx];
      self.execute_node(node, idx);
    }

    // After executing children, the template value should be on the stack
    // (pushed by execute_template_fragment)
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
            let trimmed = text.trim();
            if !trimmed.is_empty() {
              commands.push(UiCommand::Text {
                content: trimmed.to_string(),
                style: TextStyle::Normal,
              });
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
        _ => {
          idx += 1;
        }
      }
    }

    if !commands.is_empty() {
      let optimizer = TemplateOptimizer::new();
      commands = optimizer.optimize(commands);
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
    });

    self.sir_values.push(sir_value);

    if let Some(var_name) = self.pending_var_name.take() {
      self.sir.emit(Insn::VarDef {
        name: var_name,
        ty_id: self.ty_checker.template_ty(),
        init: Some(template_id),
        mutability: Mutability::No,
        is_pub: false,
      });
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
    // Track widget id for event binding.
    let mut widget_id: Option<String> = None;

    match classify_tag(tag) {
      TagKind::Container(dir) => {
        let direction = resolve_direction(dir, attrs);
        let id = format!("{}_{}", tag, self.template_counter);
        widget_id = Some(id.clone());
        commands.push(UiCommand::BeginContainer { id, direction });
        if self_closing {
          commands.push(UiCommand::EndContainer);
        }
      }
      TagKind::Text(style) => {
        if self_closing {
          commands.push(UiCommand::Text {
            content: String::new(),
            style,
          });
        } else {
          // Sentinel that close_template_tag converts to Text.
          commands.push(UiCommand::BeginContainer {
            id: format!("__text_{}__", style as u32),
            direction: ContainerDirection::Vertical,
          });
        }
      }
      TagKind::Button => {
        let wid = self.next_widget_id();
        widget_id = Some(wid.to_string());
        if self_closing {
          commands.push(UiCommand::Button {
            id: wid,
            content: String::new(),
          });
        } else {
          // Stash widget id in sentinel for close_template_tag
          commands.push(UiCommand::BeginContainer {
            id: format!("__button_{}__", wid),
            direction: ContainerDirection::Vertical,
          });
        }
      }
      TagKind::Input => {
        let wid = self.next_widget_id();
        widget_id = Some(wid.to_string());
        let placeholder = attr_prop_str(attrs, "placeholder");
        let value = attr_prop_str(attrs, "value");
        commands.push(UiCommand::TextInput {
          id: wid,
          placeholder,
          value,
        });
      }
      TagKind::Image => {
        let id = format!("img_{}", self.template_counter);
        widget_id = Some(id.clone());
        let src = attr_prop_str(attrs, "src");
        let width = attr_prop_num(attrs, "width");
        let height = attr_prop_num(attrs, "height");
        commands.push(UiCommand::Image {
          id,
          src,
          width,
          height,
        });
      }
      TagKind::Unknown => {
        let id = format!("{}_{}", tag, self.template_counter);
        widget_id = Some(id.clone());
        commands.push(UiCommand::BeginContainer {
          id,
          direction: ContainerDirection::Vertical,
        });
        if self_closing {
          commands.push(UiCommand::EndContainer);
        }
      }
    }

    // Emit UiCommand::Event for each @event attribute.
    if let Some(wid) = widget_id {
      for attr in attrs {
        if let Attr::Event {
          event_kind,
          handler,
          ..
        } = attr
        {
          commands.push(UiCommand::Event {
            widget_id: wid.clone(),
            event_kind: event_kind.clone(),
            handler: handler.clone(),
          });
        }
      }
    }
  }

  /// Handle closing tag: convert sentinel BeginContainers
  /// back to their actual UiCommand.
  fn close_template_tag(&mut self, commands: &mut Vec<UiCommand>) {
    // Collect text content from any Text commands added since
    // the last sentinel BeginContainer.
    let sentinel_pos = commands.iter().rposition(|cmd| {
      matches!(cmd, UiCommand::BeginContainer { id, .. }
        if id.starts_with("__"))
    });

    if let Some(pos) = sentinel_pos {
      let sentinel_id = match &commands[pos] {
        UiCommand::BeginContainer { id, .. } => id.clone(),
        _ => unreachable!(),
      };

      // Collect text and preserve event commands after sentinel
      let mut content = String::new();
      let mut events = Vec::new();
      for cmd in &commands[pos + 1..] {
        match cmd {
          UiCommand::Text { content: text, .. } => {
            if !content.is_empty() {
              content.push(' ');
            }
            content.push_str(text);
          }
          UiCommand::Event { .. } => {
            events.push(cmd.clone());
          }
          _ => {}
        }
      }

      // Remove sentinel and collected children
      commands.truncate(pos);

      if sentinel_id.starts_with("__button_") {
        let id: u32 = sentinel_id
          .trim_start_matches("__button_")
          .trim_end_matches("__")
          .parse()
          .unwrap_or(0);
        commands.push(UiCommand::Button { id, content });
      } else if sentinel_id.starts_with("__text_") {
        let style_num: u32 = sentinel_id
          .trim_start_matches("__text_")
          .trim_end_matches("__")
          .parse()
          .unwrap_or(0);
        let style = match style_num {
          0 => TextStyle::Heading1,
          1 => TextStyle::Heading2,
          2 => TextStyle::Heading3,
          3 => TextStyle::Paragraph,
          _ => TextStyle::Normal,
        };
        commands.push(UiCommand::Text { content, style });
      }
      // Re-append preserved event commands
      commands.extend(events);
    } else {
      // Regular container close
      commands.push(UiCommand::EndContainer);
    }
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
  If,
  While,
  For,
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
  // pub(crate) name: Symbol,
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

/// Tag classification for the template tag registry.
enum TagKind {
  Container(ContainerDirection),
  Text(TextStyle),
  Button,
  Input,
  Image,
  Unknown,
}

/// Static tag registry — no allocation, no HashMap.
fn classify_tag(tag: &str) -> TagKind {
  match tag {
    // Containers (vertical by default)
    "div" | "section" | "main" | "article" | "aside" | "header" | "footer"
    | "nav" | "form" | "ul" | "ol" | "li" => {
      TagKind::Container(ContainerDirection::Vertical)
    }
    // Inline container
    "span" => TagKind::Container(ContainerDirection::Horizontal),
    // Text
    "h1" => TagKind::Text(TextStyle::Heading1),
    "h2" => TagKind::Text(TextStyle::Heading2),
    "h3" => TagKind::Text(TextStyle::Heading3),
    "p" => TagKind::Text(TextStyle::Paragraph),
    // Interactive
    "button" => TagKind::Button,
    "input" | "textarea" => TagKind::Input,
    // Media
    "img" => TagKind::Image,
    _ => TagKind::Unknown,
  }
}

/// Resolve container direction from attributes.
fn resolve_direction(
  default: ContainerDirection,
  attrs: &[Attr],
) -> ContainerDirection {
  for attr in attrs {
    if let Attr::Prop {
      name,
      value: PropValue::Str(v),
    } = attr
      && name == "class"
      && v.contains("horizontal")
    {
      return ContainerDirection::Horizontal;
    }
  }
  default
}

/// Look up a string property by name.
fn attr_prop_str(attrs: &[Attr], name: &str) -> String {
  for attr in attrs {
    if attr.name() == name {
      return match attr {
        Attr::Prop {
          value: PropValue::Str(s),
          ..
        } => s.clone(),
        Attr::Prop {
          value: PropValue::Num(n),
          ..
        } => n.to_string(),
        Attr::Prop {
          value: PropValue::Bool(b),
          ..
        } => b.to_string(),
        _ => String::new(),
      };
    }
  }
  String::new()
}

/// Look up a numeric property by name, defaulting to 0.
fn attr_prop_num(attrs: &[Attr], name: &str) -> u32 {
  for attr in attrs {
    if attr.name() == name {
      return match attr {
        Attr::Prop {
          value: PropValue::Num(n),
          ..
        } => *n,
        Attr::Prop {
          value: PropValue::Str(s),
          ..
        } => s.parse().unwrap_or(0),
        _ => 0,
      };
    }
  }
  0
}
