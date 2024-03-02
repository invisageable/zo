//! ...

use smol_str::SmolStr;

#[derive(Clone, Debug, Default)]
pub struct Backend {
  pub kind: BackendKind,
  pub triplet: SmolStr,
}

#[derive(Copy, Clone, Debug, Default)]
pub enum BackendKind {
  Cranelift,
  #[default]
  Wasm,
}

impl std::fmt::Display for Backend {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", self.kind)
  }
}

impl From<SmolStr> for Backend {
  fn from(backend: SmolStr) -> Self {
    Self {
      kind: BackendKind::from(backend),
      triplet: SmolStr::new_inline("arm64-apple-darwin"),
    }
  }
}

impl From<SmolStr> for BackendKind {
  fn from(backend: SmolStr) -> Self {
    match backend.as_str() {
      "cranelift" => Self::Cranelift,
      "wasm" => Self::Wasm,
      _ => unreachable!(),
    }
  }
}

impl std::fmt::Display for BackendKind {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Cranelift => write!(f, "cranelift"),
      Self::Wasm => write!(f, "wasm"),
    }
  }
}
