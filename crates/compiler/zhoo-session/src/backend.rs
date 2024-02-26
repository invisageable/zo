#[derive(Clone, Debug, Default)]
pub struct Backend {
  pub kind: BackendKind,
  pub triplet: smol_str::SmolStr,
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

impl From<smol_str::SmolStr> for Backend {
  fn from(backend: smol_str::SmolStr) -> Self {
    Self {
      kind: BackendKind::from(backend),
      triplet: smol_str::SmolStr::new_inline("arm64-apple-darwin"),
    }
  }
}

impl From<smol_str::SmolStr> for BackendKind {
  fn from(backend: smol_str::SmolStr) -> Self {
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
