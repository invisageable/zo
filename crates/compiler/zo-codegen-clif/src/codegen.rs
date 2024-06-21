//! ...

// todo #1 — at the moment, i'm not able to generate an executable using
// cranelift. the lack of understandable documentation is quite demotivating.
// @see — https://github.com/bytecodealliance/wasmtime/blob/main/cranelift/docs/index.md.

use super::translator::Translator;

use zo_ast::ast::Ast;
use zo_session::session::Session;

use zo_core::Result;

use cranelift::frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_codegen::settings::{Configurable, Flags};
use cranelift_codegen::{settings, Context};
use cranelift_module::Module;
use cranelift_object::{ObjectBuilder, ObjectModule};
use hashbrown::HashMap;

// todo #1.

struct Codegen {
  module: ObjectModule,
  context: Context,
  func_ctx: FunctionBuilderContext,
}

impl Codegen {
  pub fn new() -> Self {
    let mut flag_builder = settings::builder();

    flag_builder
      .set("opt_level", "speed_and_size")
      .expect("set optlevel");

    let isa_builder = cranelift_native::builder().unwrap(); // err: architecture unsupported.
    let isa = isa_builder.finish(Flags::new(flag_builder)).unwrap(); // err: unknown target triple.
    let libcall_names = cranelift_module::default_libcall_names();

    let object_builder =
      ObjectBuilder::new(isa, String::from("zo"), libcall_names).unwrap(); // err: wrong backend.

    let module = ObjectModule::new(object_builder);
    let context = module.make_context();
    let func_ctx = FunctionBuilderContext::new();

    Self {
      module,
      context,
      func_ctx,
    }
  }

  fn output(self) -> Result<Box<[u8]>> {
    let object = self.module.finish();

    match object.emit() {
      Ok(bytes) => Ok(bytes.into()),
      Err(error) => panic!("{error}"),
    }
  }

  fn generate(mut self, session: &mut Session, ast: &Ast) -> Result<Box<[u8]>> {
    let builder =
      FunctionBuilder::new(&mut self.context.func, &mut self.func_ctx);

    let translator = Translator {
      interner: &mut session.interner,
      reporter: &mut session.reporter,
      module: &mut self.module,
      builder,
      functions: HashMap::with_capacity(0usize),
    };

    translator.translate(ast)?;
    self.output()
  }
}

/// ...
///
/// ## examples.
///
/// ```rs
/// ```
pub fn generate(session: &mut Session, ast: &Ast) -> Result<Box<[u8]>> {
  Codegen::new().generate(session, ast)
}
