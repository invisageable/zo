use super::color;

use swisskit::span::Span;

use smol_str::SmolStr;

/// The label of an error.
pub(crate) const REPORT_LABEL_ERROR: &str = "error";

/// The label of a warning.
pub(crate) const REPORT_LABEL_WARNING: &str = "warning";

/// The label of a advice.
pub(crate) const REPORT_LABEL_ADVICE: &str = "advice";

/// The representation of a report error message.
///
/// It is used as a wrapper of [`ariadne::Report`].
#[derive(Debug)]
pub struct Report<'a> {
  /// A type that defines the kind of report being produced.
  pub kind: ReportKind<'a>,
  /// A message of a report.
  pub message: SmolStr,
  /// A set of labels.
  pub labels: Vec<(Span, SmolStr, ariadne::Color)>,
  /// A set of notes.
  pub notes: Vec<SmolStr>,
  /// A set of helps.
  pub helps: Vec<SmolStr>,
}

impl<'a> Default for Report<'a> {
  #[inline]
  fn default() -> Self {
    Self {
      kind: ReportKind::ERROR,
      message: SmolStr::new_inline(""),
      labels: Vec::with_capacity(0usize),
      notes: Vec::with_capacity(0usize),
      helps: Vec::with_capacity(0usize),
    }
  }
}

/// The representation of a type.
///
/// We only have to kind of error:
///
/// 1. Error — critical, crash the program if handle.
/// 2. Warning — suggestion, do not crash the program if handle.
#[derive(Debug, PartialEq)]
pub enum ReportKind<'a> {
  /// Tells the user that's there is something critical in his pprogram.
  Error(&'a str),
  /// Prevents the user that's there is a mistake in his pprogram.
  Warning(&'a str),
  /// Gives the user some advice regarding his pprogram.
  Advice(&'a str),
}

impl<'a> ReportKind<'a> {
  /// The constant of a error kind.
  pub const ERROR: Self = Self::Error(REPORT_LABEL_ERROR);
  /// The constant of a warning kind.
  pub const WARNING: Self = Self::Warning(REPORT_LABEL_WARNING);
  /// The constant of a advice kind.
  pub const ADVICE: Self = Self::Advice(REPORT_LABEL_ADVICE);
}

impl Default for ReportKind<'_> {
  #[inline]
  fn default() -> Self {
    Self::ERROR
  }
}

impl<'a> From<ReportKind<'a>> for ariadne::ReportKind<'a> {
  #[inline]
  fn from(kind: ReportKind<'a>) -> Self {
    match kind {
      ReportKind::Error(title) => {
        ariadne::ReportKind::Custom(title, color::error())
      }
      ReportKind::Warning(title) => {
        ariadne::ReportKind::Custom(title, color::warning())
      }
      ReportKind::Advice(title) => {
        ariadne::ReportKind::Custom(title, color::advice())
      }
    }
  }
}
