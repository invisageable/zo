//! Shared test helpers for the analyzer crate.
//!
//! Drives the real `Source → Tokens → Tree → SIR` pipeline so
//! tests see the exact SIR the backend would — no hand-crafted
//! insn streams (those live in `zo-sir` unit tests). Returns
//! both the [`SemanticResult`] and a [`ValidationReport`] so
//! callers can assert on both semantic output and SIR type-
//! invariant violations.

use crate::{Analyzer, SemanticResult};

use zo_interner::Interner;
use zo_parser::Parser;
use zo_sir::{ValidationReport, validate};
use zo_tokenizer::Tokenizer;
use zo_ty_checker::TyChecker;

/// Runs the full analyzer pipeline on `source` and returns the
/// resulting [`SemanticResult`] alongside the
/// [`ValidationReport`] from the SIR validator.
pub(crate) fn analyze_and_validate(
  source: &str,
) -> (SemanticResult, ValidationReport) {
  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();

  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let mut ty_checker = TyChecker::new();

  let analyzer = Analyzer::new(
    &parsing.tree,
    &mut interner,
    &tokenization.literals,
    &mut ty_checker,
  );

  let semantic = analyzer.analyze();
  let report = validate(&semantic.sir.instructions);

  (semantic, report)
}
