//! Phase 2 of `PLAN_CHANNELS.md` — parser support for
//! `nursery { ... }`, `spawn fn(args)`, and `await expr`.
//!
//! Tree-shape contracts asserted here:
//!
//! - `Nursery` is an introducer; its body is the following
//!   `{` block; it auto-closes when the block's `}` closes.
//! - `Spawn` is an introducer; its body is the following
//!   call expression; it closes on `;`.
//! - `Await` is a prefix-unary; it lands postfix after the
//!   operand it applies to (same mechanism as `!`).
//!
//! ```sh
//! cargo test -p zo-parser concurrency
//! ```

use crate::tests::common::assert_nodes_stream;

use zo_token::Token::{
  Await, ColonEq, Fun, Ident, Imu, LBrace, LParen, Nursery, RBrace, RParen,
  Semicolon, Spawn, Thread,
};
use zo_tree::NodeValue;

#[test]
fn nursery_empty_block_auto_closes() {
  assert_nodes_stream(
    r#"
      fun main() {
        nursery {}
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Nursery, None),
      (LBrace, None),
      (RBrace, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn nursery_with_spawn_body() {
  assert_nodes_stream(
    r#"
      fun main() {
        nursery {
          spawn foo();
        }
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Nursery, None),
      (LBrace, None),
      (Spawn, None),
      (Ident, Some(NodeValue::TextRange(0, 3))), // "foo"
      (LParen, None),
      (RParen, None),
      (Semicolon, None),
      (RBrace, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn spawn_call_closes_on_semicolon() {
  // `spawn` followed by a call, terminated by `;`. The
  // Spawn introducer must close at the `;` (same cascade as
  // Return) so the enclosing block's sequence is flat.
  assert_nodes_stream(
    r#"
      fun main() {
        nursery {
          spawn compute(42);
        }
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Nursery, None),
      (LBrace, None),
      (Spawn, None),
      (Ident, Some(NodeValue::TextRange(0, 7))), // "compute"
      (LParen, None),
      (zo_token::Token::Int, Some(NodeValue::Literal(0))), // 42
      (RParen, None),
      (Semicolon, None),
      (RBrace, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn await_ident_emits_postfix_unary() {
  // `await task` with `task` a bare ident. The unary-stash
  // machinery drains Await right after the operand, so the
  // tree reads `task Await` in postorder.
  assert_nodes_stream(
    r#"
      fun main() {
        imu x := await task;
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Imu, None),
      (Ident, Some(NodeValue::TextRange(0, 1))), // "x"
      (ColonEq, None),
      (Ident, Some(NodeValue::TextRange(0, 4))), // "task"
      (Await, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn await_inside_function_arg() {
  // Mid-expression `await`: `showln(await t)` must stash
  // Await until the operand `t` lands inside the call
  // args, drain it postfix, and still close the call at
  // the matching `)`.
  assert_nodes_stream(
    r#"
      fun main() {
        showln(await t);
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(0, 6))), // "showln"
      (LParen, None),
      (Ident, Some(NodeValue::TextRange(0, 1))), // "t"
      (Await, None),
      (RParen, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn nursery_multiple_spawns_flat() {
  // Two `spawn` statements at the same nursery level — the
  // Spawn introducers must each close on their own `;` so
  // the second `spawn` sees a clean stack.
  assert_nodes_stream(
    r#"
      fun main() {
        nursery {
          spawn a();
          spawn b();
        }
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Nursery, None),
      (LBrace, None),
      (Spawn, None),
      (Ident, Some(NodeValue::TextRange(0, 1))), // "a"
      (LParen, None),
      (RParen, None),
      (Semicolon, None),
      (Spawn, None),
      (Ident, Some(NodeValue::TextRange(0, 1))), // "b"
      (LParen, None),
      (RParen, None),
      (Semicolon, None),
      (RBrace, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn spawn_thread_emits_thread_marker() {
  // `spawn thread worker()` lowers to `Spawn Thread
  // Ident(worker) LParen RParen Semicolon`. The
  // `Thread` marker is synthetic — emitted by the
  // parser after it recognizes the contextual
  // modifier; the tokenizer still lexes "thread" as
  // a regular `Ident`, so user code remains free to
  // use it as an identifier elsewhere.
  assert_nodes_stream(
    r#"
      fun main() {
        nursery {
          spawn thread worker();
        }
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Nursery, None),
      (LBrace, None),
      (Spawn, None),
      (Thread, None),
      (Ident, Some(NodeValue::TextRange(0, 6))), // "worker"
      (LParen, None),
      (RParen, None),
      (Semicolon, None),
      (RBrace, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn plain_spawn_does_not_emit_thread_marker() {
  // Sanity: make sure the contextual recognition
  // doesn't false-positive on `spawn foo()` — no
  // Thread marker should appear between Spawn and
  // the callee Ident.
  assert_nodes_stream(
    r#"
      fun main() {
        nursery {
          spawn thread();
        }
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Nursery, None),
      (LBrace, None),
      (Spawn, None),
      (Ident, Some(NodeValue::TextRange(0, 6))), // "thread" (the callee)
      (LParen, None),
      (RParen, None),
      (Semicolon, None),
      (RBrace, None),
      (RBrace, None),
    ],
  );
}
