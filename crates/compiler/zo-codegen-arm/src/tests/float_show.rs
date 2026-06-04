//! Bare-float `show` / `showln` lower the argument through
//! the runtime's shortest-round-trip formatter
//! `_zo_float_to_str` (`format!("{f}")`), not an inline
//! fixed-6-decimal `ftoa`. The old inline path dropped the
//! sign onto the fraction, capped at six digits, and never
//! zero-padded — so `-3.14159` printed as `-3.-141589` and
//! `0.001` as `0.1000`.
//!
//! These tests pin the routing: a float argument to a
//! print builtin must register `_zo_float_to_str` in
//! `extern_used`. An integer argument must NOT — it stays
//! on the inline `itoa` path — guarding against the float
//! branch swallowing every numeric print.
//!
//! ```sh
//! cargo test -p zo-codegen-arm float_show
//! ```

use crate::ARM64Gen;

use zo_executor::Executor;
use zo_interner::Interner;
use zo_parser::Parser;
use zo_tokenizer::Tokenizer;
use zo_ty_checker::TyChecker;

const FLOAT_TO_STR: &str = "_zo_float_to_str";

fn compile_and_inspect<F: FnOnce(&[String])>(source: &str, check: F) {
  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();

  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let mut ty_checker = TyChecker::new();

  let executor = Executor::new(
    &parsing.tree,
    &mut interner,
    &tokenization.literals,
    &mut ty_checker,
  );

  let (sir, _, _, _, _, _, _) = executor.execute();

  let mut codegen = ARM64Gen::new(&interner);
  let _artifact = codegen.generate(&sir);

  check(codegen.extern_used());
}

#[test]
fn showln_negative_float_registers_float_to_str_extern() {
  compile_and_inspect(
    r#"
      fun main() {
        showln(-3.14159);
      }
    "#,
    |externs| {
      assert!(
        externs.iter().any(|s| s == FLOAT_TO_STR),
        "expected `{FLOAT_TO_STR}` in extern_used, got {externs:?}"
      );
    },
  );
}

#[test]
fn show_float_registers_float_to_str_extern() {
  compile_and_inspect(
    r#"
      fun main() {
        show(0.001);
      }
    "#,
    |externs| {
      assert!(
        externs.iter().any(|s| s == FLOAT_TO_STR),
        "expected `{FLOAT_TO_STR}` in extern_used, got {externs:?}"
      );
    },
  );
}

#[test]
fn eshowln_float_registers_float_to_str_extern() {
  compile_and_inspect(
    r#"
      fun main() {
        eshowln(-0.169075164);
      }
    "#,
    |externs| {
      assert!(
        externs.iter().any(|s| s == FLOAT_TO_STR),
        "expected `{FLOAT_TO_STR}` in extern_used, got {externs:?}"
      );
    },
  );
}

#[test]
fn showln_integer_does_not_register_float_to_str_extern() {
  compile_and_inspect(
    r#"
      fun main() {
        showln(42);
      }
    "#,
    |externs| {
      assert!(
        !externs.iter().any(|s| s == FLOAT_TO_STR),
        "integer print must not route through `{FLOAT_TO_STR}`, \
         got {externs:?}"
      );
    },
  );
}
