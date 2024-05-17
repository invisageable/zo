// use cranelift::frontend::{FunctionBuilder, FunctionBuilderContext};
// use cranelift_codegen::ir::{types, AbiParam, InstBuilder};
// use cranelift_codegen::settings::Flags;
// use cranelift_codegen::{settings, settings::Configurable};
// use cranelift_module::{Linkage, Module};
// use cranelift_object::{ObjectBuilder, ObjectModule};

// fn main() -> Result<(), anyhow::Error> {
//   let verbose = true;

//   let mut settings_builder = settings::builder();

//   settings_builder.set("is_pic", "true").unwrap();

//   let isa_builder = cranelift_native::builder().unwrap();
//   let isa = isa_builder.finish(Flags::new(settings_builder)).unwrap();
//   let libcall_names = cranelift_module::default_libcall_names();

//   let object_builder =
//     ObjectBuilder::new(isa.clone(), "zo", libcall_names).unwrap();

//   let mut module = ObjectModule::new(object_builder);
//   let mut context = module.make_context();
//   let mut func_ctx = FunctionBuilderContext::new();

//   let function_name = "main";
//   let inputs: Vec<usize> = Vec::with_capacity(0usize);

//   let mut signature = module.make_signature();
//   let _ty = module.target_config().pointer_type();

//   for _input in &inputs {
//     signature.params.push(AbiParam::new(types::F64));
//   }

//   signature.params.push(AbiParam::new(types::I64));
//   signature.returns.push(AbiParam::new(types::F64));

//   let func_id = module
//     .declare_function(function_name, Linkage::Export, &signature)
//     .unwrap();

//   let mut builder = FunctionBuilder::new(&mut context.func, &mut func_ctx);

//   let entry_block = builder.create_block();

//   builder.append_block_params_for_function_params(entry_block);
//   builder.switch_to_block(entry_block);
//   builder.seal_block(entry_block);

//   builder.ins().return_(&[]);
//   builder.finalize();

//   module.define_function(func_id, &mut context).unwrap();
//   module.clear_context(&mut context);

//   let bytes = module.finish().emit().unwrap();

//   std::fs::write("./program/main.o", bytes).unwrap();

//   #[cfg(target_os = "macos")]
//   std::process::Command::new("gcc")
//     .args(&[
//       if verbose { "-v" } else { "" },
//       "-o",
//       "./program/main",
//       "./program/main.o",
//       "-Xlinker",
//       "-ld_classic",
//     ])
//     .status()
//     .unwrap();

//   Ok(())
// }

use zo_driver::driver;

fn main() {
  driver::main();
}
