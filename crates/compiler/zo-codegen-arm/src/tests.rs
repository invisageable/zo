pub(crate) mod common;
pub(crate) mod errors;
pub(crate) mod templates;

use crate::ARM64Gen;

use zo_executor::Executor;
use zo_interner::Interner;
use zo_parser::Parser;
use zo_sir::{Insn, Sir};
use zo_tokenizer::Tokenizer;
use zo_ty::TyId;
use zo_value::ValueId;

#[test]
fn test_complete_pipeline_hello_world() {
  let source = r#"fun main() { show("hello world") }"#;

  let tokenizer = Tokenizer::new(source);
  let tokenization = tokenizer.tokenize();

  let parser = Parser::new(&tokenization, source);
  let parsing_result = parser.parse();

  let executor = Executor::new(
    &parsing_result.tree,
    &tokenization.interner,
    &tokenization.literals,
  );

  let (sir, _annotations) = executor.execute();

  assert_eq!(sir.instructions.len(), 4);

  if let Insn::FunDef {
    name,
    params,
    body_start,
    ..
  } = &sir.instructions[0]
  {
    assert_eq!(tokenization.interner.get(*name), "main");
    assert_eq!(params.len(), 0);
    assert_eq!(*body_start, 1);
  } else {
    panic!("Expected FunDef instruction");
  }

  if let Insn::ConstString { symbol, .. } = &sir.instructions[1] {
    assert_eq!(tokenization.interner.get(*symbol), "hello world");
  } else {
    panic!("Expected ConstString instruction");
  }

  if let Insn::Call { name, args, .. } = &sir.instructions[2] {
    assert_eq!(tokenization.interner.get(*name), "show");
    assert_eq!(args.len(), 1);
  } else {
    panic!("Expected Call instruction");
  }

  assert!(matches!(
    sir.instructions[3],
    Insn::Return { value: None, .. }
  ));

  let mut codegen = ARM64Gen::new(&tokenization.interner);
  let artifact = codegen.generate(&sir);

  assert!(!artifact.code.is_empty());
  // Should have at least: MOV X16, MOV X0, ADR X1, MOV X2, SVC (5 instructions
  // * 4 bytes) Plus exit syscall: MOV X16, MOV X0, SVC (3 more instructions)
  // Total: 8 instructions * 4 bytes = 32 bytes minimum
  assert!(artifact.code.len() >= 32);

  let hello_bytes = b"hello world\0";

  let code_contains_string = artifact
    .code
    .windows(hello_bytes.len())
    .any(|window| window == hello_bytes);

  assert!(
    code_contains_string,
    "Generated code should contain the string 'hello world'"
  );
}

#[test]
fn test_main_function_detection() {
  let mut interner = Interner::new();

  let mut sir = Sir::new();

  sir.emit(Insn::FunDef {
    name: interner.intern("main"),
    params: vec![],
    return_ty: TyId(0),
    body_start: 1,
  });
  sir.emit(Insn::Return {
    value: None,
    ty_id: TyId(0),
  });

  let mut codegen = ARM64Gen::new(&interner);
  let artifact = codegen.generate(&sir);

  assert!(!artifact.code.is_empty());
  // assert!(artifact.code.len() >= 12);
}

#[test]
fn test_string_fixup() {
  let mut interner = Interner::new();

  let mut sir = Sir::new();
  let main_sym = interner.intern("main");
  let show_sym = interner.intern("show");
  let hello_sym = interner.intern("hello");
  let world_sym = interner.intern("world");

  sir.emit(Insn::FunDef {
    name: main_sym,
    params: vec![],
    return_ty: TyId(0),
    body_start: 1,
  });

  sir.emit(Insn::ConstString {
    symbol: hello_sym,
    ty_id: TyId(1),
  });
  sir.emit(Insn::Call {
    name: show_sym,
    args: vec![ValueId(0)],
    ty_id: TyId(0),
  });
  sir.emit(Insn::ConstString {
    symbol: world_sym,
    ty_id: TyId(1),
  });
  sir.emit(Insn::Call {
    name: show_sym,
    args: vec![ValueId(1)],
    ty_id: TyId(0),
  });
  sir.emit(Insn::Return {
    value: None,
    ty_id: TyId(0),
  });

  let mut codegen = ARM64Gen::new(&interner);
  let artifact = codegen.generate(&sir);

  let hello_bytes = b"hello\0";
  let world_bytes = b"world\0";

  let code_contains_hello = artifact
    .code
    .windows(hello_bytes.len())
    .any(|window| window == hello_bytes);
  let code_contains_world = artifact
    .code
    .windows(world_bytes.len())
    .any(|window| window == world_bytes);

  assert!(code_contains_hello, "should contain 'hello'");
  assert!(code_contains_world, "should contain 'world'");
}
