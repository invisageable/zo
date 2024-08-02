use clap::builder::PossibleValue;
use clap::{Parser, ValueEnum};

/// The representation of a backend for the code generation.
///
/// Default is [`Backend::Wasm`].
#[derive(Clone, Debug, Default, Parser)]
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
