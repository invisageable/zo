use super::translator::Translator;

use zhoo_ast::ast;
use zhoo_session::session::Session;

use zo_core::Result;

// use hashbrown::HashMap;
// use inkwell::builder::Builder;
// use inkwell::context::Context;
// use inkwell::module::Module;
// use inkwell::passes::PassManager;
// use inkwell::values::{FunctionValue, PointerValue};

// #[derive(Debug)]
// struct Codegen<'program> {
//   context: &'program Context,
//   module: Module<'program>,
//   builder: Builder<'program>,
//   variables: HashMap<String, PointerValue<'program>>,
//   pass_manager: Option<PassManager<FunctionValue<'program>>>,
// }

#[derive(Debug)]
struct Codegen;

impl Codegen {
  fn generate(
    &mut self,
    session: &mut Session,
    program: &ast::Program,
  ) -> Result<Box<[u8]>> {
    let mut translator = Translator::new(&session.interner, &session.reporter);

    translator.translate(program)?;
    translator.output()
  }
}

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn generate(
  session: &mut Session,
  program: &ast::Program,
) -> Result<Box<[u8]>> {
  // let context = Context::create();
  // let module = context.create_module("zhoo_main");
  // let builder = context.create_builder();

  // let mut codegen = Codegen {
  //   context: &context,
  //   module,
  //   builder,
  //   variables: HashMap::with_capacity(0usize),
  //   pass_manager: None,
  // };

  Codegen.generate(session, program)
}
