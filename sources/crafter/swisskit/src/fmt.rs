mod doc;
mod formatter;
pub mod zo;

pub use doc::{Doc, pp};
pub use formatter::{Formatter, format};

/// The representation of a separator.
///
/// it takes a collection of `<T>`` and a separator.
struct Sep<'a, T: 'a>(pub &'a [T], pub &'a str);

impl<'a, T: std::fmt::Display> std::fmt::Display for Sep<'a, T> {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    self.iter().enumerate().try_fold((), |_, (idx, node)| {
      if idx < self.0.len() - 1 {
        return write!(f, "{node}{}", self.1);
      }

      write!(f, "{node}")
    })
  }
}
impl<'a, T: std::fmt::Display> std::ops::Deref for Sep<'a, T> {
  type Target = [T];

  #[inline]
  fn deref(&self) -> &Self::Target {
    self.0
  }
}

/// Separates elements based on the separator.
#[inline]
pub fn sep<'a>(
  nodes: &'a [impl std::fmt::Display],
  separator: &'a str,
) -> String {
  Sep(nodes, separator).to_string()
}

/// Separates elements by colon.
#[inline]
pub fn sep_colon(nodes: &[impl std::fmt::Display]) -> String {
  sep(nodes, ": ")
}

/// Separates elements by comma.
#[inline]
pub fn sep_comma(nodes: &[impl std::fmt::Display]) -> String {
  sep(nodes, ", ")
}

/// Separates elements by new line.
#[inline]
pub fn sep_newline(nodes: &[impl std::fmt::Display]) -> String {
  sep(nodes, "\n")
}

/// Separates elements by new line.
#[inline]
pub fn sep_space(nodes: &[impl std::fmt::Display]) -> String {
  sep(nodes, " ")
}
