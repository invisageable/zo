use zo_codegen_arm::ARM64Gen;
use zo_codegen_backend::{Artifact, Backend, Target};
use zo_codegen_clif::CliftGen;
use zo_interner::Interner;
use zo_linker::link_to_executable;
use zo_sir::Sir;

use std::path::Path;

/// Concrete backend selected per [`Target`]. The common
/// `generate` path routes through the [`Backend`] trait;
/// ARM-specific post-processing (Mach-O writer, executable
/// bit, asm text) stays on the `ARM64Gen` arm. `ARM64Gen`
/// is ~700 bytes so it's boxed to keep the enum compact —
/// `Concrete` is held briefly on the stack in `make_backend`.
enum Concrete<'a> {
  Arm64(Box<ARM64Gen<'a>>),
  Clift(CliftGen<'a>),
}

/// Represents the [`Codegen`] dispatcher.
pub struct Codegen {
  target: Target,
}

impl Codegen {
  /// Creates a new [`Codegen`] instance.
  pub const fn new(target: Target) -> Self {
    Self { target }
  }

  /// Instantiates the backend matching `self.target`.
  fn make_backend<'a>(&self, interner: &'a Interner) -> Concrete<'a> {
    match self.target {
      Target::Arm64AppleDarwin | Target::Arm64UnknownLinuxGnu => {
        Concrete::Arm64(Box::new(ARM64Gen::new(interner)))
      }
      Target::X8664AppleDarwin
      | Target::X8664UnknownLinuxGnu
      | Target::X8664PcWindowsMsvc
      | Target::Arm64PcWindowsMsvc => {
        Concrete::Clift(CliftGen::new(interner, self.target))
      }
      Target::Wasm32UnknownUnknown => todo!("wasm backend not yet wired"),
    }
  }

  /// Generates binary code and writes to file.
  ///
  /// The ARM path wraps the raw machine code into a Mach-O
  /// executable and sets the executable bit. The Cranelift path
  /// shells out to `cc` (via `zo-linker`) to turn the emitted
  /// relocatable object into an executable — the system linker
  /// supplies `crt0` / `crt1` and resolves FFI imports against
  /// libc / libSystem. Errors from `cc` are surfaced to stderr;
  /// the user's output file is left untouched on failure.
  pub fn generate(self, interner: &Interner, sir: &Sir, output_path: &Path) {
    let target = self.target;

    match self.make_backend(interner) {
      Concrete::Arm64(mut codegen) => {
        let artifact = codegen.generate(sir);
        let executable = codegen.generate_macho(artifact);

        ARM64Gen::write_executable(executable, output_path).ok();
      }
      Concrete::Clift(mut codegen) => {
        let artifact = codegen.generate(sir);

        if let Err(err) =
          link_to_executable(&artifact.code, output_path, target)
        {
          eprintln!("zo: link failed: {err}");
        }
      }
    }
  }

  /// Generates the [`Artifact`].
  pub fn generate_artifact(&self, interner: &Interner, sir: &Sir) -> Artifact {
    match self.make_backend(interner) {
      Concrete::Arm64(mut codegen) => codegen.generate(sir),
      Concrete::Clift(mut codegen) => codegen.generate(sir),
    }
  }

  /// Generates assembly text for display. ARM returns
  /// disassembled ARM64; Cranelift returns CLIF IR text (pre-
  /// machine-code — equivalently useful for debugging the
  /// backend's own decisions, and avoids pulling a disassembler
  /// dep into the CLIF path).
  pub fn generate_asm(&self, interner: &Interner, sir: &Sir) -> String {
    match self.make_backend(interner) {
      Concrete::Arm64(mut codegen) => codegen.generate_asm(sir),
      Concrete::Clift(mut codegen) => codegen.generate_asm(sir),
    }
  }
}
