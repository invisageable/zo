use crate::Executor;

use zo_interner::Interner;
use zo_parser::Parser;
use zo_sir::Insn;
use zo_tokenizer::Tokenizer;
use zo_ty::Ty;

/// Assert that execution produces the expected type annotations and SIR
pub(crate) fn assert_annotations_stream(
  source: &str,
  expected: &[(usize, Ty, Insn)],
) {
  let tokenizer = Tokenizer::new(source);
  let tokenization = tokenizer.tokenize();

  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let executor = Executor::new(
    &parsing.tree,
    &tokenization.interner,
    &tokenization.literals,
  );
  let (sir, annotations, ty_checker) = executor.execute_with_tychecker();
  let mut actual: Vec<(usize, Ty, Insn)> = Vec::new();

  for (idx, annotation) in annotations.iter().enumerate() {
    let ty = ty_checker.resolve_ty(annotation.ty_id);

    if idx < sir.instructions.len() {
      actual.push((annotation.node_idx, ty, sir.instructions[idx].clone()));
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
  let tokenizer = Tokenizer::new(source);
  let tokenization = tokenizer.tokenize();

  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let executor = Executor::new(
    &parsing.tree,
    &tokenization.interner,
    &tokenization.literals,
  );
  let (sir, _, _) = executor.execute_with_tychecker();

  // eprintln!("Full SIR instructions: {:#?}", sir.instructions);
  assert_eq!(
    sir.instructions, expected,
    "\n\nSIR instructions mismatch.\n\nExpected:\n{:#?}\n\nActual:\n{:#?}\n",
    expected, sir.instructions
  );
}

/// Assert SIR stream structure without caring about exact Symbol/TyId values
pub(crate) fn assert_sir_structure(source: &str, check: impl Fn(&[Insn])) {
  let tokenizer = Tokenizer::new(source);
  let tokenization = tokenizer.tokenize();

  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let executor = Executor::new(
    &parsing.tree,
    &tokenization.interner,
    &tokenization.literals,
  );
  let (sir, _, _) = executor.execute_with_tychecker();

  eprintln!("Full SIR instructions: {:#?}", sir.instructions);
  check(&sir.instructions);
}
