//! A simple [`Timer`] to get durations at a specific instant.

use std::fmt::Debug;

/// The representation of a timer.
#[derive(Clone, Debug, Default)]
pub struct Timer {
  /// The start time of the timer.
  pub maybe_time_start: Option<Time>,
  /// The end time of the timer.
  pub maybe_time_end: Option<Time>,
}

impl Timer {
  /// Creates a new timer.
  #[inline]
  pub fn new() -> Self {
    Self::default()
  }

  /// Sets the start time corresponding to `now`.
  #[inline]
  pub fn start(&mut self) {
    self.maybe_time_start = Some(Time::now());
  }

  /// Sets the end time corresponding to `now`.
  #[inline]
  pub fn end(&mut self) {
    self.maybe_time_end = Some(Time::now());
  }

  /// Sleeps from `ms`.
  #[inline]
  pub fn sleep(&mut self, millis: u64) {
    std::thread::sleep(std::time::Duration::from_millis(millis));
  }

  /// Resets the timer.
  #[inline]
  pub fn reset(&mut self) {
    self.maybe_time_start = None;
    self.maybe_time_end = None;
  }

  /// Gets the current duration.
  #[inline]
  pub fn duration(&self) -> Option<std::time::Duration> {
    match (&self.maybe_time_start, &self.maybe_time_end) {
      (Some(start), Some(end)) => Time::merge(start, end),
      _ => None,
    }
  }

  /// Gets the current duration from a unit.
  ///
  /// See also [`Unit`] to get the full list of available units.
  #[inline]
  pub fn duration_in_unit(&self, unit: impl Into<Unit>) -> Option<f64> {
    self
      .duration()
      .map(|duration| duration.as_nanos() as f64 / unit.into().as_factor())
  }
}

impl Drop for Timer {
  /// Just kill this motherfucker before drop in it into the river.
  #[inline]
  fn drop(&mut self) {
    self.reset();
  }
}

/// A [`Time`] representation.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Time {
  /// The current instant corresponding to `now`.
  pub maybe_instant: Option<std::time::Instant>,
}

impl Time {
  /// Creates a new time instance.
  #[inline]
  pub fn now() -> Self {
    Self {
      maybe_instant: Some(std::time::Instant::now()),
    }
  }

  /// Gets the duration from two times.
  #[inline]
  pub fn merge(start: &Self, end: &Self) -> Option<std::time::Duration> {
    match (start.maybe_instant, end.maybe_instant) {
      (Some(start), Some(end)) => Some(end.duration_since(start)),
      _ => None,
    }
  }
}

impl std::fmt::Display for Time {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    std::fmt::Debug::fmt(&self, f)
  }
}

/// The representation of a duration unit.
pub enum Unit {
  /// The nanosecond duration unit.
  Ns,
  /// The micro-second duration unit.
  Us,
  /// The millisecond duration unit.
  Ms,
  /// The second duration unit.
  S,
}

impl Unit {
  /// Converts unit variant into a factor unit.
  #[inline]
  pub fn as_factor(&self) -> f64 {
    match self {
      Self::Ns => 1.0,
      Self::Us => 1_000.0,
      Self::Ms => 1_000_000.0,
      Self::S => 1_000_000_000.0,
    }
  }
}

impl std::fmt::Display for Unit {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Ns => write!(f, "ns"),
      Self::Us => write!(f, "us"),
      Self::Ms => write!(f, "ms"),
      Self::S => write!(f, "s"),
    }
  }
}

impl From<Unit> for &'static str {
  fn from(unit: Unit) -> Self {
    match unit {
      Unit::Ns => "ns",
      Unit::Us => "us",
      Unit::Ms => "ms",
      Unit::S => "s",
    }
  }
}
