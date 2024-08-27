use super::translator::Translator;

use zo_ast::ast::Ast;
use zo_reporter::{error, Result};
use zo_session::session::Session;

use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::passes::PassManager;

/// The representation of `llvm` code generation.
struct Codegen;

impl Codegen {
  /// Transforms an AST into bytecode.
  fn generate(&self, session: &mut Session, ast: &Ast) -> Result<Box<[u8]>> {
    let ctx = Context::create();
    let module = ctx.create_module("zo");
    let builder = ctx.create_builder();

    run_passes_on(&module);

    let engine = module
      .create_jit_execution_engine(inkwell::OptimizationLevel::None)
      .map_err(error::generate::llvm::engine)?;

    let mut translator = Translator {
      ctx: &ctx,
      builder: &builder,
      module: &module,
      interner: &mut session.interner,
      reporter: &mut session.reporter,
    };

    let _value = translator.translate(ast)?;

    // let entry = engine
    //   .get_function_value("main")
    //   .map_err(error::generate::llvm::engine_execution)?;

    // println!("Entry = {entry:?}");

    let ir = translator.module.print_to_string();

    println!("Content = {ir}");

    Ok(ir.to_bytes().into())
  }
}

/// Runs passess
fn run_passes_on(module: &Module) {
  let fpm = PassManager::create(());

  fpm.add_instruction_combining_pass();
  fpm.add_reassociate_pass();
  fpm.add_gvn_pass();
  fpm.add_cfg_simplification_pass();
  fpm.add_basic_alias_analysis_pass();
  fpm.add_promote_memory_to_register_pass();

  fpm.run_on(module);
}

/// Transforms an AST into bytecode — see also [`Codegen::generate`].
#[inline]
pub fn generate(session: &mut Session, ast: &Ast) -> Result<Box<[u8]>> {
  Codegen.generate(session, ast)
}
