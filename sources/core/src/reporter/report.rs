mod assembly;
mod io;
mod lexical;
mod semantic;
mod syntax;

use crate::color;
use crate::span::Span;

pub(crate) const REPORT_TITLE_ERROR: &str = "error";

pub trait Error: Sized {
  fn report(&self) -> Report;
}

#[derive(Debug)]
pub struct Report {
  pub kind: ReportKind,
  pub message: smol_str::SmolStr,
  pub labels: Vec<(Span, smol_str::SmolStr, ariadne::Color)>,
  pub notes: Vec<smol_str::SmolStr>,
  pub helps: Vec<smol_str::SmolStr>,
}

impl Default for Report {
  fn default() -> Self {
    Self {
      kind: ReportKind::Error(REPORT_TITLE_ERROR),
      message: smol_str::SmolStr::default(),
      labels: Vec::with_capacity(0),
      notes: Vec::with_capacity(0),
      helps: Vec::with_capacity(0),
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
  Io(std::io::Error),
  Lexical(lexical::Lexical),
  Syntax(syntax::Syntax),
  Semantic(semantic::Semantic),
  Assembly(assembly::Assembly),
}

impl ReportError {
  fn as_code(&self) -> i32 {
    match self {
      Self::Io(_) => 1,
      Self::Lexical(_) => 2,
      Self::Syntax(_) => 3,
      Self::Semantic(_) => 4,
      Self::Assembly(_) => 5,
    }
  }
}

impl std::fmt::Display for ReportError {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{:03}", self.as_code())
  }
}
