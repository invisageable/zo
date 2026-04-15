use zo_interner::Interner;
use zo_ty_checker::TyChecker;

/// Shared compilation state that lives for the entire compilation pipeline.
/// Owns the interner (symbol table) and type checker so all stages —
/// tokenizer, parser, executor, codegen — share a single namespace.
pub struct Session {
  pub interner: Interner,
  pub ty_checker: TyChecker,
}

impl Session {
  pub fn new() -> Self {
    Self {
      interner: Interner::new(),
      ty_checker: TyChecker::new(),
    }
  }
}

impl Default for Session {
  fn default() -> Self {
    Self::new()
  }
}
