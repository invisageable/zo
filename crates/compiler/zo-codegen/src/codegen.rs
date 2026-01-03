use zo_codegen_arm::ARM64Gen;
use zo_codegen_backend::{Artifact, Target};
use zo_interner::Interner;
use zo_sir::Sir;

use std::path::Path;

/// Represents the [`Codegen`] dispatcher.
pub struct Codegen {
  target: Target,
}
impl Codegen {
  /// Creates a new [`Codegen`] instance.
  pub const fn new(target: Target) -> Self {
    Self { target }
  }

  /// Generates the [`Artifact`] binary code and writes to file.
  pub fn generate(self, interner: &Interner, sir: &Sir, output_path: &Path) {
    match self.target {
      Target::Arm64AppleDarwin => {
        let mut codegen = ARM64Gen::new(interner);
        let artifact = codegen.generate(sir);
        let executable = codegen.generate_macho(artifact);

        if let Err(_error) = ARM64Gen::write_executable(executable, output_path)
        {
          // Handle error
        }
      }
      target => todo!("{target:?} not implemented"),
    }
  }

  /// Generates the [`Artifact`].
  pub fn generate_artifact(&self, interner: &Interner, sir: &Sir) -> Artifact {
    match self.target {
      Target::Arm64AppleDarwin => {
        let mut codegen = ARM64Gen::new(interner);
        codegen.generate(sir)
      }
      target => todo!("{target:?} not implemented"),
    }
  }

  /// Generates assembly text for display.
  pub fn generate_asm(&self, interner: &Interner, sir: &Sir) -> String {
    match self.target {
      Target::Arm64AppleDarwin => {
        let mut codegen = ARM64Gen::new(interner);
        codegen.generate_asm(sir)
      }
      target => todo!("{target:?} not implemented"),
    }
  }
}
