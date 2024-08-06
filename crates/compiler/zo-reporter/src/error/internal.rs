use super::{Diagnostic, Error};

use crate::color;
use crate::report::{Report, ReportKind};

use ariadne::Fmt;
use smol_str::SmolStr;

/// The `internal` errors.
#[derive(Debug)]
pub enum Internal {
  /// A `channel` errors.
  Channel(Channel),
  /// An expected backend error.
  ExpectedBackend(Vec<SmolStr>, SmolStr),
  /// An expected event error.
  ExpectedEvent(SmolStr),
  /// An io error.
  Io(std::io::Error),
}

impl<'a> Diagnostic<'a> for Internal {
  #[inline]
  fn report(&self) -> Report<'a> {
    match self {
      Self::Channel(error) => error.report(),
      Self::ExpectedBackend(events, event) => todo!("{events:?} — {event}"),
      Self::ExpectedEvent(name) => todo!("{name}"),
      Self::Io(error) => Report {
        kind: ReportKind::ERROR,
        message: format!("{error}").into(),
        ..Default::default()
      },
      _ => todo!(),
    }
  }
}

/// The `channel` errors.
#[derive(Debug)]
pub enum Channel {
  /// A no sender error.
  NoSender(SmolStr),
  /// A no receiver error.
  NoReceiver(SmolStr),
}

impl<'a> Diagnostic<'a> for Channel {
  #[inline]
  fn report(&self) -> Report<'a> {
    match self {
      Self::NoReceiver(error) => Report {
        kind: ReportKind::ERROR,
        message: format!(
          "{} {error}",
          "no receiver signal received.".fg(color::title())
        )
        .into(),
        ..Default::default()
      },
      Self::NoSender(error) => Report {
        kind: ReportKind::ERROR,
        message: format!(
          "{}: {error}",
          "no sender signal sended.".fg(color::title())
        )
        .into(),
        ..Default::default()
      },
    }
  }
}

/// An expected backend error.
#[inline]
pub fn expected_backend(
  events: Vec<impl Into<SmolStr>>,
  event: impl Into<SmolStr>,
) -> Error {
  Error::Internal(Internal::ExpectedBackend(
    events.into_iter().map(|e| e.into()).collect(),
    event.into(),
  ))
}

/// An expected event error.
#[inline]
pub fn expected_event(event: impl Into<SmolStr>) -> Error {
  Error::Internal(Internal::ExpectedEvent(event.into()))
}

/// A no sender error.
#[inline]
pub fn no_receiver(send_error: impl Into<SmolStr>) -> Error {
  Error::Internal(Internal::Channel(Channel::NoReceiver(send_error.into())))
}

/// A no receiver error.
#[inline]
pub fn no_sender(receive_error: impl Into<SmolStr>) -> Error {
  Error::Internal(Internal::Channel(Channel::NoSender(receive_error.into())))
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
pub const fn io(message: std::io::Error) -> Error {
  Error::Internal(Internal::Io(message))
}
