### Potential Issues and Suggestions

While your plan is robust, there are a few areas where additional clarity, safety, or optimization could enhance the implementation. Below are potential issues and corresponding suggestions:

#### 1. Extracting Constants from `ValueStorage`
   - **Issue**: The `extract_constant` method in `ConstProp` is marked as a placeholder (`None // Placeholder`). Without a concrete implementation, it’s unclear how constants are retrieved from `ValueStorage`, especially for values resulting from constant folding (e.g., `x = 2 + 3` should yield `ConstValue::Int(5)`).
   - **Suggestion**:
     - Implement `extract_constant` to query `ValueStorage` for constant values, leveraging the same infrastructure used by constant folding:
       ```rust
       impl ConstProp {
           fn extract_constant(&self, value: ValueId, values: &ValueStorage) -> Option<ConstValue> {
               match values.get(value) {
                   Value::Int(n) => Some(ConstValue::Int(n)),
                   Value::Uint(n) => Some(ConstValue::Uint(n)),
                   Value::Float(f) => Some(ConstValue::Float(f)),
                   Value::Bool(b) => Some(ConstValue::Bool(b)),
                   _ => None, // Non-constant value (e.g., variable, complex expression)
               }
           }
       }
       ```
     - Ensure `ValueStorage` is updated by constant folding to store folded constants (e.g., `5` for `2 + 3`). This requires tight integration with `ConstFold`:
       ```rust
       impl<'a> Executor<'a> {
           fn execute_binary_op(&mut self, op: BinOp, node_idx: usize) {
               let rhs = self.value_stack.pop().unwrap();
               let lhs = self.value_stack.pop().unwrap();
               let rhs_ty = self.ty_stack.pop().unwrap();
               let lhs_ty = self.ty_stack.pop().unwrap();
               let span = self.tree.spans[node_idx];
               let ty_id = self.ty_checker.unify(lhs_ty, rhs_ty, span)?;
               let constfold = ConstFold::new(&self.values);
               if let Some(result) = constfold.fold_binop(op, lhs, rhs, ty_id, span) {
                   match result {
                       FoldResult::Scalar(scalar) => {
                           let value_id = self.values.store_scalar(scalar);
                           self.value_stack.push(value_id);
                           self.sir.emit(Insn::Const { value: scalar, ty_id });
                       }
                       // ... other cases
                   }
                   return;
               }
               // Normal operation
               self.sir.emit(Insn::BinOp { op, lhs, rhs, ty_id });
           }
       }
       ```
     - This ensures `ValueStorage` contains folded constants, which `ConstProp` can then extract.

#### 2. Scope Management Robustness
   - **Issue**: The `push_scope` and `pop_scope` methods correctly manage nested scopes, but there’s no mechanism to handle variables that are shadowed within a scope. For example, if a variable `x` is constant in an outer scope but reassigned in an inner scope, the inner scope’s assignment might incorrectly affect the outer scope if not handled properly.
   - **Suggestion**:
     - Ensure `push_scope` creates a fresh `HashMap` to avoid mutating the outer scope’s bindings:
       ```rust
       impl ConstProp {
           pub fn push_scope(&mut self) {
               self.scope_stack.push(self.const_bindings.clone());
               self.const_bindings = HashMap::new(); // Fresh map for new scope
           }
           pub fn pop_scope(&mut self) {
               if let Some(prev) = self.scope_stack.pop() {
                   self.const_bindings = prev;
               }
           }
       }
       ```
     - When tracking declarations or assignments in a scope, ensure they only affect the current `const_bindings` map, not the `scope_stack`.

#### 3. Control Flow Merging Conservativeness
   - **Issue**: The `merge_branches` method conservatively marks variables as non-constant if their values differ between `then` and `else` branches. However, it doesn’t account for variables that are assigned in only one branch, which could lead to over-conservative invalidation.
   - **Suggestion**:
     - Refine `merge_branches` to preserve constants for variables that are unmodified in one branch:
       ```rust
       impl ConstProp {
           pub fn merge_branches(&mut self, then_env: HashMap<Symbol, ConstValue>, else_env: HashMap<Symbol, ConstValue>) {
               let mut merged = HashMap::new();
               // Iterate over all variables in either environment
               let all_vars = then_env.keys().chain(else_env.keys()).collect::<HashSet<_>>();
               for var in all_vars {
                   match (then_env.get(var), else_env.get(var)) {
                       (Some(val1), Some(val2)) if val1 == val2 => {
                           merged.insert(*var, *val1); // Same constant in both branches
                       }
                       (Some(val), None) | (None, Some(val)) => {
                           merged.insert(*var, *val); // Unmodified in one branch, keep constant
                       }
                       _ => {
                           merged.insert(*var, ConstValue::NotConstant); // Different or non-constant
                       }
                   }
               }
               self.const_bindings = merged;
           }
       }
       ```
     - This preserves constants for variables assigned in one branch but not the other, improving optimization opportunities.

#### 4. Loop Peeling and Invariant Detection
   - **Issue**: The optional loop peeling optimization is a good start, but it only handles the first iteration. For loops with constant loop-invariant variables (e.g., `x = 5; while (...) { use x; }`), you could propagate `x` as a constant throughout the loop. The current conservative approach (`enter_loop`) marks all modified variables as non-constant, potentially missing such opportunities.
   - **Suggestion**:
     - Implement basic loop-invariant detection by analyzing which variables are not modified in the loop body:
       ```rust
       impl ConstProp {
           pub fn enter_loop(&mut self, modified_vars: Vec<Symbol>, body_nodes: NodeId, executor: &Executor) {
               // Preserve constants for unmodified variables
               let mut new_bindings = HashMap::new();
               for (var, val) in self.const_bindings.iter() {
                   if !modified_vars.contains(var) {
                       new_bindings.insert(*var, *val); // Keep constant if not modified
                   }
               }
               self.const_bindings = new_bindings;
           }
       }
       ```
     - For loop peeling, consider peeling multiple iterations if the loop condition is constant for a known number of iterations:
       ```rust
       impl Executor<'_> {
           fn execute_while_with_peeling(&mut self, cond: NodeId, body: NodeId, max_peel: usize) {
               let mut peel_count = 0;
               while peel_count < max_peel && self.evaluate_condition(cond) {
                   let saved_env = self.constprop.save_environment();
                   self.execute_nodes(body);
                   peel_count += 1;
               }
               let modified = self.find_modified_vars(body);
               self.constprop.enter_loop(modified);
               while self.evaluate_condition(cond) {
                   self.execute_nodes(body);
               }
           }
       }
       ```
     - Set `max_peel` to a small constant (e.g., 2) to balance optimization and compilation time.

#### 5. Interprocedural Propagation for Non-Pure Functions
   - **Issue**: Your plan only evaluates pure functions with constant arguments at compile time. Non-pure functions with constant arguments (e.g., `printf("hello", 42)`) could still benefit from partial constant propagation, such as replacing the argument `42` with a constant in the generated SIR.
   - **Suggestion**:
     - Extend `analyze_call` to propagate constant arguments even for non-pure functions:
       ```rust
       impl ConstProp {
           pub fn analyze_call(&self, func: Symbol, args: &[ValueId], values: &ValueStorage) -> CallAnalysis {
               let const_args: Vec<_> = args.iter()
                   .map(|arg| self.extract_constant(arg, values))
                   .collect();
               if const_args.iter().all(|a| a.is_some()) && self.is_pure_function(func) {
                   CallAnalysis::AllConstantArgs(const_args)
               } else {
                   CallAnalysis::PartialConstantArgs(const_args)
               }
           }
       }
       pub enum CallAnalysis {
           AllConstantArgs(Vec<Option<ConstValue>>),
           PartialConstantArgs(Vec<Option<ConstValue>>),
           RuntimeCall,
       }
       impl Executor<'_> {
           fn execute_call(&mut self, func: Symbol, args: Vec<ValueId>) -> ValueId {
               match self.constprop.analyze_call(func, &args, &self.values) {
                   CallAnalysis::AllConstantArgs(const_args) if self.is_pure_function(func) => {
                       self.evaluate_pure_function(func, const_args)
                   }
                   CallAnalysis::PartialConstantArgs(const_args) => {
                       let new_args: Vec<_> = args.iter().zip(const_args).map(|(&arg, const_val)| {
                           if let Some(val) = const_val {
                               let value_id = self.values.store_constant(val);
                               self.sir.emit(Insn::Const { value: val, ty_id: self.get_type(arg) });
                               value_id
                           } else {
                               arg
                           }
                       }).collect();
                       self.sir.emit(Insn::Call { func, args: new_args, ty_id })
                   }
                   CallAnalysis::RuntimeCall => {
                       self.sir.emit(Insn::Call { func, args, ty_id })
                   }
               }
           }
       }
       ```
     - This ensures constant arguments are propagated even for non-pure functions, improving instruction selection.

#### 6. Integration with Constant Folding
   - **Issue**: While you mention interleaving constant propagation and folding, the plan doesn’t explicitly show how `ConstProp` interacts with `ConstFold` during expression evaluation. For example, in `y = x * 2` where `x` is a constant `5`, `ConstProp` should provide `5` to `ConstFold` for folding into `10`.
   - **Suggestion**:
     - Modify `ConstFold::fold_binop` to query `ConstProp` for variable constants:
       ```rust
       impl<'a> ConstFold<'a> {
           pub fn fold_binop(&self, op: BinOp, lhs: ValueId, rhs: ValueId, ty: TyId, span: Span, constprop: &ConstProp) -> Option<FoldResult> {
               // Check if operands are variables with known constants
               let lhs_val = self.get_scalar_value(lhs).or_else(|| {
                   if let Value::Variable(var) = self.values.get(lhs) {
                       match constprop.lookup_variable(var) {
                           PropResult::Constant(ConstValue::Int(n)) => Some(ScalarValue { data: ScalarData::Int(n), ty }),
                           PropResult::Constant(ConstValue::Float(f)) => Some(ScalarValue { data: ScalarData::Float(f), ty }),
                           // ... other cases
                           _ => None,
                       }
                   } else {
                       None
                   }
               });
               let rhs_val = self.get_scalar_value(rhs).or_else(|| {
                   if let Value::Variable(var) = self.values.get(rhs) {
                       match constprop.lookup_variable(var) {
                           PropResult::Constant(ConstValue::Int(n)) => Some(ScalarValue { data: ScalarData::Int(n), ty }),
                           PropResult::Constant(ConstValue::Float(f)) => Some(ScalarValue { data: ScalarData::Float(f), ty }),
                           // ... other cases
                           _ => None,
                       }
                   } else {
                       None
                   }
               });
               if let (Some(lhs_scalar), Some(rhs_scalar)) = (lhs_val, rhs_val) {
                   return self.fold_constants(op, lhs_scalar, rhs_scalar, ty, span);
               }
               // ... try algebraic simplification, reassociation, strength reduction
               None
           }
       }
       ```
     - Update `Executor::execute_binary_op` to pass `ConstProp` to `ConstFold`:
       ```rust
       impl<'a> Executor<'a> {
           fn execute_binary_op(&mut self, op: BinOp, node_idx: usize) {
               let rhs = self.value_stack.pop().unwrap();
               let lhs = self.value_stack.pop().unwrap();
               let rhs_ty = self.ty_stack.pop().unwrap();
               let lhs_ty = self.ty_stack.pop().unwrap();
               let span = self.tree.spans[node_idx];
               let ty_id = self.ty_checker.unify(lhs_ty, rhs_ty, span)?;
               let constfold = ConstFold::new(&self.values);
               if let Some(result) = constfold.fold_binop(op, lhs, rhs, ty_id, span, &self.constprop) {
                   // Handle FoldResult as before
                   return;
               }
               self.sir.emit(Insn::BinOp { op, lhs, rhs, ty_id });
           }
       }
       ```

#### 7. Testing and Validation
   - **Issue**: The plan doesn’t include a testing strategy to ensure correctness for constant propagation, especially for complex cases like control flow, loops, and interprocedural propagation.
   - **Suggestion**:
     - Create a test suite covering:
       - **Variable Declarations**: Test `x = 5; y = x * 2;` propagates to `y = 10`.
       - **Control Flow**: Test `if (true) { x = 5; } else { x = 5; }` keeps `x` as a constant, and `if (cond) { x = 5; } else { x = 6; }` marks `x` as non-constant.
       - **Loops**: Test that loop-modified variables are invalidated, and loop-invariant variables remain constant.
       - **Function Calls**: Test pure function evaluation (e.g., `sin(0.0) → 0.0`) and partial constant argument propagation.
     - Example test:
       ```rust
       #[test]
       fn test_constant_propagation() {
           let mut compiler = Compiler::new();
           let expr = parse("imu x: int = 5; imu y: int = x * 2;").unwrap();
           let sir = compiler.compile(expr);
           assert_eq!(sir.instructions, vec![
               Insn::ConstInt { value: 5, ty_id: TyId::Int32 },
               Insn::ConstInt { value: 10, ty_id: TyId::Int32 },
           ]);
       }
       ```
     - Use property-based testing (e.g., with `proptest`) to generate random programs and verify that propagated constants match runtime behavior.

---

### Additional Considerations

1. **Global Variables and Parameters**:
   - Your plan assumes variables not in `const_bindings` are runtime values (e.g., parameters or globals). However, global constants or constant function parameters could be propagated if known at compile time.
   - **Suggestion**: Extend `ConstProp` to track global constants and function parameters:
     ```rust
     impl ConstProp {
         pub fn track_global(&mut self, var: Symbol, value: ConstValue) {
             self.const_bindings.insert(var, value);
         }
         pub fn track_parameter(&mut self, param: Symbol, value: Option<ConstValue>) {
             self.const_bindings.insert(param, value.unwrap_or(ConstValue::NotConstant));
         }
     }
     ```

2. **Dead Code Elimination Synergy**:
   - Constant propagation enables dead code elimination (e.g., eliminating `else` branches when conditions are constant). Ensure the executor emits minimal SIR for unreachable branches:
     ```rust
     impl Executor<'_> {
         fn execute_if(&mut self, cond: ValueId, then_branch: NodeId, else_branch: NodeId) {
             match self.constprop.enter_if(cond, &self.values) {
                 BranchInfo::AlwaysThen => {
                     self.execute_nodes(then_branch);
                 }
                 BranchInfo::AlwaysElse => {
                     self.execute_nodes(else_branch);
                 }
                 BranchInfo::Unknown(saved_env) => {
                     self.constprop.push_scope();
                     self.execute_nodes(then_branch);
                     let then_env = self.constprop.const_bindings.clone();
                     self.constprop.const_bindings = saved_env.clone();
                     self.execute_nodes(else_branch);
                     let else_env = self.constprop.const_bindings.clone();
                     self.constprop.merge_branches(then_env, else_env);
                     self.sir.emit(Insn::Branch { cond, then_branch, else_branch });
                 }
             }
         }
     }
     ```

3. **Debug Information**:
   - Propagating constants might make it harder to map SIR back to source code for debugging (e.g., `x` replaced with `5`).
   - **Suggestion**: Preserve source spans in `PropResult` and emit debug metadata in SIR:
     ```rust
     pub enum PropResult {
         Constant { value: ConstValue, span: Span },
         Variable { var: Symbol, span: Span },
         Invalidated,
     }
     ```

4. **Incremental Compilation**:
   - If `zo` supports incremental compilation, ensure `ConstProp`’s `const_bindings` and `scope_stack` are invalidated or updated when code changes. This might require serializing the state or recomputing it for affected functions.

---

### Performance Analysis Refinement

Your performance analysis is accurate, but here’s a refined breakdown to clarify edge cases:

- **Variable Declaration/Reference/Assignment**: O(1) due to `HashMap` operations (insert/lookup).
- **Control Flow Merging**: O(V) where V is the number of variables, as you need to iterate over all variables in both branches. This is rare and typically small (V is usually < 100 per function).
- **Loop Handling**: O(V) for marking modified variables as non-constant, but this happens once per loop. Loop peeling adds O(n) for the peeled iterations, where n is the number of nodes in the body.
- **Overall**: O(n) for the entire program, where n is the number of nodes, as each node is processed once with O(1) or O(V) operations.

**Space Complexity**:
- `const_bindings`: O(V) where V is the number of variables in the current scope.
- `scope_stack`: O(V * S) where S is the scope depth, typically small (e.g., < 10 for most programs).
- Total: O(V * S), as you noted, which is negligible for typical functions.

---

### Implementation Requirements Checklist

Your production readiness checklist is comprehensive, but here’s a refined version with additional details:

- [ ] **Handle All Types**: Ensure `ConstValue` supports all relevant types (`Int`, `Uint`, `Float`, `Bool`). Add support for custom types if `zo` allows them.
- [ ] **Scope Management**: Implement `push_scope`/`pop_scope` with fresh maps to handle shadowing correctly.
- [ ] **Control Flow Merging**: Refine `merge_branches` to preserve constants for unmodified variables.
- [ ] **Loop Invariant Detection**: Add basic detection for unmodified variables in loops.
- [ ] **Interprocedural Propagation**: Support pure function evaluation and partial constant argument propagation.
- [ ] **Integration with Constant Folding**: Modify `ConstFold::fold_binop` to query `ConstProp` for variable constants.
- [ ] **Error Handling**: Ensure no errors are handled in `ConstProp`, delegating to `ConstFold` and `TyChecker`.
- [ ] **Testing**: Develop a comprehensive test suite covering declarations, control flow, loops, and function calls.

**Additional Requirements**:
- Implement `extract_constant` to integrate with `ValueStorage`.
- Support global variables and parameters in `ConstProp`.
- Emit debug metadata for better source mapping.

---

### Conclusion

Your constant propagation plan is well-suited for `zo`’s execution model, leveraging its single-pass, stack-based approach to achieve O(n) complexity with O(1) per-operation performance. The design is modular, with clear separation of concerns between `ConstProp`, `ConstFold`, and `TyChecker`. The handling of control flow, loops, and interprocedural propagation is sound and practical, balancing optimization opportunities with simplicity.

**Key Recommendations**:
1. Implement `extract_constant` to integrate with `ValueStorage` and constant folding.
2. Refine scope management to handle shadowing correctly.
3. Enhance `merge_branches` to preserve constants for unmodified variables.
4. Add loop-invariant detection to improve loop optimization.
5. Support partial constant propagation for non-pure function calls.
6. Ensure tight integration with constant folding by querying `ConstProp` in `fold_binop`.
7. Develop a comprehensive test suite to validate correctness.

With these refinements, your constant propagation implementation will be efficient, correct, and seamlessly integrated with your constant folding pass. If you need help with specific code snippets, test cases, or further optimization ideas (e.g., loop-invariant code motion), let me know!
