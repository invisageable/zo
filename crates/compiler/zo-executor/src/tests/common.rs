use crate::Executor;

use zo_error::ErrorKind;
use zo_interner::Interner;
use zo_parser::Parser;
use zo_reporter::collect_errors;
use zo_sir::Insn;
use zo_tokenizer::Tokenizer;
use zo_ty::{Annotation, Ty};

/// Assert that execution produces the expected type annotations and SIR
pub(crate) fn assert_annotations_stream(
  source: &str,
  expected: &[(usize, Ty, Insn)],
) {
  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();

  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let executor =
    Executor::new(&parsing.tree, &mut interner, &tokenization.literals);

  let (sir, annotations, ty_checker, _) = executor.execute();
  let mut actual = Vec::new();

  // Zip annotations with non-Nop SIR instructions.
  let live_insns: Vec<_> = sir
    .instructions
    .iter()
    .filter(|i| !matches!(i, Insn::Nop))
    .collect();

  for (idx, annotation) in annotations.iter().enumerate() {
    let ty = ty_checker.resolve_ty(annotation.ty_id);

    if idx < live_insns.len() {
      actual.push((annotation.node_idx, ty, live_insns[idx].clone()));
    }
  }

  assert_eq!(
    actual, expected,
    "\n\nExecution results mismatch.\n\nExpected:\n{:#?}\n\nActual:\n{:#?}\n",
    expected, actual
  );
}

/// Assert that execution produces the expected SIR instructions
pub(crate) fn assert_sir_stream(source: &str, expected: &[Insn]) {
  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();

  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let executor =
    Executor::new(&parsing.tree, &mut interner, &tokenization.literals);

  let (sir, _, _, _) = executor.execute();

  assert_eq!(
    sir.instructions, expected,
    "\n\nSIR instructions mismatch.\n\nExpected:\n{:#?}\n\nActual:\n{:#?}\n",
    expected, sir.instructions
  );
}

/// Assert SIR structure via a predicate.
pub(crate) fn assert_sir_structure(source: &str, check: impl Fn(&[Insn])) {
  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();

  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let executor =
    Executor::new(&parsing.tree, &mut interner, &tokenization.literals);

  let (sir, _, _, _) = executor.execute();

  check(&sir.instructions);
}

/// Assert that execution produces NO errors.
pub(crate) fn assert_no_errors(source: &str) {
  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();

  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let executor =
    Executor::new(&parsing.tree, &mut interner, &tokenization.literals);

  let _ = executor.execute();

  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "Expected no errors, but got: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

/// Assert that execution produces the expected error.
pub(crate) fn assert_execution_error(source: &str, expected_error: ErrorKind) {
  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();

  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let executor =
    Executor::new(&parsing.tree, &mut interner, &tokenization.literals);

  let _ = executor.execute();

  let errors = collect_errors();

  assert!(
    errors.iter().any(|e| e.kind() == expected_error),
    "Expected error {:?}, but got: {:?}",
    expected_error,
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

/// Execute source and return raw (SIR instructions, annotations).
pub(crate) fn execute_raw(source: &str) -> (Vec<Insn>, Vec<Annotation>) {
  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();

  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let executor =
    Executor::new(&parsing.tree, &mut interner, &tokenization.literals);

  let (sir, annotations, _, _) = executor.execute();

  (sir.instructions, annotations)
}
