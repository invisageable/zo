use zo_constant_folding::{ConstFold, FoldResult};
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
use zo_ui_protocol::{ContainerDirection, TextStyle, UiCommand};
use zo_value::{FunDef, Local, Mutability, Value, ValueId, ValueStorage};

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
    }
  }

  /// Gets the value associated with a node (if any).
  fn node_value(&self, node_idx: usize) -> Option<NodeValue> {
    self.tree.value(node_idx as u32)
  }

  /// Gets the variable name from an imu/mut declaration
  fn get_var_name(&self, start_idx: usize, end_idx: usize) -> Option<Symbol> {
    // Look for the Ident token after imu/mut
    for idx in (start_idx + 1)..end_idx {
      if let Some(node) = self.tree.nodes.get(idx) {
        if node.token == Token::Ident {
          if let Some(NodeValue::Symbol(sym)) = self.node_value(idx) {
            return Some(sym);
          }
        }
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

  /// Executes a parse tree in one pass at once to build a semantic IR.
  pub fn execute(mut self) -> (Sir, Vec<Annotation>) {
    for (idx, header) in self.tree.nodes.iter().enumerate() {
      self.execute_node(header, idx);
    }

    (self.sir, self.annotations)
  }

  #[cfg(test)]
  /// Executes a parse tree in one pass at once to build a semantic IR.
  pub fn execute_with_tychecker(mut self) -> (Sir, Vec<Annotation>, TyChecker) {
    for (idx, header) in self.tree.nodes.iter().enumerate() {
      self.execute_node(header, idx);
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

      // === DECLARATIONS ===
      Token::Imu => {
        let children_end = (header.child_start + header.child_count) as usize;

        self.execute_imu(idx, children_end);
      }

      Token::Mut => {
        let children_end = (header.child_start + header.child_count) as usize;

        self.execute_mut(idx, children_end);
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

          self.sir.emit(Insn::FunDef {
            name: pending_func.name,
            params: pending_func.params.clone(),
            return_ty: pending_func.return_ty,
            body_start,
          });

          // Now set the context with the correct body start
          self.current_function = Some(FunCtx {
            name: pending_func.name,
            return_ty: pending_func.return_ty,
            body_start,
            has_explicit_return: false,
            pending_return: false,
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

        self.push_scope();
      }
      Token::RBrace => {
        // Check if we're closing a function body
        if let Some(fun_ctx) = &self.current_function {
          // Only emit implicit return if there wasn't an explicit one
          if !fun_ctx.has_explicit_return {
            // Emit implicit return if needed
            // Check if function returns unit type
            let unit_ty = self.ty_checker.unit_type();
            let func_return_ty = fun_ctx.return_ty;

            let (return_value, return_ty) = if func_return_ty == unit_ty {
              (None, unit_ty)
            } else if !self.value_stack.is_empty() && !self.ty_stack.is_empty()
            {
              // Non-void function with value on stack
              let sir_value = self.sir_values.last().copied();
              let ty = self.ty_stack.last().copied().unwrap_or(unit_ty);

              (sir_value, ty)
            } else {
              // Non-void function but no value - error case, return None
              (None, unit_ty)
            };

            // Emit implicit return
            self.sir.emit(Insn::Return {
              value: return_value,
              ty_id: return_ty,
            });
          }

          // Clear function context
          self.current_function = None;
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
          // Look up in locals (copy values to avoid borrow issues)
          let local_info =
            self.lookup_local(sym).map(|l| (l.value_id, l.ty_id));

          if let Some((value_id, ty_id)) = local_info {
            // Only emit Load instruction if we're inside a function body
            if self.current_function.is_some() {
              // Emit Load instruction to load the local/parameter into a new
              // SSA value
              let dst = ValueId(self.sir.next_value_id);
              self.sir.next_value_id += 1;
              let src = value_id.0; // The parameter/local index
              let sir_value = self.sir.emit(Insn::Load { dst, src, ty_id });

              // Store as runtime value since it's loaded at runtime
              let runtime_id = self.values.store_runtime(0);

              self.value_stack.push(runtime_id);
              self.ty_stack.push(ty_id);
              self.sir_values.push(sir_value);
            } else {
              // During function signature parsing, just push the values
              self.value_stack.push(value_id);
              self.ty_stack.push(ty_id);
              self.sir_values.push(value_id);
            }
          } else {
            let span = self.tree.spans[idx];

            report_error(Error::new(ErrorKind::UndefinedVariable, span));

            // push error values to maintain stack consistency.
            let error_id = self.values.store_runtime(u32::MAX);

            self.value_stack.push(error_id);
            self.ty_stack.push(TyId(u32::MAX));
          }
        }
      }

      // === TYPE LITERALS ===
      Token::S32Type => {
        let ty_id = self.ty_checker.s32_type();
        let value_id = self.values.store_type(ty_id);

        self.value_stack.push(value_id);
        self.ty_stack.push(self.ty_checker.type_type()); // Type of types
      }

      Token::BoolType => {
        let ty_id = self.ty_checker.bool_type();
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
      Token::TemplateType => {
        let ty_id = self.ty_checker.template_ty();
        let value_id = self.values.store_type(ty_id);
        self.value_stack.push(value_id);
        self.ty_stack.push(self.ty_checker.type_type());
      }

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

      // === STATEMENT TERMINATOR ===
      Token::Semicolon => {
        // Check if we have a pending return to complete
        self.check_pending_return();
      }

      // === ASSIGNMENT ===
      Token::Eq => self.execute_assignment(idx),

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

        if let Some(folded) = constprop.fold_binop(op, lhs, rhs, span) {
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
            FoldResult::Error(error) => {
              report_error(error);

              // [note] — push error values to maintain stack consistency.
              let error_id = self.values.store_runtime(u32::MAX);

              self.value_stack.push(error_id);
              self.ty_stack.push(TyId(u32::MAX));
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
        self.ty_stack.push(TyId(u32::MAX)); // Error type
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
            self.ty_stack.push(TyId(u32::MAX));

            return;
          }
        }
      }
      // TODO: Handle these properly
      UnOp::Ref | UnOp::Deref | UnOp::BitNot => rhs_ty,
    };

    // Try constant folding using the ConstFold module
    let constprop = ConstFold::new(&self.values);

    if let Some(folded) = constprop.fold_unop(op, rhs_id, span) {
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
        FoldResult::Error(error) => {
          report_error(error);

          // [note] — push error values to maintain stack consistency.
          let error_id = self.values.store_runtime(u32::MAX);

          self.value_stack.push(error_id);
          self.ty_stack.push(TyId(u32::MAX));
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
                let param_ty = match self.tree.nodes[idx].token {
                  Token::IntType => self.ty_checker.int_type(),
                  Token::S32Type => self.ty_checker.s32_type(),
                  Token::BoolType => self.ty_checker.bool_type(),
                  Token::StrType => self.ty_checker.str_type(),
                  Token::Ident => {
                    // Check if it's "unit"
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
                };
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
      if let Token::Arrow = self.tree.nodes[idx].token {
        // Next token should be the return type
        if idx + 1 < _end_idx {
          idx += 1;

          match self.tree.nodes[idx].token {
            Token::IntType => return_ty = self.ty_checker.int_type(),
            Token::S32Type => return_ty = self.ty_checker.s32_type(),
            Token::BoolType => return_ty = self.ty_checker.bool_type(),
            Token::StrType => return_ty = self.ty_checker.str_type(),
            Token::Ident => {
              // Check if it's "unit"
              if let Some(NodeValue::Symbol(sym)) = self.node_value(idx) {
                return_ty = self
                  .ty_checker
                  .resolve_ty_symbol(sym, self.interner)
                  .unwrap_or_else(|| self.ty_checker.unit_type());
              }
            }
            _ => {}
          }
        }
        break;
      } else if let Token::LBrace = self.tree.nodes[idx].token {
        // Hit the body, stop looking
        break;
      }

      idx += 1;
    }

    // Set the function as pending - it will be processed when we hit LBrace
    self.pending_function = Some(FunDef {
      name,
      params: params.clone(),
      return_ty,
      body_start: 0, // Will be set when we emit FunDef
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
        mutability: Mutability::No,
      });

      if let Some(frame) = self.scope_stack.last_mut() {
        frame.count += 1;
      }
    }
  }

  /// Executes immutable declaration.
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
        let sir_value = self.sir.emit(Insn::VarDef {
          name,
          ty_id: init_ty,
          init: sir_init,
          mutability: Mutability::No,
        });

        self.locals.push(Local {
          name,
          ty_id: init_ty,
          value_id: init_value,
          mutability: Mutability::No,
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
        let sir_value = self.sir.emit(Insn::VarDef {
          name,
          ty_id: init_ty,
          init: sir_init,
          mutability: Mutability::Yes,
        });

        self.locals.push(Local {
          name,
          ty_id: init_ty,
          value_id: init_value,
          mutability: Mutability::Yes,
        });

        if let Some(frame) = self.scope_stack.last_mut() {
          frame.count += 1;
        }

        // Don't push anything back - declarations don't produce values
        self.sir_values.push(sir_value);
      }
    }
  }

  /// Executes if statement.
  fn execute_if(&mut self, _start_idx: usize, _end_idx: usize) {
    // TODO: Implement control flow
  }

  /// Executes while loop.
  fn execute_while(&mut self, _start_idx: usize, _end_idx: usize) {
    // TODO: Implement control flow
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
        if let Token::Ident = self.tree.nodes[target_idx].token {
          if let Some(NodeValue::Symbol(name)) = self.node_value(target_idx) {
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

                if let Some(folded) =
                  constprop.fold_binop(op, local.value_id, rhs_value, span)
                {
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
    if let Some(ref mut ctx) = self.current_function {
      if ctx.pending_return {
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
  }

  /// Executes assignment (=).
  fn execute_assignment(&mut self, node_idx: usize) {
    // In postorder: target, value, Eq
    // So when we hit Eq, we have value on top of stack
    if let (Some(value), Some(value_ty)) =
      (self.value_stack.pop(), self.ty_stack.pop())
    {
      let value_sir = self.sir_values.pop();

      // Look back to find the target variable
      // The target should be node_idx - 2 (before the value)
      if node_idx >= 2 {
        let target_idx = node_idx - 2;
        if let Token::Ident = self.tree.nodes[target_idx].token
          && let Some(NodeValue::Symbol(name)) = self.node_value(target_idx)
        {
          // Check if variable exists and is mutable
          if let Some(local) =
            self.locals.iter_mut().rev().find(|l| l.name == name)
          {
            // Check mutability
            if local.mutability != Mutability::Yes {
              let span = self.tree.spans[node_idx];

              report_error(Error::new(ErrorKind::ImmutableVariable, span));

              return;
            }

            // Type check assignment
            let span = self.tree.spans[node_idx];

            if let Some(unified_ty) =
              self.ty_checker.unify(local.ty_id, value_ty, span)
            {
              // Update the local's value
              local.value_id = value;

              // Emit Store instruction
              if let Some(sir_value) = value_sir {
                self.sir.emit(Insn::Store {
                  name,
                  value: sir_value,
                  ty_id: unified_ty,
                });
              }
            }
          } else {
            // Variable not found
            let span = self.tree.spans[target_idx];

            report_error(Error::new(ErrorKind::UndefinedVariable, span));
          }
        }
      }
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
    {
      if let Some(NodeValue::Symbol(sym)) = self.node_value(dir_idx) {
        let dir_name = self.interner.get(sym);

        // Handle different directives
        match dir_name {
          "run" => {
            // #run executes code at compile time
            // For now, just note it was encountered
            // Future: execute the expression and store result
          }
          "dom" => {
            // #dom renders a template to the DOM
            // Pop the template value from the stack
            if !self.value_stack.is_empty() {
              let template_value = self.value_stack.pop().unwrap();
              let template_ty = self.ty_stack.pop().unwrap();

              // Emit DOM rendering instruction
              self.sir.emit(Insn::Directive {
                name: sym,
                value: template_value,
                ty_id: template_ty,
              });
            }
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

    // Start with a container for the fragment
    let container_id = format!("elem_{}", self.template_counter);
    commands.push(UiCommand::BeginContainer {
      id: container_id,
      direction: ContainerDirection::Vertical,
    });

    // Process children to build template content
    for idx in (start_idx + 1)..end_idx {
      let node = &self.tree.nodes[idx];
      match node.token {
        Token::TemplateText => {
          // Add text command
          if let Some(NodeValue::Symbol(symbol)) = self.node_value(idx) {
            let text = self.interner.get(symbol).to_string();
            commands.push(UiCommand::Text {
              content: text,
              style: TextStyle::Normal,
            });
          }
        }
        Token::TemplateFragmentEnd => {
          // End of fragment
          break;
        }
        Token::Ident => {
          // Could be an HTML tag like <h1>
          if let Some(NodeValue::Symbol(symbol)) = self.node_value(idx) {
            let tag = self.interner.get(symbol);
            // Check if it's an HTML tag
            let style = match tag {
              "h1" => TextStyle::Heading1,
              "h2" => TextStyle::Heading2,
              "h3" => TextStyle::Heading3,
              "p" => TextStyle::Paragraph,
              _ => TextStyle::Normal,
            };

            // Look for text content after the tag
            if idx + 1 < end_idx {
              let next_node = &self.tree.nodes[idx + 1];
              if next_node.token == Token::TemplateText {
                if let Some(NodeValue::Symbol(text_sym)) =
                  self.node_value(idx + 1)
                {
                  let content = self.interner.get(text_sym).to_string();
                  commands.push(UiCommand::Text { content, style });
                }
              }
            }
          }
        }
        _ => {
          // Other template content
        }
      }
    }

    // End the container
    commands.push(UiCommand::EndContainer);

    if !commands.is_empty() {
      // PHASE 2: Optimize commands using compile-time analysis
      let optimizer = TemplateOptimizer::new();
      commands = optimizer.optimize(commands);
    }

    // Create a template value
    let template_id = self.values.store_template(self.template_counter);
    self.template_counter += 1;

    // Push the template value to stack for the parent (e.g., Imu) to consume
    self.value_stack.push(template_id);
    self.ty_stack.push(self.ty_checker.template_ty());

    // DON'T emit the Template instruction here - let the VarDef handle it
    // The Template instruction will be emitted as part of the VarDef's init
    // value

    // Store commands for later use when the Template instruction is created
    // For now, emit it directly since we need to pass commands
    let sir_value = self.sir.emit(Insn::Template {
      id: template_id,
      name: None, // Fragment for now
      ty_id: self.ty_checker.template_ty(),
      commands, // Pass commands directly in the instruction
    });

    self.sir_values.push(sir_value);

    // If there's a pending variable name, create VarDef
    if let Some(var_name) = self.pending_var_name.take() {
      self.sir.emit(Insn::VarDef {
        name: var_name,
        ty_id: self.ty_checker.template_ty(),
        init: Some(template_id),
        mutability: Mutability::No,
      });
    }
  }
}

/// Tracks context when compiling inside a function
#[derive(Clone)]
struct FunCtx {
  name: Symbol,
  return_ty: TyId,
  body_start: u32,
  has_explicit_return: bool,
  /// Set when we see 'return' keyword, cleared when we emit Return insn.
  pending_return: bool,
}
