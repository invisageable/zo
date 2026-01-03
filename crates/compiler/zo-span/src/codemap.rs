use crate::Span;

#[derive(Debug)]
pub struct CodeMap {
  /// Store file paths
  file_paths: Vec<String>,
  /// For each file, store line-by-line locations
  lines_info: Vec<Vec<Span>>,
}
impl CodeMap {
  pub fn new() -> Self {
    Self {
      file_paths: Vec::new(),
      lines_info: Vec::new(),
    }
  }

  /// Add a file to the [`CodeMap`].
  pub fn add_file(&mut self, path: &str) {
    self.file_paths.push(path.to_string());
    self.lines_info.push(Vec::new());
  }
}
impl Default for CodeMap {
  fn default() -> Self {
    Self::new()
  }
}
