use clap::builder::PossibleValue;
use clap::{Parser, ValueEnum};
use smol_str::{SmolStr, ToSmolStr};

/// The representation of a backend for the code generation.
///
/// Default is [`Backend::Wasm`].
#[derive(Clone, Copy, Debug, Default, Parser)]
pub enum Backend {
  /// The `cranelift` backend.
  Clif,
  /// The `python` backend.
  Py,
  /// The `webassembly` backend.
  #[default]
  Wasm,
  /// The `zo` backend.
  Zo,
}

impl From<Backend> for SmolStr {
  #[inline]
  fn from(backend: Backend) -> Self {
    backend.to_smolstr()
  }
}

impl std::fmt::Display for Backend {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Clif => write!(f, "clif"),
      Self::Py => write!(f, "py"),
      Self::Wasm => write!(f, "wasm"),
      Self::Zo => write!(f, "zo"),
    }
  }
}

impl ValueEnum for Backend {
  fn value_variants<'a>() -> &'a [Self] {
    &[Self::Clif, Self::Py, Self::Wasm, Self::Zo]
  }

  fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
    match self {
      Self::Clif => Some(PossibleValue::new("clif")),
      Self::Py => Some(PossibleValue::new("py")),
      Self::Wasm => Some(PossibleValue::new("wasm")),
      Self::Zo => Some(PossibleValue::new("zo")),
    }
  }
}
