use zo_session::backend::Backend;

/// The output information — pathname, backend, etc.
#[derive(Debug, Default)]
pub struct Output {
  /// The current backend.
  pub backend: Backend,
  /// A set of files generated by the `build` compiler phase.
  pub files: Vec<File>,
}

impl std::fmt::Display for Output {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{self:?}")
  }
}

/// The representation of a file output.
#[derive(Debug, Default)]
pub struct File {
  /// A file kind — see also [`FileKind`] for more information.
  pub kind: FileKind,
  /// A pathname of a file.
  pub pathname: std::path::PathBuf,
}

impl File {
  /// Creates a new file.
  #[inline]
  pub fn new(kind: FileKind, pathname: impl Into<std::path::PathBuf>) -> Self {
    Self {
      kind,
      pathname: pathname.into(),
    }
  }
}

impl std::fmt::Display for File {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}: `{}`", self.kind, self.pathname.display())
  }
}

/// The representation of a file kind.
#[derive(Debug, Default)]
pub enum FileKind {
  /// The `.exe` file kind.
  Exe,
  /// The `.ll` file kind.
  Ll,
  /// The `.py` file kind.
  Py,
  /// The `.o` file kind.
  Obj,
  /// The `.wasm` file kind.
  #[default]
  Wasm,
  /// The `.wat` file kind.
  Wat,
}

impl std::fmt::Display for FileKind {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Exe => write!(f, "exe"),
      Self::Ll => write!(f, "ll"),
      Self::Py => write!(f, "py"),
      Self::Obj => write!(f, "o"),
      Self::Wasm => write!(f, "wasm"),
      Self::Wat => write!(f, "wat"),
    }
  }
}
