//! ```sh
//! cargo test -p zo-executor --lib tests::cross_module_components
//! ```
//!
//! Cross-pack components, end to end at the executor level: module
//! B's executor records its `pub fun … -> </>` body fragments; the
//! importer splices them into its own tree (the same pre-pass
//! generics ride) and registers the ranges, so `<header />` in
//! module A instantiates a component defined in module B.

use crate::Executor;

use zo_interner::Interner;
use zo_module_resolver::{ImportedSymbols, splice_component_bodies};
use zo_parser::Parser;
use zo_sir::Insn;
use zo_tokenizer::Tokenizer;
use zo_ty_checker::TyChecker;

#[test]
fn imported_component_instantiates_in_tag_position() {
  // One interner across modules — symbols stay portable, exactly
  // as the compiler driver shares `session.interner`.
  let mut interner = Interner::new();
  let mut ty_checker = TyChecker::new();

  // --- module B: defines the components. ---
  let widgets_src = r#"
pub fun header() -> </> {
  return <p>from widgets</p>;
}

pub fun greeting(name: str) -> </> {
  return <h1>hello, {name}!</h1>;
}

fun main() {}
"#;

  let tokenizer = Tokenizer::new(widgets_src, &mut interner);
  let widgets_tok = tokenizer.tokenize();
  let parser = Parser::new(&widgets_tok, widgets_src);
  let widgets_par = parser.parse();

  let executor = Executor::new(
    &widgets_par.tree,
    &mut interner,
    &widgets_tok.literals,
    &mut ty_checker,
  );

  let widgets_out = executor.execute();
  let (funs, component_bodies) =
    (widgets_out.funs, widgets_out.component_bodies);

  assert_eq!(
    component_bodies.len(),
    2,
    "both pub components must export their fragments"
  );

  // --- module A: imports and uses them. ---
  let main_src = r#"
fun main() {
  imu page ::= <div>
    <header />
    <greeting name="zo" />
  </div>;

  #render page;
}
"#;

  let tokenizer = Tokenizer::new(main_src, &mut interner);
  let main_tok = tokenizer.tokenize();
  let parser = Parser::new(&main_tok, main_src);
  let mut main_par = parser.parse();
  let mut main_literals = main_tok.literals;

  // The compiler pre-pass: splice B's fragments into A's tree.
  let spliced = splice_component_bodies(
    &mut main_par.tree,
    &mut main_literals,
    component_bodies,
  );

  let imports = ImportedSymbols {
    funs,
    component_bodies: spliced,
    ..ImportedSymbols::default()
  };

  let mut ty_checker = TyChecker::new();
  let executor = Executor::new(
    &main_par.tree,
    &mut interner,
    &main_literals,
    &mut ty_checker,
  )
  .with_imports(imports);

  let sir = executor.execute().sir;

  let page = sir
    .instructions
    .iter()
    .filter_map(|i| match i {
      Insn::Template { commands, .. } => Some(commands),
      _ => None,
    })
    .next_back()
    .expect("page template");

  use zo_ui_protocol::UiCommand;

  let texts: Vec<&str> = page
    .iter()
    .filter_map(|c| match c {
      UiCommand::Text(t) => Some(t.as_str()),
      _ => None,
    })
    .collect();

  assert!(
    texts.iter().any(|t| t.contains("from widgets")),
    "imported zero-prop component missing: {texts:?}"
  );
  assert!(
    texts.iter().any(|t| t.contains("hello, zo")),
    "imported parametrized component missing: {texts:?}"
  );
}

#[test]
fn imported_mutual_recursion_reports_circular_component() {
  // In B's own compilation, `<pong />` inside `ping` falls through
  // (registration order), but the EXPORTED fragments re-execute in
  // the importer where both are registered — without the
  // instantiation stack this expands forever at compile time.
  use zo_error::ErrorKind;
  use zo_reporter::{clear_errors, collect_errors};

  clear_errors();

  let mut interner = Interner::new();
  let mut ty_checker = TyChecker::new();

  let widgets_src = r#"
pub fun ping() -> </> {
  return <div><pong /></div>;
}

pub fun pong() -> </> {
  return <div><ping /></div>;
}

fun main() {}
"#;

  let tokenizer = Tokenizer::new(widgets_src, &mut interner);
  let widgets_tok = tokenizer.tokenize();
  let parser = Parser::new(&widgets_tok, widgets_src);
  let widgets_par = parser.parse();

  let executor = Executor::new(
    &widgets_par.tree,
    &mut interner,
    &widgets_tok.literals,
    &mut ty_checker,
  );

  let widgets_out = executor.execute();

  let main_src = r#"
fun main() {
  imu page ::= <div><ping /></div>;

  #render page;
}
"#;

  let tokenizer = Tokenizer::new(main_src, &mut interner);
  let main_tok = tokenizer.tokenize();
  let parser = Parser::new(&main_tok, main_src);
  let mut main_par = parser.parse();
  let mut main_literals = main_tok.literals;

  let spliced = splice_component_bodies(
    &mut main_par.tree,
    &mut main_literals,
    widgets_out.component_bodies,
  );

  let imports = ImportedSymbols {
    funs: widgets_out.funs,
    component_bodies: spliced,
    ..ImportedSymbols::default()
  };

  let mut ty_checker = TyChecker::new();
  let executor = Executor::new(
    &main_par.tree,
    &mut interner,
    &main_literals,
    &mut ty_checker,
  )
  .with_imports(imports);

  // The execution must COMPLETE (no hang) and carry the cycle
  // diagnostic.
  executor.execute();

  let errors = collect_errors();

  assert!(
    errors
      .iter()
      .any(|e| e.kind() == ErrorKind::CircularComponent),
    "expected CircularComponent, got: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}
