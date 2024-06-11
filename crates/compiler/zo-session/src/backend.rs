//! ...

use smol_str::SmolStr;

#[derive(Clone, Debug, Default)]
pub struct Backend {
  pub kind: BackendKind,
}

#[derive(Copy, Clone, Debug, Default)]
pub enum BackendKind {
  Py,
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
    }
  }
}

impl From<SmolStr> for BackendKind {
  fn from(backend: SmolStr) -> Self {
    match backend.as_str() {
      "py" => Self::Py,
      "wasm" => Self::Wasm,
      _ => unreachable!(),
    }
  }
}

impl std::fmt::Display for BackendKind {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Py => write!(f, "py"),
      Self::Wasm => write!(f, "wasm"),
    }
  }
}
