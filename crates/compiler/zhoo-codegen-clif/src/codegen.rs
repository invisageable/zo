#![allow(dead_code)]

use super::translator::Translator;

use zhoo_ast::ast;
use zhoo_session::session::Session;

use zo_core::Result;

use cranelift::frontend::FunctionBuilderContext;
use cranelift_codegen::settings::Configurable;
use cranelift_codegen::settings::Flags;
use cranelift_codegen::{settings, Context};
use cranelift_module::Module;
use cranelift_native::builder;
use cranelift_object::{ObjectBuilder, ObjectModule};

const ENTRY_NAME: &str = "main";

pub struct Codegen {
  module: ObjectModule,
  context: Context,
  function_builder_context: FunctionBuilderContext,
  // builder: FunctionBuilder,
}

impl Codegen {
  fn new() -> Self {
    let mut flag_builder = settings::builder();

    flag_builder
      .set("opt_level", "speed_and_size")
      .expect("set optlevel");

    let isa_builder = builder().unwrap();
    let isa = isa_builder.finish(Flags::new(flag_builder)).unwrap();

    let object_builder = ObjectBuilder::new(
      isa,
      String::from(ENTRY_NAME),
      cranelift_module::default_libcall_names(),
    )
    .unwrap();

    let module = ObjectModule::new(object_builder);
    let context = module.make_context();
    let function_builder_context = FunctionBuilderContext::new();

    Self {
      module,
      context,
      function_builder_context,
      // builder,
    }
  }

  fn generate(
    mut self,
    session: &mut Session,
    program: &ast::Program,
  ) -> Result<Box<[u8]>> {
    let mut translator = Translator::new(
      &session.interner,
      &session.reporter,
      &mut self.module,
      &mut self.context,
      // &mut self.function_builder_context,
    );

    translator.translate(program)?;

    self.output()
  }

  fn output(self) -> Result<Box<[u8]>> {
    let object = self.module.finish();

    match object.emit() {
      Ok(bytes) => Ok(bytes.into()),
      Err(_) => panic!(),
    }
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
  Codegen::new().generate(session, program)
}
