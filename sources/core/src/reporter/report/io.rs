use super::{Error, Report, ReportError, ReportKind, REPORT_TITLE_ERROR};

#[derive(Debug)]
pub struct Io(pub std::io::Error);

impl Error for Io {
  fn report(&self) -> Report {
    Report {
      kind: ReportKind::Error(REPORT_TITLE_ERROR),
      message: format!("{}", self.0).into(),
      labels: Vec::with_capacity(0usize),
      notes: Vec::with_capacity(0usize),
      helps: Vec::with_capacity(0usize),
    }
  }
}

impl Io {
  pub fn error(message: std::io::Error) -> ReportError {
    ReportError::Io(Io(message))
  }
}
