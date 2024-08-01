use super::{Diagnostic, Error};

use crate::report::{Report, ReportKind};

/// The `io` errors.
#[derive(Debug)]
pub struct Io(std::io::Error);

impl Io {
  /// A callback to map an [`std::io::Error`].
  ///
  /// #### examples.
  ///
  /// ```
  /// std::fs::read("my-pathname")
  ///   .map_err(Io::error)
  ///   .map(|f| /* do your stuff */);
  /// ```
  #[inline]
  pub fn error(message: std::io::Error) -> Error {
    Error::Io(Io(message))
  }
}

impl<'a> Diagnostic<'a> for Io {
  fn report(&self) -> Report<'a> {
    Report {
      kind: ReportKind::ERROR,
      message: format!("{}", self.0).into(),
      ..Default::default()
    }
  }
}
