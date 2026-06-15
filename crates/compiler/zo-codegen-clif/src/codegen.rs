//! The Cranelift backend entry point.
//!
//! Owns the `ObjectModule`, the ISA derived from the `Target`,
//! and the per-function state (FuncId map, block map, value
//! map, stack-slot map). One `CliftGen` per build invocation.

use crate::translate;

use zo_codegen_backend::{Artifact, Backend, Target};
use zo_interner::Interner;
use zo_sir::Sir;

use cranelift::codegen::settings::{self, Configurable};
use cranelift_object::{ObjectBuilder, ObjectModule};
use target_lexicon::Triple;

/// Cranelift-backed code generator.
pub struct CliftGen<'a> {
  target: Target,
  #[allow(dead_code)]
  interner: &'a Interner,
}

impl<'a> CliftGen<'a> {
  /// Creates a new [`CliftGen`] for the given target.
  pub const fn new(interner: &'a Interner, target: Target) -> Self {
    Self { target, interner }
  }

  /// Maps a [`Target`] to a `target_lexicon::Triple`. Only the
  /// four non-ARM-native targets route here; the unreachable
  /// arm catches any accidental dispatch of an ARM-native or
  /// wasm target to this backend.
  fn triple(target: Target) -> Triple {
    match target {
      Target::X8664AppleDarwin => "x86_64-apple-darwin".parse().unwrap(),
      Target::X8664UnknownLinuxGnu => {
        "x86_64-unknown-linux-gnu".parse().unwrap()
      }
      Target::X8664PcWindowsMsvc => "x86_64-pc-windows-msvc".parse().unwrap(),
      Target::Arm64PcWindowsMsvc => "aarch64-pc-windows-msvc".parse().unwrap(),
      Target::Aarch64LinuxAndroid => "aarch64-linux-android".parse().unwrap(),
      Target::Arm64AppleDarwin
      | Target::Arm64UnknownLinuxGnu
      | Target::Arm64AppleIos
      | Target::Arm64AppleIosSim
      | Target::Arm64AppleWatchOsSim
      | Target::Wasm32UnknownUnknown
      | Target::Web => {
        unreachable!("{target:?} not routed through CliftGen");
      }
    }
  }

  /// Builds an `ObjectModule` ready to accept CLIF functions.
  fn new_module(target: Target) -> ObjectModule {
    let triple = Self::triple(target);
    let isa_builder = cranelift::codegen::isa::lookup(triple)
      .expect("unsupported ISA for cranelift backend");

    let mut flag_builder = settings::builder();
    flag_builder.set("use_colocated_libcalls", "false").unwrap();
    flag_builder.set("is_pic", "true").unwrap();

    let flags = settings::Flags::new(flag_builder);
    let isa = isa_builder
      .finish(flags)
      .expect("cranelift ISA finalization failed");

    let builder = ObjectBuilder::new(
      isa,
      "zo".to_string(), // TODO: this should come from name field from `fret.zo`.
      cranelift_module::default_libcall_names(),
    )
    .expect("object builder init failed");

    ObjectModule::new(builder)
  }
}

impl<'a> CliftGen<'a> {
  /// Translates the SIR and returns the concatenated CLIF IR
  /// text (per cranelift's `ir::Function` Display impl). The
  /// ARM path returns disassembled ARM64; the CLIF path
  /// returns pre-codegen IR — equivalently debuggable for
  /// the backend's own decision-making, without pulling a
  /// disassembler dep. Fresh `ObjectModule` each call because
  /// `translate_module` mutates; the resulting object bytes
  /// are discarded.
  pub fn generate_asm(&mut self, sir: &Sir) -> String {
    let mut module = Self::new_module(self.target);

    translate::translate_module(
      &mut module,
      self.interner,
      &sir.instructions,
      &sir.int_bases,
    )
  }
}

impl<'a> Backend for CliftGen<'a> {
  /// Translates the SIR instruction stream — one CLIF function
  /// per `Insn::FunDef` — into the module, then emits the
  /// resulting relocatable object bytes.
  fn generate(&mut self, sir: &Sir) -> Artifact {
    let mut module = Self::new_module(self.target);

    // `translate_module` returns CLIF IR text for the
    // `--emit asm` path; the binary path doesn't need it.
    let _ = translate::translate_module(
      &mut module,
      self.interner,
      &sir.instructions,
      &sir.int_bases,
    );

    let product = module.finish();
    let code = product.emit().expect("object emit failed");

    Artifact { code }
  }
}
