use clap::builder::PossibleValue;
use clap::{Parser, ValueEnum};

/// The representation of a backend for the code generation.
#[derive(Clone, Debug, Default, Parser)]
pub enum Backend {
  /// python.
  Py,
  /// webassembly.
  #[default]
  Wasm,
}

impl ValueEnum for Backend {
  fn value_variants<'a>() -> &'a [Self] {
    &[Self::Wasm, Self::Py]
  }

  fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
    match self {
      Self::Py => Some(PossibleValue::new("py")),
      Self::Wasm => Some(PossibleValue::new("wasm")),
    }
  }
}
