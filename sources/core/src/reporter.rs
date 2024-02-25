pub mod report;

use report::{Error, ReportError};

#[derive(Debug)]
pub struct Reporter {
  has_errors: std::sync::Arc<std::sync::Mutex<bool>>,
}

impl Reporter {
  pub fn new() -> Self {
    Self {
      has_errors: std::sync::Arc::new(std::sync::Mutex::new(false)),
    }
  }

  pub fn add_report(&self, error: ReportError) {
    let _report = match error {
      ReportError::Io(error) => error.report(),
      ReportError::Lexical(error) => error.report(),
      ReportError::Syntax(error) => error.report(),
      ReportError::Semantic(error) => error.report(),
      ReportError::Assembly(error) => error.report(),
    };

    *self.has_errors.lock().unwrap() = true;
  }

  pub fn raise(&self, error: ReportError) -> ! {
    self.add_report(error);
    self.abort()
  }

  fn abort(&self) -> ! {
    std::process::exit(1)
  }
}

impl Default for Reporter {
  fn default() -> Self {
    Self::new()
  }
}
