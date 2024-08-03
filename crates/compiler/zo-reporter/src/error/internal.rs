use super::{Diagnostic, Error};

use crate::report::{Report, ReportKind};

/// The `internal` errors.
#[derive(Debug)]
pub enum Internal {
  Io(std::io::Error),
}

impl<'a> Diagnostic<'a> for Internal {
  fn report(&self) -> Report<'a> {
    match self {
      Self::Io(error) => Report {
        kind: ReportKind::ERROR,
        message: format!("{error}").into(),
        ..Default::default()
      },
    }
  }
}

/// A callback to map an [`std::io::Error`].
///
/// #### examples.
///
/// ```ignore
/// use zo_reporter::error;
///
/// std::fs::read("my-pathname")
///   .map_err(error::internal::io)
///   .map(|f| /* do your stuff */);
/// ```
#[inline]
pub fn io(message: std::io::Error) -> Error {
  Error::Internal(Internal::Io(message))
}
