/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

/**
 * Tree-sitter grammar for the Zo programming language.
 * Based on zo.ebnf grammar specification.
 */

const PREC = {
  TERNARY: 1,
  OR: 2,
  AND: 3,
  EQ: 4,
  CMP: 5,
  BIT_OR: 6,
  BIT_XOR: 7,
  BIT_AND: 8,
  SHIFT: 9,
  ADD: 10,
  MUL: 11,
  CAST: 12,
  UNARY: 13,
  POSTFIX: 14,
  CALL: 15,
};

module.exports = grammar({
  name: 'zo',

  externals: $ => [
    $.block_comment,
  ],

  extras: $ => [
    /\s/,
    $.line_comment,
    $.block_comment,
  ],

  word: $ => $.identifier,

  conflicts: $ => [
    [$.primary_expression, $.path_segment],
    [$._statement, $.primary_expression],
    [$.for_statement, $.for_expression, $.pattern, $.path_segment],
    [$.while_statement, $.while_expression],
    [$.loop_statement, $.loop_expression],
    [$.if_statement, $.if_expression],
    [$.for_statement, $.for_expression],
    [$.pattern, $.path_segment],
    [$.module_path],
  ],

  rules: {
    // ===== Top Level Program =====

    source_file: $ => repeat($._item),

    // ===== Items (Top Level Constructs) =====

    _item: $ => seq(
      optional($.attributes),
      optional($.visibility),
      $._item_content,
    ),

    _item_content: $ => choice(
      $.pack_declaration,
      $.load_declaration,
      $.directive,
      $.type_declaration,
      $.ext_declaration,
      $.abstract_declaration,
      $.apply_declaration,
      $.function_declaration,
      $.val_declaration,
      $.struct_declaration,
      $.enum_declaration,
      $.group_type_declaration,
    ),

    // ===== Attributes =====

    attributes: $ => repeat1($.attribute),

    attribute: $ => seq(
      '%%',
      field('name', $.identifier),
      optional(seq('(', $.identifier, ')')),
    ),

    // ===== Visibility =====

    visibility: $ => 'pub',

    // ===== Pack Declaration =====

    pack_declaration: $ => seq(
      'pack',
      field('name', $.identifier),
      optional($.pack_body),
    ),

    pack_body: $ => seq('{', repeat($._item), '}'),

    // ===== Load Declaration =====

    load_declaration: $ => seq(
      'load',
      field('path', $.module_path),
      optional(seq('::', '(', $.import_list, ')')),
      ';',
    ),

    module_path: $ => seq(
      $.identifier,
      repeat(prec.right(1, seq('::', $.identifier))),
    ),

    import_list: $ => seq(
      $.identifier,
      repeat(seq(',', $.identifier)),
      optional(','),
    ),

    // ===== Directive =====

    directive: $ => prec.right(seq(
      '#',
      field('name', $.identifier),
      field('value', $._expression),
    )),

    // ===== Type Declaration =====

    type_declaration: $ => seq(
      'type',
      field('name', $.identifier),
      '=',
      field('type', $._type),
    ),

    // ===== Group Type Declaration =====

    group_type_declaration: $ => seq(
      'group',
      'type',
      $.type_binding,
      repeat(seq('and', $.type_binding)),
      ';',
    ),

    type_binding: $ => seq(
      field('name', $.identifier),
      '=',
      field('type', $._type),
    ),

    // ===== External Declaration =====

    ext_declaration: $ => seq(
      'ext',
      field('name', $.identifier),
      '(',
      optional($.parameter_list),
      ')',
      '->',
      field('return_type', $._type),
      ';',
    ),

    // ===== Abstract Declaration =====

    abstract_declaration: $ => seq(
      'abstract',
      field('name', $.identifier),
      '{',
      repeat($.abstract_method),
      '}',
    ),

    abstract_method: $ => seq(
      'fun',
      field('name', $.identifier),
      '(',
      optional($.parameter_list),
      ')',
      optional(seq('->', field('return_type', $._type))),
      ';',
    ),

    // ===== Apply Declaration =====

    apply_declaration: $ => seq(
      'apply',
      field('trait', $.identifier),
      optional(seq('for', field('type', $.identifier))),
      '{',
      repeat($._apply_content),
      '}',
    ),

    _apply_content: $ => choice(
      $.function_declaration,
      $.state_declaration,
    ),

    // ===== State Declaration (for typestates) =====

    state_declaration: $ => seq(
      'state',
      $.state_variant,
      repeat(seq('and', $.state_variant)),
    ),

    state_variant: $ => seq(
      field('name', $.identifier),
      optional(seq('{', $.field_list, '}')),
    ),

    // ===== Function Declaration =====

    function_declaration: $ => seq(
      optional('raw'),
      choice('fun', 'fn'),
      field('name', $.identifier),
      optional($.generic_parameters),
      '(',
      optional($.parameter_list),
      ')',
      optional(seq(
        '->',
        field('return_type', $._type),
        optional(seq('->>', '(', $.identifier, ')')),
      )),
      field('body', $.block),
    ),

    generic_parameters: $ => seq(
      '<',
      $.generic_parameter,
      repeat(seq(',', $.generic_parameter)),
      optional(','),
      '>',
    ),

    generic_parameter: $ => seq('$', $.identifier),

    parameter_list: $ => seq(
      $.parameter,
      repeat(seq(',', $.parameter)),
      optional(','),
    ),

    parameter: $ => seq(
      field('name', choice($.identifier, 'self')),
      ':',
      field('type', $._type),
    ),

    // ===== Value Declaration =====

    val_declaration: $ => seq(
      'val',
      field('name', $.identifier),
      ':',
      field('type', $._type),
      '=',
      field('value', $._expression),
      ';',
    ),

    // ===== Struct Declaration =====

    struct_declaration: $ => seq(
      optional($.typestate_attribute),
      'struct',
      field('name', $.identifier),
      optional($.generic_parameters),
      '{',
      optional($.field_list),
      '}',
    ),

    typestate_attribute: $ => 'type@state',

    field_list: $ => seq(
      $.field,
      repeat(seq(',', $.field)),
      optional(','),
    ),

    field: $ => seq(
      optional($.visibility),
      field('name', $.identifier),
      ':',
      field('type', $._type),
      optional(seq('=', field('default', $._expression))),
    ),

    // ===== Enum Declaration =====

    enum_declaration: $ => seq(
      'enum',
      field('name', $.identifier),
      optional($.generic_parameters),
      '{',
      $.enum_variant_list,
      '}',
    ),

    enum_variant_list: $ => seq(
      $.enum_variant,
      repeat(seq(',', $.enum_variant)),
      optional(','),
    ),

    enum_variant: $ => seq(
      field('name', $.identifier),
      optional(seq('(', field('type', $._type), ')')),
      optional(seq('=', field('value', $.integer_literal))),
    ),

    // ===== Statements =====

    _statement: $ => choice(
      $.imu_statement,
      $.mut_statement,
      $.assignment_statement,
      $.loop_statement,
      $.while_statement,
      $.for_statement,
      $.nursery_statement,
      $.if_statement,
      $.block,
      $.directive,
      $.expression_statement,
    ),

    imu_statement: $ => seq(
      'imu',
      field('pattern', $.pattern),
      choice(
        seq(':', field('type', $._type), '=', field('value', $._expression)),
        seq(':=', field('value', $._expression)),
        seq('::=', field('value', $.template_literal)),
      ),
      ';',
    ),

    mut_statement: $ => seq(
      'mut',
      field('pattern', $.pattern),
      choice(
        seq(':', field('type', $._type), '=', field('value', $._expression)),
        seq(':=', field('value', $._expression)),
      ),
      ';',
    ),

    assignment_statement: $ => seq(
      field('left', $._expression),
      field('operator', $.assignment_operator),
      field('right', $._expression),
      ';',
    ),

    assignment_operator: $ => choice(
      '=', '+=', '-=', '*=', '/=', '%=',
      '<<=', '>>=', '&=', '|=', '^=',
    ),

    expression_statement: $ => seq($._expression, ';'),

    // ===== Control Flow Statements =====

    loop_statement: $ => choice(
      seq('loop', $.block),
      seq('loop', '=>', $._expression),
    ),

    while_statement: $ => choice(
      seq('while', field('condition', $._expression), $.block),
      seq('while', field('condition', $._expression), '=>', $._expression),
    ),

    for_statement: $ => choice(
      seq('for', field('pattern', $.pattern), ':=', field('iterator', $._expression), $.block),
      seq('for', field('variable', $.identifier), ':=', field('iterator', $._expression), '=>', $._expression),
    ),

    nursery_statement: $ => seq(
      'nursery',
      '{',
      repeat($.nursery_item),
      '}',
    ),

    nursery_item: $ => seq(
      'imu',
      field('name', $.identifier),
      ':=',
      choice(
        seq('spawn', field('value', $._expression)),
        seq('await', field('value', $._expression)),
      ),
      ';',
    ),

    if_statement: $ => seq(
      'if',
      field('condition', $._expression),
      field('consequence', $.block),
      optional(seq('else', field('alternative', choice($.if_statement, $.block)))),
    ),

    // ===== Block =====

    block: $ => seq(
      '{',
      repeat($._statement),
      optional(field('value', $._expression)),
      '}',
    ),

    // ===== Expressions =====

    _expression: $ => choice(
      $.ternary_expression,
      $.binary_expression,
      $.unary_expression,
      $.cast_expression,
      $.postfix_expression,
      $.primary_expression,
    ),

    ternary_expression: $ => prec.right(PREC.TERNARY, seq(
      field('condition', $._expression),
      'when',
      field('guard', $._expression),
      '?',
      field('consequence', $._expression),
      ':',
      field('alternative', $._expression),
    )),

    binary_expression: $ => choice(
      prec.left(PREC.OR, seq(field('left', $._expression), '||', field('right', $._expression))),
      prec.left(PREC.AND, seq(field('left', $._expression), '&&', field('right', $._expression))),
      prec.left(PREC.EQ, seq(field('left', $._expression), choice('==', '!='), field('right', $._expression))),
      prec.left(PREC.CMP, seq(field('left', $._expression), choice('<', '>', '<=', '>='), field('right', $._expression))),
      prec.left(PREC.BIT_OR, seq(field('left', $._expression), '|', field('right', $._expression))),
      prec.left(PREC.BIT_XOR, seq(field('left', $._expression), '^', field('right', $._expression))),
      prec.left(PREC.BIT_AND, seq(field('left', $._expression), '&', field('right', $._expression))),
      prec.left(PREC.SHIFT, seq(field('left', $._expression), choice('<<', '>>'), field('right', $._expression))),
      prec.left(PREC.ADD, seq(field('left', $._expression), choice('+', '-'), field('right', $._expression))),
      prec.left(PREC.MUL, seq(field('left', $._expression), choice('*', '/', '%'), field('right', $._expression))),
      // Pipe operator
      prec.left(PREC.POSTFIX, seq(field('left', $._expression), '|>', field('right', $._expression))),
      // Range operators
      prec.left(PREC.CMP, seq(field('left', $._expression), '..', field('right', $._expression))),
      prec.left(PREC.CMP, seq(field('left', $._expression), '..=', field('right', $._expression))),
    ),

    cast_expression: $ => prec.left(PREC.CAST, seq(
      field('value', $._expression),
      'as',
      field('type', $._type),
    )),

    unary_expression: $ => prec.right(PREC.UNARY, seq(
      field('operator', choice('!', '-', '+')),
      field('operand', $._expression),
    )),

    postfix_expression: $ => choice(
      // Field access
      prec.left(PREC.POSTFIX, seq(
        field('object', $._expression),
        '.',
        field('field', $.identifier),
      )),
      // Tuple index access
      prec.left(PREC.POSTFIX, seq(
        field('object', $._expression),
        '.',
        field('index', $.integer_literal),
      )),
      // Index access
      prec.left(PREC.POSTFIX, seq(
        field('object', $._expression),
        '[',
        field('index', $._expression),
        ']',
      )),
      // Function call
      prec.left(PREC.CALL, seq(
        field('function', $._expression),
        '(',
        optional($.argument_list),
        ')',
      )),
      // Pattern check
      prec.left(PREC.POSTFIX, seq(
        field('value', $._expression),
        'is',
        field('pattern', $.pattern),
      )),
    ),

    argument_list: $ => seq(
      $._expression,
      repeat(seq(',', $._expression)),
      optional(','),
    ),

    primary_expression: $ => choice(
      $._literal,
      $.identifier,
      $.path_expression,
      $.self,
      $.parenthesized_expression,
      $.array_expression,
      $.tuple_expression,
      $.struct_expression,
      $.closure_expression,
      $.if_expression,
      $.match_expression,
      $.loop_expression,
      $.while_expression,
      $.for_expression,
      $.block,
      $.return_expression,
      $.break_expression,
      $.continue_expression,
      $.ellipsis,
    ),

    self: $ => choice('self', 'Self'),

    parenthesized_expression: $ => seq('(', $._expression, ')'),

    array_expression: $ => seq(
      '[',
      optional(seq(
        $._expression,
        repeat(seq(',', $._expression)),
        optional(','),
      )),
      ']',
    ),

    tuple_expression: $ => seq(
      '(',
      optional(seq(
        $._expression,
        ',',
        repeat(seq($._expression, ',')),
        optional($._expression),
      )),
      ')',
    ),

    struct_expression: $ => prec(PREC.CALL, seq(
      field('type', $.path_expression),
      '{',
      optional($.struct_field_list),
      '}',
    )),

    struct_field_list: $ => seq(
      $.struct_field,
      repeat(seq(',', $.struct_field)),
      optional(','),
    ),

    struct_field: $ => choice(
      // Shorthand: identifier
      $.identifier,
      // Named: identifier: expr
      seq(field('name', $.identifier), ':', field('value', $._expression)),
      // Assignment style: identifier = expr
      seq(field('name', $.identifier), '=', field('value', $._expression)),
    ),

    closure_expression: $ => seq(
      'fn',
      '(',
      optional($.parameter_list),
      ')',
      optional(seq('->', field('return_type', $._type))),
      choice(
        seq('=>', field('body', $._expression)),
        field('body', $.block),
      ),
    ),

    if_expression: $ => seq(
      'if',
      field('condition', $._expression),
      field('consequence', $.block),
      optional(seq('else', field('alternative', choice($.if_expression, $.block)))),
    ),

    match_expression: $ => seq(
      'match',
      field('value', $._expression),
      '{',
      $.match_arm,
      repeat(seq(',', $.match_arm)),
      optional(','),
      '}',
    ),

    match_arm: $ => prec.right(PREC.CALL, seq(
      field('pattern', $.pattern),
      '=>',
      field('value', choice($._expression, $.block)),
    )),

    loop_expression: $ => choice(
      seq('loop', $.block),
      seq('loop', '=>', $._expression),
    ),

    while_expression: $ => choice(
      seq('while', field('condition', $._expression), $.block),
      seq('while', field('condition', $._expression), '->', $._expression),
    ),

    for_expression: $ => choice(
      seq('for', field('variable', $.identifier), ':=', field('iterator', $._expression), $.block),
      seq('for', field('variable', $.identifier), ':=', field('iterator', $._expression), '=>', $._expression),
    ),

    return_expression: $ => prec.right(seq('return', optional($._expression))),

    break_expression: $ => 'break',

    continue_expression: $ => 'continue',

    ellipsis: $ => '...',

    // ===== Patterns =====

    pattern: $ => choice(
      $.wildcard_pattern,
      $.identifier,
      $._literal,
      $.path_pattern,
      $.tuple_pattern,
      $.array_pattern,
      $.struct_pattern,
    ),

    wildcard_pattern: $ => '_',

    path_pattern: $ => prec.left(seq(
      $.path_expression,
      optional(seq('(', $.pattern, ')')),
    )),

    tuple_pattern: $ => seq(
      '(',
      $.pattern,
      repeat(seq(',', $.pattern)),
      optional(','),
      ')',
    ),

    array_pattern: $ => seq(
      '[',
      $.pattern,
      repeat(seq(',', $.pattern)),
      optional(','),
      ']',
    ),

    struct_pattern: $ => seq(
      '{',
      $.struct_pattern_field,
      repeat(seq(',', $.struct_pattern_field)),
      optional(','),
      '}',
    ),

    struct_pattern_field: $ => seq(
      field('name', $.identifier),
      optional(seq(':', field('pattern', $.pattern))),
    ),

    // ===== Template Literals =====

    template_literal: $ => choice(
      $.template_element,
      $.template_fragment,
    ),

    template_fragment: $ => seq(
      '<>',
      repeat($._template_node),
      '</>',
    ),

    template_element: $ => choice(
      // Self-closing element
      seq(
        '<',
        field('name', $.identifier),
        repeat($.template_attribute),
        '/>',
      ),
      // Element with children
      seq(
        '<',
        field('name', $.identifier),
        repeat($.template_attribute),
        '>',
        repeat($._template_node),
        '</',
        $.identifier,
        '>',
      ),
    ),

    _template_node: $ => choice(
      $.template_element,
      $.template_text,
      $.template_interpolation,
    ),

    template_interpolation: $ => seq('{', $._expression, '}'),

    template_attribute: $ => seq(
      field('name', $.identifier),
      '=',
      field('value', choice(
        seq('{', $._expression, '}'),
        $.string_literal,
      )),
    ),

    template_text: $ => /[^<>{}]+/,

    // ===== Types =====

    _type: $ => choice(
      $.primitive_type,
      $.path_type,
      $.array_type,
      $.tuple_type,
      $.function_type,
      $.generic_type,
      $.reference_type,
      $.template_type,
      $.self_type,
    ),

    primitive_type: $ => choice(
      'int', 's8', 's16', 's32', 's64',
      'uint', 'u8', 'u16', 'u32', 'u64',
      'float', 'f32', 'f64',
      'bool', 'bytes', 'char', 'str',
    ),

    path_type: $ => $.path_expression,

    array_type: $ => seq(
      '[',
      optional($.integer_literal),
      ']',
      $._type,
    ),

    tuple_type: $ => seq(
      '(',
      $._type,
      repeat(seq(',', $._type)),
      optional(','),
      ')',
    ),

    function_type: $ => seq(
      'Fn',
      '(',
      optional(seq(
        $._type,
        repeat(seq(',', $._type)),
        optional(','),
      )),
      ')',
      '->',
      $._type,
    ),

    generic_type: $ => seq('$', $.identifier),

    reference_type: $ => seq(
      '&',
      optional('mut'),
      $._type,
    ),

    template_type: $ => '</>',

    self_type: $ => 'Self',

    // ===== Paths =====

    path_expression: $ => prec.left(seq(
      optional('::'),
      $.path_segment,
      repeat(seq('::', $.path_segment)),
    )),

    path_segment: $ => $.identifier,

    // ===== Literals =====

    _literal: $ => choice(
      $.integer_literal,
      $.float_literal,
      $.boolean_literal,
      $.bytes_literal,
      $.char_literal,
      $.string_literal,
      $.raw_string_literal,
    ),

    integer_literal: $ => choice(
      $.decimal_literal,
      $.binary_literal,
      $.octal_literal,
      $.hex_literal,
      $.base_literal,
    ),

    decimal_literal: $ => /[0-9][0-9_]*/,

    binary_literal: $ => /0b[01][01_]*/,

    octal_literal: $ => /0o[0-7][0-7_]*/,

    hex_literal: $ => /0x[0-9a-fA-F][0-9a-fA-F_]*/,

    base_literal: $ => /[box]#[0-9]+/,

    float_literal: $ => choice(
      /[0-9][0-9_]*\.[0-9][0-9_]*([eE][+-]?[0-9][0-9_]*)?/,
      /[0-9][0-9_]*[eE][+-]?[0-9][0-9_]*/,
    ),

    boolean_literal: $ => choice('true', 'false'),

    char_literal: $ => seq(
      "'",
      choice($.escape_sequence, /[^'\\]/),
      "'",
    ),

    bytes_literal: $ => seq(
      '`',
      $.escape_sequence,
      '`',
    ),

    string_literal: $ => seq(
      '"',
      repeat(choice(
        $.escape_sequence,
        $.string_content,
      )),
      '"',
    ),

    string_content: $ => /[^"\\]+/,

    raw_string_literal: $ => seq(
      '$"',
      /[^"]*/,
      '"',
    ),

    escape_sequence: $ => choice(
      /\\[nrt\\'\"0]/,
      /\\x[0-9a-fA-F]{2}/,
    ),

    // ===== Identifiers =====

    identifier: $ => /[a-zA-Z_][a-zA-Z0-9_]*/,

    // ===== Comments =====

    line_comment: $ => token(seq('--', /.*/)),

    // Block comments are handled by external scanner (src/scanner.c)
    // to support nested comments: -* outer -* inner *- outer *-
  },
});
