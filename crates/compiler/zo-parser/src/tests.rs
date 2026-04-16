pub(crate) mod arrays;
pub(crate) mod assignments;
pub(crate) mod common;
pub(crate) mod directives;
pub(crate) mod errors;
pub(crate) mod generics;
pub(crate) mod modules;
pub(crate) mod styles;
pub(crate) mod templates;
pub(crate) mod unary;

use crate::tests::common::assert_nodes_stream;

use zo_token::Token::{
  Abstract, Apply, Arrow, As, BoolType, CharType, Colon, ColonEq, Comma, Dot,
  DotDot, DotDotEq, Else, Eq, False, FloatType, For, Fun, Gt, Ident, If, Imu,
  Int, IntType, LBrace, LBracket, LParen, Lt, Minus, Mut, Plus, RBrace,
  RBracket, RParen, Return, S32Type, SelfLower, Semicolon, Star, StrType, True,
  While,
};
use zo_tree::NodeValue;

#[test]
fn test_reordering_complex_lit() {
  assert_nodes_stream(
    r#"
      fun main() -> int {
        1 + 2 * 3 - 4
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (Arrow, None),
      (IntType, None),
      (LBrace, None),
      (Int, Some(NodeValue::Literal(0))),
      (Int, Some(NodeValue::Literal(1))),
      (Int, Some(NodeValue::Literal(2))),
      (Star, None),
      (Plus, None),
      (Int, Some(NodeValue::Literal(3))),
      (Minus, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_reordering_lit() {
  assert_nodes_stream(
    r#"
      fun main() {
        20 + 1 * 2
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Int, Some(NodeValue::Literal(0))),
      (Int, Some(NodeValue::Literal(1))),
      (Int, Some(NodeValue::Literal(2))),
      (Star, None),
      (Plus, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_int_lit() {
  assert_nodes_stream(
    r#"
      fun main() {
        42
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Int, Some(NodeValue::Literal(0))),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_line_comments() {
  assert_nodes_stream(
    r#"
      -- this is a line comment.
      fun main() {
        -- this is another line comment.
      } -- another one line comment.
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(44, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_block_comments() {
  assert_nodes_stream(
    r#"
      -*
        this is a block comment.
      *-
      fun main() {
        -*
          this is a another block comment.
        *-
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(62, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_if_statement() {
  assert_nodes_stream(
    r#"
      fun main() {
        if x > 5 {
          y = 10
        }
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (If, None),
      (Ident, Some(NodeValue::TextRange(31, 1))), // "x"
      (Int, Some(NodeValue::Literal(0))),         // 5
      (Gt, None),
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(49, 1))), // "y"
      (Eq, None),
      (Int, Some(NodeValue::Literal(1))), // 10
      (RBrace, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_if_else_statement() {
  assert_nodes_stream(
    r#"
      fun main() {
        if x {
          a
        } else {
          b
        }
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (If, None),
      (Ident, Some(NodeValue::TextRange(31, 1))), // "x"
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(45, 1))), // "a"
      (RBrace, None),
      (Else, None),
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(74, 1))), // "b"
      (RBrace, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_while_loop() {
  assert_nodes_stream(
    r#"
      fun main() {
        while i < 10 {
          i = i + 1
        }
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (While, None),
      (Ident, Some(NodeValue::TextRange(34, 1))), // "i"
      (Int, Some(NodeValue::Literal(0))),         // 10
      (Lt, None),
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(53, 1))), // "i"
      (Eq, None),
      (Ident, Some(NodeValue::TextRange(57, 1))), // "i"
      (Int, Some(NodeValue::Literal(1))),         // 1
      (Plus, None),
      (RBrace, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_return_statement() {
  assert_nodes_stream(
    r#"
      fun main() {
        return 42;
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Return, None),
      (Int, Some(NodeValue::Literal(0))), // 42
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_return_empty() {
  assert_nodes_stream(
    r#"
      fun main() {
        return;
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Return, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_imu_declaration_with_type() {
  assert_nodes_stream(
    r#"
      fun main() {
        imu x: s32 = 42;
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Imu, None),
      (Ident, Some(NodeValue::TextRange(32, 1))), // "x"
      (S32Type, None),
      (Colon, None),
      (Eq, None),
      (Int, Some(NodeValue::Literal(0))), // 42
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_imu_declaration_with_inference() {
  assert_nodes_stream(
    r#"
      fun main() {
        imu y := 100;
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Imu, None),
      (Ident, Some(NodeValue::TextRange(32, 1))), // "y"
      (ColonEq, None),
      (Int, Some(NodeValue::Literal(0))), // 100
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_mut_declaration() {
  assert_nodes_stream(
    r#"
      fun main() {
        mut count: int = 0;
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Mut, None),
      (Ident, Some(NodeValue::TextRange(32, 5))), // "count"
      (IntType, None),
      (Colon, None),
      (Eq, None),
      (Int, Some(NodeValue::Literal(0))), // 0
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_if_else_if_else() {
  assert_nodes_stream(
    r#"
      fun main() {
        if x > 10 {
          a
        } else if x > 5 {
          b
        } else {
          c
        }
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (If, None),
      (Ident, Some(NodeValue::TextRange(31, 1))), // "x"
      (Int, Some(NodeValue::Literal(0))),         // 10
      (Gt, None),
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(50, 1))), // "a"
      (RBrace, None),
      (Else, None),
      (If, None),
      (Ident, Some(NodeValue::TextRange(70, 1))), // "x"
      (Int, Some(NodeValue::Literal(1))),         // 5
      (Gt, None),
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(88, 1))), // "b"
      (RBrace, None),
      (Else, None),
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(117, 1))), // "c"
      (RBrace, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_nested_if() {
  assert_nodes_stream(
    r#"
      fun main() {
        if x > 0 {
          if y > 0 {
            z = 1
          }
        }
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (If, None),
      (Ident, Some(NodeValue::TextRange(31, 1))), // "x"
      (Int, Some(NodeValue::Literal(0))),         // 0
      (Gt, None),
      (LBrace, None),
      (If, None),
      (Ident, Some(NodeValue::TextRange(52, 1))), // "y"
      (Int, Some(NodeValue::Literal(1))),         // 0
      (Gt, None),
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(72, 1))), // "z"
      (Eq, None),
      (Int, Some(NodeValue::Literal(2))), // 1
      (RBrace, None),
      (RBrace, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_assignment_with_expression() {
  assert_nodes_stream(
    r#"
      fun main() {
        x = y + 1;
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(28, 1))), // "x"
      (Eq, None),
      (Ident, Some(NodeValue::TextRange(32, 1))), // "y"
      (Int, Some(NodeValue::Literal(0))),         // 1
      (Plus, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_array_type() {
  assert_nodes_stream(
    r#"
      fun main() {
        imu arr: []int = x;
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Imu, None),
      (Ident, Some(NodeValue::TextRange(32, 3))), // "arr"
      (LBracket, None),
      (RBracket, None),
      (IntType, None),
      (Colon, None),
      (Eq, None),
      (Ident, Some(NodeValue::TextRange(45, 1))), // "x"
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_array_literal() {
  assert_nodes_stream(
    r#"
      fun main() {
        imu nums := [1, 2, 3];
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Imu, None),
      (Ident, Some(NodeValue::TextRange(32, 4))), // "nums"
      (ColonEq, None),
      (LBracket, None),
      (Int, Some(NodeValue::Literal(0))), // 1
      (Comma, None),
      (Int, Some(NodeValue::Literal(1))), // 2
      (Comma, None),
      (Int, Some(NodeValue::Literal(2))), // 3
      (RBracket, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_array_repetition() {
  assert_nodes_stream(
    r#"
      fun main() {
        mut flags: []bool = [true * 10];
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Mut, None),
      (Ident, Some(NodeValue::TextRange(32, 5))), // "flags"
      (LBracket, None),
      (RBracket, None),
      (BoolType, None),
      (Colon, None),
      (Eq, None),
      (LBracket, None),
      (True, None),
      (Int, Some(NodeValue::Literal(0))), // 10
      (Star, None),
      (RBracket, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_for_loop_exclusive_range() {
  assert_nodes_stream(
    r#"
      fun main() {
        for i := 0..n {
          sum = sum + i
        }
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (For, None),
      (Ident, Some(NodeValue::TextRange(32, 1))), // "i"
      (ColonEq, None),
      (Int, Some(NodeValue::Literal(0))), // 0
      (Ident, Some(NodeValue::TextRange(40, 1))), // "n"
      (DotDot, None),
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(54, 3))), // "sum"
      (Eq, None),
      (Ident, Some(NodeValue::TextRange(60, 3))), // "sum"
      (Ident, Some(NodeValue::TextRange(66, 1))), // "i"
      (Plus, None),
      (RBrace, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_for_loop_inclusive_range() {
  assert_nodes_stream(
    r#"
      fun main() {
        for j := 2..=10 {
          primes[j] = false
        }
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (For, None),
      (Ident, Some(NodeValue::TextRange(32, 1))), // "j"
      (ColonEq, None),
      (Int, Some(NodeValue::Literal(0))), // 2
      (Int, Some(NodeValue::Literal(1))), // 10
      (DotDotEq, None),
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(56, 6))), // "primes"
      (LBracket, None),
      (Ident, Some(NodeValue::TextRange(63, 1))), // "j"
      (RBracket, None),
      (Eq, None),
      (False, None),
      (RBrace, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_for_loop_with_complex_range() {
  assert_nodes_stream(
    r#"
      fun main() {
        for k := i*2..n+1 {
          process(k)
        }
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (For, None),
      (Ident, Some(NodeValue::TextRange(32, 1))), // "k"
      (ColonEq, None),
      (Ident, Some(NodeValue::TextRange(37, 1))), // "i"
      (Int, Some(NodeValue::Literal(0))),         // 2
      (Star, None),
      (Ident, Some(NodeValue::TextRange(42, 1))), // "n"
      (Int, Some(NodeValue::Literal(1))),         // 1
      (Plus, None),
      (DotDot, None),
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(58, 7))), // "process"
      (LParen, None),
      (Ident, Some(NodeValue::TextRange(66, 1))), // "k"
      (RParen, None),
      (RBrace, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_nested_for_loops() {
  assert_nodes_stream(
    r#"
      fun main() {
        for i := 0..m {
          for j := 0..n {
            matrix[i][j] = 0
          }
        }
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (For, None),
      (Ident, Some(NodeValue::TextRange(32, 1))), // "i"
      (ColonEq, None),
      (Int, Some(NodeValue::Literal(0))), // 0
      (Ident, Some(NodeValue::TextRange(40, 1))), // "m"
      (DotDot, None),
      (LBrace, None),
      (For, None),
      (Ident, Some(NodeValue::TextRange(58, 1))), // "j"
      (ColonEq, None),
      (Int, Some(NodeValue::Literal(1))), // 0
      (Ident, Some(NodeValue::TextRange(66, 1))), // "n"
      (DotDot, None),
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(82, 6))), // "matrix"
      (LBracket, None),
      (Ident, Some(NodeValue::TextRange(89, 1))), // "i"
      (RBracket, None),
      (LBracket, None),
      (Ident, Some(NodeValue::TextRange(92, 1))), // "j"
      (RBracket, None),
      (Eq, None),
      (Int, Some(NodeValue::Literal(2))), // 0
      (RBrace, None),
      (RBrace, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_method_call_no_args() {
  assert_nodes_stream(
    r#"
      fun main() {
        result.clear();
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(28, 6))), // "result"
      (Ident, Some(NodeValue::TextRange(35, 5))), // "clear"
      (Dot, None),
      (LParen, None),
      (RParen, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_method_call_with_args() {
  assert_nodes_stream(
    r#"
      fun main() {
        array.push(42);
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(28, 5))), // "array"
      (Ident, Some(NodeValue::TextRange(34, 4))), // "push"
      (Dot, None),
      (LParen, None),
      (Int, Some(NodeValue::Literal(0))), // 42
      (RParen, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_chained_method_calls() {
  assert_nodes_stream(
    r#"
      fun main() {
        text.trim().to_upper();
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(28, 4))), // "text"
      (Ident, Some(NodeValue::TextRange(33, 4))), // "trim"
      (Dot, None),
      (LParen, None),
      (RParen, None),
      (Ident, Some(NodeValue::TextRange(40, 8))), // "to_upper"
      (Dot, None),
      (LParen, None),
      (RParen, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_field_access() {
  assert_nodes_stream(
    r#"
      fun main() {
        x = point.x + point.y;
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(28, 1))), // "x"
      (Eq, None),
      (Ident, Some(NodeValue::TextRange(32, 5))), // "point"
      (Ident, Some(NodeValue::TextRange(38, 1))), // "x"
      (Dot, None),
      (Ident, Some(NodeValue::TextRange(42, 5))), // "point"
      (Ident, Some(NodeValue::TextRange(48, 1))), // "y"
      (Dot, None),
      (Plus, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_simple_field_access() {
  assert_nodes_stream(
    r#"
      fun main() {
        x = point.y;
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(28, 1))), // "x"
      (Eq, None),
      (Ident, Some(NodeValue::TextRange(32, 5))), // "point"
      (Ident, Some(NodeValue::TextRange(38, 1))), // "y"
      (Dot, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_function_with_parameters() {
  assert_nodes_stream(
    r#"
      fun add(x: int, y: int) -> int {
        x + y
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 3))), // "add"
      (LParen, None),
      (Ident, Some(NodeValue::TextRange(15, 1))), // "x"
      (IntType, None),
      (Comma, None),
      (Ident, Some(NodeValue::TextRange(23, 1))), // "y"
      (IntType, None),
      (RParen, None),
      (Arrow, None),
      (IntType, None),
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(48, 1))), // "x"
      (Ident, Some(NodeValue::TextRange(52, 1))), // "y"
      (Plus, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_function_call_with_args() {
  assert_nodes_stream(
    r#"
      fun main() {
        add(10, 20)
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(28, 3))), // "add"
      (LParen, None),
      (Int, Some(NodeValue::Literal(0))), // 10
      (Comma, None),
      (Int, Some(NodeValue::Literal(1))), // 20
      (RParen, None),
      (RBrace, None),
    ],
  );
}

// #[test]
// fn test_member_access_in_expression() {
//   // For a.b + c.d, we want:
//   // a b Dot (evaluates to a.b)
//   // c d Dot (evaluates to c.d)
//   // Plus (adds them)
//   assert_nodes_stream(
//     r#"
//       fun main() {
//         result = a.b + c.d;
//       }
//     "#,
//     &[
//       (Fun, None),
//       (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
//       (LParen, None),
//       (RParen, None),
//       (LBrace, None),
//       (Ident, Some(NodeValue::TextRange(28, 6))), // "result"
//       (Eq, None),
//       (Ident, Some(NodeValue::TextRange(37, 1))), // "a"
//       (Ident, Some(NodeValue::TextRange(39, 1))), // "b"
//       (Dot, None),
//       (Ident, Some(NodeValue::TextRange(43, 1))), // "c"
//       (Ident, Some(NodeValue::TextRange(45, 1))), // "d"
//       (Dot, None),
//       (Plus, None),
//       (Semicolon, None),
//       (RBrace, None),
//     ],
//   );
// }

#[test]
fn test_array_indexing() {
  assert_nodes_stream(
    r#"
      fun main() {
        x = arr[i + 1];
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(28, 1))), // "x"
      (Eq, None),
      (Ident, Some(NodeValue::TextRange(32, 3))), // "arr"
      (LBracket, None),
      (Ident, Some(NodeValue::TextRange(36, 1))), // "i"
      (Int, Some(NodeValue::Literal(0))),         // 1
      (Plus, None),
      (RBracket, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

// === AS CAST ===

#[test]
fn test_as_cast_int_to_char() {
  // `72 as char` → tree: 72, As, CharType (adjacent).
  assert_nodes_stream(
    r#"
      fun main() {
        72 as char;
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))),
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Int, Some(NodeValue::Literal(0))),
      (As, None),
      (CharType, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_as_cast_in_variable_decl() {
  // `imu f: float = 42 as float;`
  assert_nodes_stream(
    r#"
      fun main() {
        imu f: float = 42 as float;
      }
    "#,
    &[
      (Fun, None),                                // 0
      (Ident, Some(NodeValue::TextRange(11, 4))), // 1
      (LParen, None),                             // 2
      (RParen, None),                             // 3
      (LBrace, None),                             // 4
      (Imu, None),                                // 5
      (Ident, Some(NodeValue::TextRange(34, 1))), // 6
      (FloatType, None),                          // 7
      (Colon, None),                              // 8
      (Eq, None),                                 // 9
      (Int, Some(NodeValue::Literal(0))),         // 10
      (As, None),                                 // 11
      (FloatType, None),                          // 12
      (Semicolon, None),                          // 13
      (RBrace, None),                             // 14
    ],
  );
}

// === ABSTRACT ===

#[test]
fn test_abstract_empty() {
  assert_nodes_stream(
    "abstract Show {}",
    &[
      (Abstract, None),
      (Ident, Some(NodeValue::TextRange(9, 4))),
      (LBrace, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_abstract_with_method() {
  // abstract method = fun signature ending with ;
  // Fun inside Abstract closes on Semicolon (ancestor check).
  assert_nodes_stream(
    r#"abstract Show {
  fun display(self) -> str;
}"#,
    &[
      (Abstract, None),
      (Ident, Some(NodeValue::TextRange(9, 4))),
      (LBrace, None),
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(20, 7))),
      (LParen, None),
      (SelfLower, None),
      (RParen, None),
      (Arrow, None),
      (StrType, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_apply_for_tree_order() {
  // `apply Show for Point {}` — For and Point Ident
  // appear after Show Ident (flush_expr before For).
  assert_nodes_stream(
    "apply Show for Point {}",
    &[
      (Apply, None),
      (Ident, Some(NodeValue::TextRange(6, 4))),
      (For, None),
      (Ident, Some(NodeValue::TextRange(15, 5))),
      (LBrace, None),
      (RBrace, None),
    ],
  );
}
