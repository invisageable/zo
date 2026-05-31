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

/// Dynamic, per-diagnostic detail a static
/// `fn(ErrorKind) -> &str` table can't express. The compact
/// `Error` carries none of it; it rides in a side store
/// keyed by the `Error`.
#[derive(Clone, Debug)]
pub enum Detail {
  /// The two conflicting type names of a mismatch.
  Types(TyNames),
  /// Closest in-scope name for an undefined name (a typo) —
  /// `count` for `cont`.
  Suggestion(Box<str>),
  /// A call passed the wrong number of arguments. Carries the
  /// callee name, the expected/given counts, and the callee's
  /// rendered signature for the help.
  ArgCount {
    callee: Box<str>,
    expected: u16,
    given: u16,
    signature: Box<str>,
  },
  /// An argument's type doesn't match the parameter. Carries
  /// the callee name, the `found`/`expected` type names, and
  /// the callee's rendered signature for the help.
  ArgType {
    callee: Box<str>,
    found: Box<str>,
    expected: Box<str>,
    signature: Box<str>,
  },
  /// A non-unit function falls off its end without returning a
  /// value — it implicitly returns unit. Primary caret on the
  /// function (`found`, implicitly `unit`), secondary on the
  /// declared return type (`expected`).
  ReturnType { found: Box<str>, expected: Box<str> },
  /// A value is produced where the function has no return type,
  /// so it's discarded. Carries the value's type (`found`).
  /// Primary caret on the value, secondary on the function name.
  DiscardedValue { found: Box<str> },
}

impl Detail {
  /// Whether this detail's own labels already explain the
  /// mismatch, making the generic per-kind note redundant. The
  /// human renderer and the JSON encoder share this rule so it
  /// can't drift between them.
  pub fn suppresses_note(&self) -> bool {
    matches!(
      self,
      Detail::ArgType { .. }
        | Detail::ReturnType { .. }
        | Detail::DiscardedValue { .. }
    )
  }

  /// Primary-caret label — the claim about the offending span.
  /// `None` defers to the per-kind label (`Suggestion` carries
  /// no claim of its own; the typo's kind speaks for it).
  pub fn primary_label(&self) -> Option<String> {
    Some(match self {
      Detail::ReturnType { .. } => "returns no value".to_owned(),
      Detail::DiscardedValue { .. } => {
        "this function has no return type".to_owned()
      }
      Detail::ArgType {
        found, expected, ..
      } => format!("expected `{expected}`, found `{found}`"),
      Detail::Types(names) => {
        format!("incompatible type `{}` here", names.primary)
      }
      Detail::ArgCount {
        expected, given, ..
      } => format!("expected {expected} arguments, found {given}"),
      Detail::Suggestion(_) => return None,
    })
  }

  /// Secondary-caret label — the value the primary conflicts
  /// with. `None` defers to the per-kind secondary label.
  pub fn secondary_label(&self) -> Option<String> {
    match self {
      Detail::ReturnType { expected, .. } => {
        Some(format!("expected `{expected}`"))
      }
      Detail::DiscardedValue { found } => {
        Some(format!("this `{found}` is discarded"))
      }
      Detail::Types(names) => {
        Some(format!("conflicts with this type `{}`", names.secondary))
      }
      _ => None,
    }
  }

  /// Help text — the resolution. `None` defers to the per-kind
  /// help prose.
  pub fn help(&self) -> Option<String> {
    match self {
      Detail::Suggestion(name) => Some(format!("did you mean `{name}`?")),
      Detail::ArgCount {
        callee, signature, ..
      }
      | Detail::ArgType {
        callee, signature, ..
      } => Some(format!("match `{callee}`'s signature: `{signature}`")),
      Detail::ReturnType { expected, .. } => {
        Some(format!("return a value of type `{expected}` from the body"))
      }
      Detail::DiscardedValue { found } => Some(format!(
        "declare `-> {found}` to return it, or drop the value"
      )),
      Detail::Types(_) => None,
    }
  }
}

/// Thread-local error reporter with fixed-size buffer.
/// This provides zero-allocation error collection during compilation.
pub struct ThreadLocalReporter {
  /// Fixed-size array for errors.
  errors: [Error; MAX_ERRORS],
  /// Current number of errors.
  count: usize,
  /// Side store of dynamic detail, keyed by the `Error` it
  /// annotates. A `Vec` (not a `HashMap`) so `new` stays
  /// `const`; lookups happen only on the cold render path.
  details: Vec<(Error, Detail)>,
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

  /// Reports an error and attaches dynamic detail.
  pub fn report_with_detail(&mut self, error: Error, detail: Detail) -> bool {
    if self.report(error) {
      self.details.push((error, detail));

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

  /// Drains errors together with their dynamic detail.
  pub fn drain_with_details(&mut self) -> (Vec<Error>, Vec<(Error, Detail)>) {
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
  report_error_with_detail(error, Detail::Types(names))
}

/// Reports an undefined-name error with the closest in-scope
/// name as a suggestion.
pub fn report_error_with_suggestion(error: Error, name: &str) -> bool {
  report_error_with_detail(error, Detail::Suggestion(name.into()))
}

/// Reports an error and attaches dynamic detail.
pub fn report_error_with_detail(error: Error, detail: Detail) -> bool {
  REPORTER
    .with(|reporter| reporter.borrow_mut().report_with_detail(error, detail))
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
pub fn collect_diagnostics() -> (Vec<Error>, Vec<(Error, Detail)>) {
  REPORTER.with(|reporter| reporter.borrow_mut().drain_with_details())
}
