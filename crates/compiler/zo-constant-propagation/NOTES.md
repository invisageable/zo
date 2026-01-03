# Constant Propagation

> Related: See `zo-constant-folding/NOTES.md` for evaluating constant expressions.
> Constant propagation tracks constant values through variables; constant folding evaluates expressions.

• If the value of a variable is known to be a constant, replace the use of the variable by that constant.
• Value of the variable must be propagated forward from the point of assignment.
 - This is a substitution operation.
• Example:

|----------------|        |----------------|        |----------------|        |----------------|
| int x = 5;     |        |                |        |                |        |                |
| int y = x * 2; |   ->   | int y = 5 * 2; |   ->   | int y = 10;    |   ->   |                |
| int z = a[y];  |        | int z = a[y];  |        | int z = a[y];  |        | int z = a[10]; |
|----------------|        |----------------|        |----------------|        |----------------|

• To be most effective, constant propagation can be interleaved with constant folding.

• For safety, it requires a data-flow analysis.
• What performance metric does it intend to improve?
  - Reduces memory accesses (constants are immediates)
  - Enables further optimizations (dead code elimination)
  - Improves instruction selection (immediate operands)
• At which compilation step can it be applied?
  - In zo: During Tree→SIR execution (single pass)
• What is the computational complexity of this optimization?
  - Traditional: O(n²) with dataflow analysis
  - zo's approach: O(1) per operation during execution

---

## Algorithm for zo's Execution Model

### Traditional Approach (Multiple Passes)

Traditional compilers use **Sparse Conditional Constant Propagation** (Wegman & Zadeck, 1991):

```
1. Build CFG and SSA form
2. Initialize lattice values (⊥ = undefined, constants, ⊤ = not constant)  
3. Worklist algorithm:
  - Process each SSA definition
  - Update lattice values
  - Propagate to uses
4. Replace variables with constants
```

This requires multiple passes and doesn't fit zo's execution model.

### zo's Approach: Execution-Time Constant Propagation

Based on **"Partial Evaluation"** (Jones et al., 1993) and **"Abstract Interpretation"** (Cousot, 1977):

```rust
// In zo-constant-propagation crate - ALL propagation logic lives here
pub struct ConstProp {
  // Maps variable Symbol to constant value (if known)
  const_bindings: HashMap<Symbol, ConstValue>,
  // Stack for nested scopes
  scope_stack: Vec<HashMap<Symbol, ConstValue>>,
}

pub enum ConstValue {
  Int(i128),      // Known integer constant
  Uint(u128),     // Known unsigned constant
  Float(f64),     // Known float constant
  Bool(bool),     // Known boolean constant
  NotConstant,    // Variable, but not constant
}

pub enum PropResult {
  Constant(ConstValue),    // Variable is known constant
  Variable(Symbol),         // Not constant, emit variable load
  Invalidated,              // Was constant, now invalidated
}

impl ConstProp {
  pub fn new() -> Self {
    Self {
      const_bindings: HashMap::new(),
      scope_stack: Vec::new(),
    }
  }
  
  // Track variable declaration: x = expr
  pub fn track_declaration(&mut self, var: Symbol, value: ValueId, values: &ValueStorage) {
    // Check if value is a constant
    if let Some(const_val) = self.extract_constant(value, values) {
      self.const_bindings.insert(var, const_val);
    } else {
      self.const_bindings.insert(var, ConstValue::NotConstant);
    }
  }
  
  // Query if variable is constant
  pub fn lookup_variable(&self, var: Symbol) -> PropResult {
    match self.const_bindings.get(&var) {
      Some(ConstValue::NotConstant) | None => PropResult::Variable(var),
      Some(const_val) => PropResult::Constant(*const_val),
    }
  }
  
  // Handle assignment: x = y or x = expr
  pub fn track_assignment(&mut self, target: Symbol, value: ValueId, values: &ValueStorage) {
    if let Some(const_val) = self.extract_constant(value, values) {
      self.const_bindings.insert(target, const_val);
    } else {
      // Assignment kills constantness
      self.const_bindings.insert(target, ConstValue::NotConstant);
    }
  }
  
  // Push/pop scopes for nested blocks
  pub fn push_scope(&mut self) {
    self.scope_stack.push(self.const_bindings.clone());
  }
  
  pub fn pop_scope(&mut self) {
    if let Some(prev) = self.scope_stack.pop() {
      self.const_bindings = prev;
    }
  }
  
  // Helper to extract constant from ValueStorage
  fn extract_constant(&self, value: ValueId, values: &ValueStorage) -> Option<ConstValue> {
    // Check if value is a constant in ValueStorage
    // Implementation depends on ValueStorage structure
    None // Placeholder
  }
}

// In executor.rs - just calls ConstProp
impl<'a> Executor<'a> {
  fn execute_var_decl(&mut self, var: Symbol, value: ValueId) {
    // Let ConstProp handle tracking
    self.constprop.track_declaration(var, value, &self.values);
    // Normal variable declaration handling...
  }
  
  fn execute_var_ref(&mut self, var: Symbol) -> ValueId {
    // Ask ConstProp if this is constant
    match self.constprop.lookup_variable(var) {
      PropResult::Constant(ConstValue::Int(n)) => {
        // Emit constant instead of variable load
        self.sir.emit(Insn::ConstInt { value: n, ty_id });
        self.values.store_int(n)
      }
      PropResult::Variable(var) => {
        // Normal variable load
        self.sir.emit(Insn::LoadLocal { var, ty_id });
        // ... 
      }
      // ... other cases
    }
  }
  
  fn execute_assignment(&mut self, target: Symbol, value: ValueId) {
    // Let ConstProp handle tracking
    self.constprop.track_assignment(target, value, &self.values);
    // Normal assignment handling...
  }
}
```

### Key Algorithms from Literature

1. **"Online Partial Evaluation"** (Consel & Khoo, 1991)
  - Evaluates what can be computed at compile time
  - Perfect for zo's execution model

2. **"Constant Propagation via Abstract Interpretation"** (Cousot & Cousot, 1977)
  - Track abstract values (constant vs non-constant)
  - Sound approximation of runtime behavior

3. **"Copy Propagation"** (Aho, Sethi, Ullman - Dragon Book)
  - When `y = x` and x is constant, y becomes constant
  - Trivial in execution model

### Handling Control Flow

Unlike traditional compilers, zo handles control flow during execution:

```rust
// In ConstProp - handles control flow merging
impl ConstProp {
  // Called before entering if statement
  pub fn enter_if(&mut self, cond: ValueId, values: &ValueStorage) -> BranchInfo {
    // Check if condition is constant
    if let Some(ConstValue::Bool(true)) = self.extract_constant(cond, values) {
      BranchInfo::AlwaysThen
    } else if let Some(ConstValue::Bool(false)) = self.extract_constant(cond, values) {
      BranchInfo::AlwaysElse
    } else {
      // Save environment for merging later
      BranchInfo::Unknown(self.const_bindings.clone())
    }
  }
  
  // Called after executing both branches
  pub fn merge_branches(&mut self, then_env: HashMap<Symbol, ConstValue>, else_env: HashMap<Symbol, ConstValue>) {
    let mut merged = HashMap::new();
    
    // Only keep constants that are same in both branches
    for (var, val1) in then_env.iter() {
      if let Some(val2) = else_env.get(var) {
        if val1 == val2 {
          merged.insert(*var, *val1);
        } else {
          merged.insert(*var, ConstValue::NotConstant);
        }
      }
    }
    
    self.const_bindings = merged;
  }
}

pub enum BranchInfo {
  AlwaysThen,           // Condition is constant true
  AlwaysElse,           // Condition is constant false  
  Unknown(HashMap<Symbol, ConstValue>), // Runtime condition
}

// In executor.rs - just orchestrates the flow
impl<'a> Executor<'a> {
  fn execute_if(&mut self, cond: ValueId, then_branch: NodeId, else_branch: NodeId) {
    match self.constprop.enter_if(cond, &self.values) {
      BranchInfo::AlwaysThen => {
        // Only execute then branch
        self.execute_nodes(then_branch);
      }
      BranchInfo::AlwaysElse => {
        // Only execute else branch
        self.execute_nodes(else_branch);
      }
      BranchInfo::Unknown(saved_env) => {
        // Execute both, then merge
        self.constprop.push_scope();
        self.execute_nodes(then_branch);
        let then_env = self.constprop.const_bindings.clone();
        
        self.constprop.const_bindings = saved_env;
        self.execute_nodes(else_branch);
        let else_env = self.constprop.const_bindings.clone();
        
        self.constprop.merge_branches(then_env, else_env);
      }
    }
  }
}
```

### Interprocedural Constant Propagation

For function calls with constant arguments:

```rust
// In ConstProp - analyzes if call can be compile-time evaluated
impl ConstProp {
  pub fn analyze_call(&self, func: Symbol, args: &[ValueId], values: &ValueStorage) -> CallAnalysis {
    // Check if all arguments are constants
    let const_args: Vec<_> = args.iter()
      .map(arg| self.extract_constant(arg, values))
      .collect();
    
    if const_args.iter().all(|a| a.is_some()) {
      CallAnalysis::AllConstantArgs(const_args)
    } else {
      CallAnalysis::RuntimeCall
    }
  }
}

pub enum CallAnalysis {
  AllConstantArgs(Vec<Option<ConstValue>>),
  RuntimeCall,
}

// In executor.rs - just orchestrates based on ConstProp's analysis
impl<'a> Executor<'a> {
  fn execute_call(&mut self, func: Symbol, args: Vec<ValueId>) -> ValueId {
    match self.constprop.analyze_call(func, &args, &self.values) {
      CallAnalysis::AllConstantArgs(const_args) if self.is_pure_function(func) => {
        // Pure function with constant args = compile-time evaluation!
        self.evaluate_pure_function(func, const_args)
      }
      _ => {
        // Normal runtime call
        self.sir.emit(Insn::Call { func, args, ty_id })
      }
    }
  }
}
```

### Handling Loops

Loops require special care to avoid infinite propagation:

```rust
// In ConstProp - handles loop analysis
impl ConstProp {
  pub fn enter_loop(&mut self, modified_vars: Vec<Symbol>) {
    // Conservative: mark all modified variables as non-constant
    for var in modified_vars {
      self.const_bindings.insert(var, ConstValue::NotConstant);
    }
  }
  
  // For loop peeling optimization
  pub fn save_environment(&self) -> HashMap<Symbol, ConstValue> {
    self.const_bindings.clone()
  }
  
  pub fn restore_environment(&mut self, env: HashMap<Symbol, ConstValue>) {
    self.const_bindings = env;
  }
}

// In executor.rs - just orchestrates
impl<'a> Executor<'a> {
  fn execute_while(&mut self, cond_nodes: NodeId, body_nodes: NodeId) {
    // Find which variables are modified in loop body
    let loop_modified = self.find_modified_vars(body_nodes);
    
    // Tell ConstProp about loop-modified variables
    self.constprop.enter_loop(loop_modified);
    
    // Execute loop normally
    while self.evaluate_condition(cond_nodes) {
      self.execute_nodes(body_nodes);
    }
  }

  // Loop peeling optimization (optional)
  fn execute_while_with_peeling(&mut self, cond: NodeId, body: NodeId) {
    if self.evaluate_condition(cond) {
      // Save environment for first iteration
      let saved_env = self.constprop.save_environment();
      
      // First iteration with constants
      self.execute_nodes(body);
      
      // Find modified vars and mark them non-constant
      let modified = self.find_modified_vars(body);
      self.constprop.enter_loop(modified);
      
      // Remaining iterations
      while self.evaluate_condition(cond) {
        self.execute_nodes(body);
      }
      
      // Could restore if needed: self.constprop.restore_environment(saved_env);
    }
  }
}
```

## Performance Analysis

### Time Complexity

| Operation            | Traditional SCCP     | zo's Execution Model                |
|----------------------|----------------------|-------------------------------------|
| Variable declaration | O(E) edges in CFG    | **O(1)** - direct binding           |
| Variable reference   | O(1) after analysis  | **O(1)** - hashmap lookup           |
| Assignment           | O(E) propagation     | **O(1)** - update binding           |
| Control flow merge   | O(V) lattice joins   | **O(V)** - merge environments       |
| Loop handling        | O(N²) fixpoint       | **O(N)** - single conservative pass |
| **Overall**          | **O(N²)** worst case | **O(N)** single pass                |

Where N = number of nodes, V = number of variables, E = edges in CFG

### Space Complexity

```rust
// Memory overhead per variable
struct ConstBinding {
  symbol: Symbol,      // 4 bytes (interned)
  value: ConstValue,   // 17 bytes (enum with i128)
}

// Total space: O(V * S) where V = variables, S = scope depth
// Typically < 1KB for most functions
```

## Implementation Requirements

### Core Infrastructure Needed

1. **ConstProp Structure in zo-constant-propagation crate**
```rust
pub struct ConstProp {
  const_bindings: HashMap<Symbol, ConstValue>,
  scope_stack: Vec<HashMap<Symbol, ConstValue>>,
}
```

2. **Minimal Integration Points in Executor**
  - `execute_var_decl()` → calls `constprop.track_declaration()`
  - `execute_var_ref()` → calls `constprop.lookup_variable()`
  - `execute_assignment()` → calls `constprop.track_assignment()`
  - `execute_if()` → calls `constprop.enter_if()` and `merge_branches()`
  - `execute_while()` → calls `constprop.mark_loop_modified()`

3. **ConstProp Public API**
```rust
impl ConstProp {
  // Core tracking methods
  pub fn track_declaration(&mut self, var: Symbol, value: ValueId, values: &ValueStorage);
  pub fn lookup_variable(&self, var: Symbol) -> PropResult;
  pub fn track_assignment(&mut self, target: Symbol, value: ValueId, values: &ValueStorage);
  
  // Control flow handling
  pub fn enter_if(&mut self, cond: ValueId, values: &ValueStorage) -> BranchInfo;
  pub fn merge_branches(&mut self, then_env: HashMap<Symbol, ConstValue>, else_env: HashMap<Symbol, ConstValue>);
  pub fn mark_loop_modified(&mut self, vars: Vec<Symbol>);
  
  // Scope management
  pub fn push_scope(&mut self);
  pub fn pop_scope(&mut self);
  
  // Internal helpers (private)
  fn extract_constant(&self, value: ValueId, values: &ValueStorage) -> Option<ConstValue>;
}
```

4. **Additional Helpers (where they belong)**
```rust
// In Executor (owns tree traversal):
impl Executor {
  fn find_modified_vars(&self, start: NodeId, end: NodeId) -> Vec<Symbol>;
  fn is_pure_function(&self, func: Symbol) -> bool;
}

// ConstProp only receives the results:
impl ConstProp {
  pub fn mark_loop_modified(&mut self, vars: Vec<Symbol>);  // Already in API above
}
```

### Production Readiness Checklist

- [ ] Handle all types (int, uint, float, bool)
- [ ] Scope management (push/pop environments)
- [ ] Control flow merging (conservative correctness)
- [ ] Loop invariant detection (optional optimization)
- [ ] Interprocedural propagation for pure functions
- [ ] Integration with constant folding
- [ ] Proper error handling for overflow

## Error Handling

### What ConstProp Does NOT Handle

Unlike traditional compilers, zo's ConstProp doesn't handle errors because of separation of concerns:

1. **Arithmetic errors** (overflow, div-by-zero) → Handled by **ConstFold** when evaluating
2. **Type errors** → Already checked by **TyChecker** before execution
3. **Undefined behavior** → Detected by **ConstFold** during evaluation
4. **Runtime errors** → Handled by **Executor**

ConstProp is purely an **analysis module** that tracks constantness. It never evaluates expressions or produces errors.

### What ConstProp DOES Handle

```rust
impl ConstProp {
  // lookup_variable is only called for variables that exist (already validated)
  pub fn lookup_variable(&self, var: Symbol) -> PropResult {
    match self.const_bindings.get(&var) {
      Some(ConstValue::NotConstant) => PropResult::Variable(var),
      Some(const_val) => PropResult::Constant(*const_val),
      None => {
        // Variable not in our tracking - could be:
        // 1. Parameter (not tracked as local)
        // 2. Global variable
        // 3. Variable from outer scope before we started tracking
        // These are runtime variables, not constants
        PropResult::Variable(var)
      }
    }
  }
}
```

**Why None is not an error**: By the time we reach constant propagation, semantic analysis has already verified all variables exist. If a variable isn't in our tracking, it's simply not a local constant - it's a runtime value from parameters, globals, or outer scopes.

## Why This Works in zo

1. **Single Pass**: No need for iterative dataflow analysis
2. **Natural Integration**: Fits directly into execution model
3. **Cache Friendly**: Linear access patterns
4. **Predictable Performance**: O(1) per operation
5. **Correctness by Construction**: Execution order guarantees soundness
6. **Clean Error Boundaries**: Each module handles its own error domain

## References

1. Wegman, M. N., & Zadeck, F. K. (1991). "Constant propagation with conditional branches"
2. Jones, N. D., Gomard, C. K., & Sestoft, P. (1993). "Partial evaluation and automatic program generation"
3. Cousot, P., & Cousot, R. (1977). "Abstract interpretation: a unified lattice model"
4. Consel, C., & Khoo, S. C. (1991). "Online partial evaluation"
5. Click, C., & Cooper, K. D. (1995). "Combining analyses, combining optimizations"
