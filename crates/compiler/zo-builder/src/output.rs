//! ...

use zo_session::backend::Backend;

#[derive(Clone, Debug)]
pub struct Output {
  pub backend: Backend,
  pub files: Files,
}

#[derive(Clone, Debug, Default)]
pub struct Files(pub Vec<File>);

impl Files {
  // no allocations.
  #[inline]
  pub fn new() -> Self {
    Self(Vec::with_capacity(0usize))
  }

  #[inline]
  pub fn add_file(&mut self, file: File) {
    self.0.push(file);
  }
}

impl std::ops::Deref for Files {
  type Target = Vec<File>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

#[derive(Clone, Debug)]
pub struct File {
  pub kind: FileKind,
  pub pathname: std::path::PathBuf,
}

impl File {
  pub fn new<P: Into<std::path::PathBuf>>(kind: FileKind, pathname: P) -> Self {
    Self {
      kind,
      pathname: pathname.into(),
    }
  }
}

impl std::fmt::Display for File {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}: `{}`", self.kind, self.pathname.display())
  }
}

#[derive(Clone, Debug)]
pub enum FileKind {
  Py,
  Wasm,
  Wat,
}

impl std::fmt::Display for FileKind {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Py => write!(f, "py"),
      Self::Wasm => write!(f, "wasm"),
      Self::Wat => write!(f, "wat"),
    }
  }
}
