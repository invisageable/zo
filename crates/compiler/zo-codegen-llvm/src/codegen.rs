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

    let _value = translator.translate(ast)?;

    // let entry = engine
    //   .get_function_value("main")
    //   .map_err(error::generate::llvm::engine_execution)?;

    // println!("Entry = {entry:?}");

    let ir = translator.module.print_to_string();

    println!("Content = {ir}");

    Ok(vec![].into_boxed_slice())
  }
}

/// Runs passess
fn run_passes_on(module: &Module) {
  let fpm = PassManager::create(());

  // todo(ivs) — some passes does not work on my architecture.
  // I'm disabling all of them for now. I will investigate next time to
  // understand what pass work depending of the architure. A better system
  // should be implemented to apply only available pass.
  //
  // ```
  // = note: ld: warning: ignoring duplicate libraries: '-lm'
  // ld: warning: object file
  // (/zo/target/debug/deps/libllvm_sys-d2b6752da5d0d329.
  // rlib[4](c9bd1b4e4b84f211-target.o)) was built for newer 'macOS' version
  // (14.5) than being linked (14.0) Undefined symbols for architecture arm64:
  // "_LLVMAddBasicAliasAnalysisPass", referenced from:
  //       inkwell::passes::PassManager$LT$T$GT$::add_basic_alias_analysis_pass::hde9b9a2c5b473a80 in libzo_codegen_llvm-93cae620b08aa237.rlib[21](zo_codegen_llvm-93cae620b08aa237.bbl2u418h3i5dx4rf4aknywye.rcgu.o)
  //   "_LLVMAddCFGSimplificationPass", referenced from:
  //       inkwell::passes::PassManager$LT$T$GT$::add_cfg_simplification_pass::h8692f04e1d7929a5 in libzo_codegen_llvm-93cae620b08aa237.rlib[21](zo_codegen_llvm-93cae620b08aa237.bbl2u418h3i5dx4rf4aknywye.rcgu.o)
  //   "_LLVMAddGVNPass", referenced from:
  //       inkwell::passes::PassManager$LT$T$GT$::add_gvn_pass::hf05d5e6d651f5561
  // in libzo_codegen_llvm-93cae620b08aa237.
  // rlib[21](zo_codegen_llvm-93cae620b08aa237.bbl2u418h3i5dx4rf4aknywye.rcgu.o)
  //   "_LLVMAddInstructionCombiningPass", referenced from:
  //       inkwell::passes::PassManager$LT$T$GT$::add_instruction_combining_pass::h86de0e507913d6ce in libzo_codegen_llvm-93cae620b08aa237.rlib[21](zo_codegen_llvm-93cae620b08aa237.bbl2u418h3i5dx4rf4aknywye.rcgu.o)
  //   "_LLVMAddPromoteMemoryToRegisterPass", referenced from:
  //       inkwell::passes::PassManager$LT$T$GT$::add_promote_memory_to_register_pass::h4f6593edab74d966 in libzo_codegen_llvm-93cae620b08aa237.rlib[21](zo_codegen_llvm-93cae620b08aa237.bbl2u418h3i5dx4rf4aknywye.rcgu.o)
  //   "_LLVMAddReassociatePass", referenced from:
  //       inkwell::passes::PassManager$LT$T$GT$::add_reassociate_pass::h71ea9107776bb6c6 in libzo_codegen_llvm-93cae620b08aa237.rlib[21](zo_codegen_llvm-93cae620b08aa237.bbl2u418h3i5dx4rf4aknywye.rcgu.o)
  // ld: symbol(s) not found for architecture arm64
  // clang: error: linker command failed with exit code 1 (use -v to see
  // invocation)
  // ```

  // fpm.add_instruction_combining_pass();
  // fpm.add_reassociate_pass();
  // fpm.add_gvn_pass();
  // fpm.add_cfg_simplification_pass();
  // fpm.add_basic_alias_analysis_pass();
  // fpm.add_promote_memory_to_register_pass();

  fpm.run_on(module);
}

/// Transforms an AST into bytecode — see also [`Codegen::generate`].
#[inline]
pub fn generate(session: &mut Session, ast: &Ast) -> Result<Box<[u8]>> {
  Codegen.generate(session, ast)
}
