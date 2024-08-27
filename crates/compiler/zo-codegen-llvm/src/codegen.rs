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

    let _engine = module
      .create_execution_engine()
      .map_err(error::generate::llvm::engine)?;

    let mut translator = Translator {
      ctx: &ctx,
      builder: &builder,
      module: &module,
      interner: &mut session.interner,
      reporter: &mut session.reporter,
    };

    translator.translate(ast)?;

    Ok(vec![].into_boxed_slice())
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
