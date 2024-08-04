use super::{Diagnostic, Error};

use crate::report::{Report, ReportKind};

use smol_str::SmolStr;

/// The `internal` errors.
#[derive(Debug)]
pub enum Internal {
  /// An expected event error.
  ExpectedEvent(SmolStr),
  /// An io error.
  Io(std::io::Error),
}

impl<'a> Diagnostic<'a> for Internal {
  #[inline]
  fn report(&self) -> Report<'a> {
    match self {
      Self::Io(error) => Report {
        kind: ReportKind::ERROR,
        message: format!("{error}").into(),
        ..Default::default()
      },
      _ => todo!(),
    }
  }
}

/// An expected event error.
#[inline]
pub fn expected_event(event: impl Into<SmolStr>) -> Error {
  Error::Internal(Internal::ExpectedEvent(event.into()))
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
