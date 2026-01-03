; Zo language syntax highlighting queries

; ===== Comments =====

(line_comment) @comment
(block_comment) @comment

; ===== Literals =====

(integer_literal) @number
(float_literal) @number
(boolean_literal) @constant.builtin
(char_literal) @character
(string_literal) @string
(raw_string_literal) @string
(bytes_literal) @string
(escape_sequence) @string.escape

; ===== Keywords =====

[
  "pack"
  "load"
  "type"
  "ext"
  "abstract"
  "apply"
  "fun"
  "fn"
  "val"
  "struct"
  "enum"
  "group"
  "state"
  "imu"
  "mut"
  "raw"
  "for"
  "while"
  "loop"
  "if"
  "else"
  "match"
  "when"
  "return"
  "nursery"
  "spawn"
  "await"
  "as"
  "is"
  "and"
] @keyword

; Break and continue are single-token expressions
(break_expression) @keyword
(continue_expression) @keyword

; ===== Special Keywords =====

(visibility) @keyword.modifier
(self_type) @type.builtin
(self) @variable.builtin
(boolean_literal) @constant.builtin

; ===== Operators =====

[
  "+"
  "-"
  "*"
  "/"
  "%"
  "!"
  "&&"
  "||"
  "&"
  "|"
  "^"
  "<<"
  ">>"
  "=="
  "!="
  "<"
  ">"
  "<="
  ">="
  "="
  "+="
  "-="
  "*="
  "/="
  "%="
  "&="
  "|="
  "^="
  "<<="
  ">>="
  ".."
  "..="
  "|>"
] @operator

; ===== Punctuation =====

[
  ","
  "."
  ";"
  ":"
  "::"
  ":="
  "::="
  "->"
  "=>"
  "%%"
  "#"
  "$"
] @punctuation.delimiter

(ellipsis) @punctuation.delimiter

[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
  "<"
  ">"
] @punctuation.bracket

; ===== Types =====

(primitive_type) @type.builtin

(generic_type
  "$" @punctuation.special
  (identifier) @type)

(array_type
  "[" @punctuation.bracket
  "]" @punctuation.bracket)

(tuple_type) @type

(function_type
  "Fn" @type.builtin)

(reference_type
  "&" @operator
  "mut" @keyword.modifier)

(template_type) @type.builtin

(self_type) @type.builtin

; ===== Declarations =====

(pack_declaration
  name: (identifier) @module)

(load_declaration
  path: (module_path) @module)

(type_declaration
  name: (identifier) @type.definition)

(function_declaration
  name: (identifier) @function)

(abstract_declaration
  name: (identifier) @type)

(abstract_method
  name: (identifier) @function)

(apply_declaration
  trait: (identifier) @type
  type: (identifier)? @type)

(struct_declaration
  name: (identifier) @type)

(enum_declaration
  name: (identifier) @type)

(enum_variant
  name: (identifier) @constant)

(val_declaration
  name: (identifier) @constant)

(state_declaration
  (state_variant
    name: (identifier) @type))

(field
  name: (identifier) @property)

(parameter
  name: (identifier) @variable.parameter)

(generic_parameter
  "$" @punctuation.special
  (identifier) @type)

; ===== Expressions =====

(closure_expression
  "fn" @keyword)

(struct_expression
  type: (path_expression) @type)

(match_arm
  pattern: (pattern) @variable)

(postfix_expression
  field: (identifier) @property)

; Call expressions - function names
(postfix_expression
  function: (primary_expression
    (identifier) @function.call))

(postfix_expression
  function: (primary_expression
    (path_expression
      (path_segment) @function.call .)))

; ===== Patterns =====

(wildcard_pattern) @variable.builtin

(struct_pattern_field
  name: (identifier) @property)

; ===== Templates (JSX-like) =====

(template_element
  name: (identifier) @tag)

(template_attribute
  name: (identifier) @attribute)

(template_text) @string

(template_interpolation
  "{" @punctuation.special
  "}" @punctuation.special)

; ===== Attributes =====

(attribute
  "%%" @punctuation.special
  name: (identifier) @attribute)

; ===== Directives =====

(directive
  "#" @punctuation.special
  name: (identifier) @function.macro)

; ===== Path Expressions =====

(path_expression
  "::" @punctuation.delimiter)

; ===== Identifiers =====

; Type identifiers (PascalCase)
((identifier) @type
  (#match? @type "^[A-Z][a-zA-Z0-9]*$"))

; Constant identifiers (ALL_CAPS)
((identifier) @constant
  (#match? @constant "^[A-Z][A-Z0-9_]*$"))

; Default identifier
(identifier) @variable
