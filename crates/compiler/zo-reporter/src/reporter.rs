use zo_error::Error;

pub struct Reporter {
  errors: Vec<Error>,
}
impl Reporter {
  pub fn new() -> Self {
    Self {
      errors: Vec::with_capacity(0),
    }
  }

  pub fn collect_errors(&mut self, errors: &[Error]) {
    self.errors.extend_from_slice(errors);
  }

  pub fn errors(&self) -> &Vec<Error> {
    &self.errors
  }
}
impl Default for Reporter {
  fn default() -> Self {
    Self::new()
  }
}
