pub mod assembly;
pub mod chan;
pub mod eval;
pub mod io;
pub mod lexical;
pub mod semantic;
pub mod syntax;

use crate::color;
use crate::span::Span;

use smol_str::SmolStr;

pub(crate) const REPORT_TITLE_ERROR: &str = "error";
pub(crate) const REPORT_TITLE_WARNING: &str = "warning";

pub trait Error: Sized {
  fn report(&self) -> Report;
}

#[derive(Debug)]
pub struct Report {
  pub kind: ReportKind,
  pub message: SmolStr,
  pub labels: Vec<(Span, SmolStr, ariadne::Color)>,
  pub notes: Vec<SmolStr>,
  pub helps: Vec<SmolStr>,
}

impl Default for Report {
  /// no allocation.
  fn default() -> Self {
    Self {
      kind: ReportKind::Error(REPORT_TITLE_ERROR),
      message: SmolStr::default(),
      labels: Vec::with_capacity(0usize),
      notes: Vec::with_capacity(0usize),
      helps: Vec::with_capacity(0usize),
    }
  }
}

#[derive(Debug, PartialEq)]
pub enum ReportKind {
  Error(&'static str),
  Warning(&'static str),
}

impl From<ReportKind> for ariadne::ReportKind<'static> {
  fn from(kind: ReportKind) -> Self {
    match kind {
      ReportKind::Error(title) => {
        ariadne::ReportKind::Custom(title, color::error())
      }
      ReportKind::Warning(title) => {
        ariadne::ReportKind::Custom(title, color::warning())
      }
    }
  }
}

#[derive(Debug)]
pub enum ReportError {
  Io(io::Io),
  Chan(chan::Chan),
  Lexical(lexical::Lexical),
  Syntax(syntax::Syntax),
  Semantic(semantic::Semantic),
  Assembly(assembly::Assembly),
  Eval(eval::Eval),
}

impl ReportError {
  fn as_code(&self) -> i32 {
    match self {
      Self::Io(_) => 1,
      Self::Chan(_) => 2,
      Self::Lexical(_) => 3,
      Self::Syntax(_) => 4,
      Self::Semantic(_) => 5,
      Self::Assembly(_) => 6,
      Self::Eval(_) => 7,
    }
  }
}

impl std::fmt::Display for ReportError {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{:03}", self.as_code())
  }
}
