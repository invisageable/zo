use zo_error::Error;

use std::cell::RefCell;

thread_local! {
  /// Thread-local error reporter instance.
  static REPORTER: RefCell<ThreadLocalReporter> = const {
    RefCell::new(ThreadLocalReporter::new())
  };
}

/// Maximum number of errors per thread.
const MAX_ERRORS: usize = 128;

/// Thread-local error reporter with fixed-size buffer.
/// This provides zero-allocation error collection during compilation.
pub struct ThreadLocalReporter {
  /// Fixed-size array for errors.
  errors: [Error; MAX_ERRORS],
  /// Current number of errors.
  count: usize,
}
impl ThreadLocalReporter {
  /// Creates a new empty reporter.
  pub const fn new() -> Self {
    Self {
      errors: [Error::new(unsafe { std::mem::zeroed() }, zo_span::Span::ZERO);
        MAX_ERRORS],
      count: 0,
    }
  }

  /// Reports an error, adding it to the buffer if there's space.
  #[inline(always)]
  pub fn report(&mut self, error: Error) -> bool {
    if self.count < MAX_ERRORS {
      self.errors[self.count] = error;
      self.count += 1;
      true
    } else {
      false
    }
  }

  /// Returns the current error count.
  #[inline(always)]
  pub fn error_count(&self) -> usize {
    self.count
  }

  /// Clears all errors from the reporter.
  #[inline(always)]
  pub fn clear(&mut self) {
    self.count = 0;
  }

  /// Returns true if the buffer is full.
  #[inline(always)]
  pub fn is_full(&self) -> bool {
    self.count >= MAX_ERRORS
  }

  /// Drains all errors into a Vec.
  pub fn drain(&mut self) -> Vec<Error> {
    let mut errors = Vec::new();

    errors.extend_from_slice(&self.errors[..self.count]);

    self.count = 0;

    errors
  }
}
impl Default for ThreadLocalReporter {
  fn default() -> Self {
    Self::new()
  }
}

/// Reports an error to the thread-local reporter.
/// The error kind itself determines the context (tokenizer vs parser vs
/// semantic).
#[inline(always)]
pub fn report_error(error: Error) -> bool {
  REPORTER.with(|reporter| reporter.borrow_mut().report(error))
}

/// Returns the current error count for this thread.
#[inline(always)]
pub fn error_count() -> usize {
  REPORTER.with(|reporter| reporter.borrow().error_count())
}

/// Clears all errors from the thread-local reporter.
#[inline(always)]
pub fn clear_errors() {
  REPORTER.with(|reporter| reporter.borrow_mut().clear())
}

/// Collects all errors from the thread-local reporter.
#[inline(always)]
pub fn collect_errors() -> Vec<Error> {
  REPORTER.with(|reporter| reporter.borrow_mut().drain())
}
