; Zo language locals (scope tracking) queries

; ===== Scope Definitions =====

(source_file) @scope

(function_declaration) @scope
(block) @scope
(closure_expression) @scope
(for_statement) @scope
(for_expression) @scope
(while_statement) @scope
(while_expression) @scope
(loop_statement) @scope
(loop_expression) @scope
(if_statement) @scope
(if_expression) @scope
(match_arm) @scope
(nursery_statement) @scope

; ===== Definitions =====

(function_declaration
  name: (identifier) @definition.function)

(parameter
  name: (identifier) @definition.parameter)

(imu_statement
  pattern: (pattern
    (identifier) @definition.var))

(mut_statement
  pattern: (pattern
    (identifier) @definition.var))

(val_declaration
  name: (identifier) @definition.constant)

(struct_declaration
  name: (identifier) @definition.type)

(enum_declaration
  name: (identifier) @definition.type)

(type_declaration
  name: (identifier) @definition.type)

(for_statement
  pattern: (pattern
    (identifier) @definition.var))

(for_expression
  variable: (identifier) @definition.var)

(nursery_item
  name: (identifier) @definition.var)

(generic_parameter
  (identifier) @definition.type)

; ===== References =====

(identifier) @reference
