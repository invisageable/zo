use crate::artifact::Artifact;

use zo_sir::Sir;

/// Backend trait for code generation targets.
pub trait Backend {
  fn generate(&mut self, sir: &Sir) -> Artifact;
}
