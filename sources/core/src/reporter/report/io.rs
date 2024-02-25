use super::{Error, Report, ReportKind, REPORT_TITLE_ERROR};

impl Error for std::io::Error {
  fn report(&self) -> Report {
    Report {
      kind: ReportKind::Error(REPORT_TITLE_ERROR),
      message: format!("{self}").into(),
      labels: Vec::with_capacity(0),
      notes: Vec::with_capacity(0),
      helps: Vec::with_capacity(0),
    }
  }
}
