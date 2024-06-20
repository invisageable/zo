//! ...
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

  fn deref(&self) -> &Self::Target {
    self.0
  }
}

#[inline]
pub fn sep<'a>(
  nodes: &'a [impl std::fmt::Display],
  separator: &'a str,
) -> String {
  Sep(nodes, separator).to_string()
}

#[inline]
pub fn sep_newline(nodes: &[impl std::fmt::Display]) -> String {
  sep(nodes, "\n")
}

#[inline]
pub fn sep_colon(nodes: &[impl std::fmt::Display]) -> String {
  sep(nodes, ": ")
}

#[inline]
pub fn sep_comma(nodes: &[impl std::fmt::Display]) -> String {
  sep(nodes, ", ")
}

// todo: implement tests.
