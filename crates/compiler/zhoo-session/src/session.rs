use zo_core::interner::Interner;
use zo_core::reporter::Reporter;

#[derive(Debug)]
pub struct Session {
  pub input: smol_str::SmolStr,
  pub interner: Interner,
  pub reporter: Reporter,
}

impl Default for Session {
  fn default() -> Self {
    Self {
      input: smol_str::SmolStr::new_inline(""),
      interner: Interner::new(),
      reporter: Reporter::new(),
    }
  }
}
