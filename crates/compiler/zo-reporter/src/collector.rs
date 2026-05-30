use zo_error::{Error, Severity};
use zo_span::Span;

use std::cell::RefCell;

thread_local! {
  /// Thread-local error reporter instance.
  static REPORTER: RefCell<ThreadLocalReporter> =
    const { RefCell::new(ThreadLocalReporter::new()) };
}

/// Maximum number of errors per thread.
const MAX_ERRORS: usize = 128;

/// The two conflicting type names for a diagnostic that names
/// them — currently a `TypeMismatch`. The `Error` itself is a
/// compact 16-byte value with no room for strings, so this
/// rich detail rides in a side store keyed by the `Error`.
#[derive(Clone, Debug)]
pub struct TyNames {
  /// Type of the primary (offending) value — `int` in
  /// `42 ++ "x"`.
  pub primary: Box<str>,
  /// Type of the value it conflicts with — `str`.
  pub secondary: Box<str>,
}

/// Thread-local error reporter with fixed-size buffer.
/// This provides zero-allocation error collection during compilation.
pub struct ThreadLocalReporter {
  /// Fixed-size array for errors.
  errors: [Error; MAX_ERRORS],
  /// Current number of errors.
  count: usize,
  /// Side store of type-name detail, keyed by the `Error` it
  /// annotates. A `Vec` (not a `HashMap`) so `new` stays
  /// `const`; lookups happen only on the cold render path.
  details: Vec<(Error, TyNames)>,
}

impl ThreadLocalReporter {
  /// Creates a new empty reporter.
  pub const fn new() -> Self {
    Self {
      errors: [Error::new(unsafe { std::mem::zeroed() }, Span::ZERO);
        MAX_ERRORS],
      count: 0,
      details: Vec::new(),
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

  /// Reports an error and attaches its conflicting type names.
  pub fn report_with_types(&mut self, error: Error, names: TyNames) -> bool {
    if self.report(error) {
      self.details.push((error, names));

      true
    } else {
      false
    }
  }

  /// Returns the count of buffered diagnostics with
  /// `Severity::Error`. Warnings are excluded — use this
  /// to decide whether the build should fail.
  #[inline(always)]
  pub fn error_count(&self) -> usize {
    self.errors[..self.count]
      .iter()
      .filter(|e| matches!(e.severity(), Severity::Error))
      .count()
  }

  /// Returns the count of buffered diagnostics with
  /// `Severity::Warning`.
  #[inline(always)]
  pub fn warning_count(&self) -> usize {
    self.errors[..self.count]
      .iter()
      .filter(|e| matches!(e.severity(), Severity::Warning))
      .count()
  }

  /// Returns the total count of buffered diagnostics
  /// (errors + warnings). Same as the buffer's fill level.
  #[inline(always)]
  pub fn total_count(&self) -> usize {
    self.count
  }

  /// Clears all errors from the reporter.
  #[inline(always)]
  pub fn clear(&mut self) {
    self.count = 0;
    self.details.clear();
  }

  /// Returns true if the buffer is full.
  #[inline(always)]
  pub fn is_full(&self) -> bool {
    self.count >= MAX_ERRORS
  }

  /// Drains all errors into a Vec, discarding type detail.
  pub fn drain(&mut self) -> Vec<Error> {
    let result = self.errors[..self.count].to_vec();
    self.count = 0;
    self.details.clear();
    result
  }

  /// Drains errors together with their type-name detail.
  pub fn drain_with_details(&mut self) -> (Vec<Error>, Vec<(Error, TyNames)>) {
    let errors = self.errors[..self.count].to_vec();
    let details = std::mem::take(&mut self.details);

    self.count = 0;

    (errors, details)
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

/// Reports an error and attaches its conflicting type names.
pub fn report_error_with_types(error: Error, names: TyNames) -> bool {
  REPORTER
    .with(|reporter| reporter.borrow_mut().report_with_types(error, names))
}

/// Returns the count of buffered hard errors for this
/// thread. Warnings are excluded.
#[inline(always)]
pub fn error_count() -> usize {
  REPORTER.with(|reporter| reporter.borrow().error_count())
}

/// Returns the count of buffered warnings for this thread.
#[inline(always)]
pub fn warning_count() -> usize {
  REPORTER.with(|reporter| reporter.borrow().warning_count())
}

/// Returns the total count of buffered diagnostics
/// (errors + warnings) for this thread.
#[inline(always)]
pub fn total_count() -> usize {
  REPORTER.with(|reporter| reporter.borrow().total_count())
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

/// Collects all errors and their type-name detail.
pub fn collect_diagnostics() -> (Vec<Error>, Vec<(Error, TyNames)>) {
  REPORTER.with(|reporter| reporter.borrow_mut().drain_with_details())
}
