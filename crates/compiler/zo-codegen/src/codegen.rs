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

  /// Creates a target-specific codegen and applies `f`.
  fn with_backend<T>(
    &self,
    interner: &Interner,
    f: impl FnOnce(&mut ARM64Gen) -> T,
  ) -> T {
    match self.target {
      Target::Arm64AppleDarwin => {
        let mut codegen = ARM64Gen::new(interner);

        f(&mut codegen)
      }
      target => todo!("{target:?} not implemented"),
    }
  }

  /// Generates binary code and writes to file.
  pub fn generate(self, interner: &Interner, sir: &Sir, output_path: &Path) {
    self.with_backend(interner, |codegen| {
      let artifact = codegen.generate(sir);
      let executable = codegen.generate_macho(artifact);

      ARM64Gen::write_executable(executable, output_path).ok();
    });
  }

  /// Generates the [`Artifact`].
  pub fn generate_artifact(&self, interner: &Interner, sir: &Sir) -> Artifact {
    self.with_backend(interner, |codegen| codegen.generate(sir))
  }

  /// Generates assembly text for display.
  pub fn generate_asm(&self, interner: &Interner, sir: &Sir) -> String {
    self.with_backend(interner, |codegen| codegen.generate_asm(sir))
  }
}
