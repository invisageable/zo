use zo_codegen_backend::{Artifact, Backend};
use zo_emitter_x86::X64Emitter;
use zo_sir::Sir;

/// Represents an [`X64Gen`] instance.
///
/// Lowers SIR into x86-64 machine code. The emitter and ABI
/// lowering are scaffolded; the SIR walk is not yet written.
pub struct X64Gen {
  emitter: X64Emitter,
}

impl X64Gen {
  /// Creates a new [`X64Gen`] instance.
  pub fn new() -> Self {
    Self { emitter: X64Emitter::new() }
  }

  /// Gets the underlying [`X64Emitter`].
  pub fn emitter(&self) -> &X64Emitter {
    &self.emitter
  }
}

impl Default for X64Gen {
  fn default() -> Self {
    Self::new()
  }
}

impl Backend for X64Gen {
  fn generate(&mut self, _sir: &Sir) -> Artifact {
    todo!("x86-64 SIR lowering — see the intel-x86-64-expert agent")
  }
}
