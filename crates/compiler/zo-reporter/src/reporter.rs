use crate::collector::Detail;

use zo_error::Error;

pub struct Reporter {
  errors: Vec<Error>,
  details: Vec<(Error, Detail)>,
}

impl Reporter {
  pub fn new() -> Self {
    Self {
      errors: Vec::with_capacity(0),
      details: Vec::new(),
    }
  }

  pub fn collect_errors(&mut self, errors: &[Error]) {
    self.errors.extend_from_slice(errors);
  }

  /// Accumulate type-name detail drained from a thread-local
  /// reporter, alongside its errors.
  pub fn collect_details(&mut self, details: Vec<(Error, Detail)>) {
    self.details.extend(details);
  }

  pub fn errors(&self) -> &Vec<Error> {
    &self.errors
  }

  pub fn details(&self) -> &[(Error, Detail)] {
    &self.details
  }
}

impl Default for Reporter {
  fn default() -> Self {
    Self::new()
  }
}
