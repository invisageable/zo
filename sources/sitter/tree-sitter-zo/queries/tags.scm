; Zo language tags (code navigation) queries

; ===== Function Definitions =====

(function_declaration
  name: (identifier) @name) @definition.function

(abstract_method
  name: (identifier) @name) @definition.method

; ===== Type Definitions =====

(struct_declaration
  name: (identifier) @name) @definition.class

(enum_declaration
  name: (identifier) @name) @definition.class

(type_declaration
  name: (identifier) @name) @definition.type

(abstract_declaration
  name: (identifier) @name) @definition.interface

; ===== Module Definitions =====

(pack_declaration
  name: (identifier) @name) @definition.module

; ===== Constant Definitions =====

(val_declaration
  name: (identifier) @name) @definition.constant

(enum_variant
  name: (identifier) @name) @definition.constant

; ===== Field Definitions =====

(field
  name: (identifier) @name) @definition.field

; ===== Call References =====

(postfix_expression
  function: (primary_expression
    (identifier) @name)) @reference.call

(postfix_expression
  function: (primary_expression
    (path_expression
      (path_segment) @name))) @reference.call
