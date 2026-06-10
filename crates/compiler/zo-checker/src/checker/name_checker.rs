//! Naming-convention checks.

use zo_error::{Error, ErrorKind};
use zo_reporter::report_error_with_rename;
use zo_span::Span;

use swisskit_core::{is, to};

/// Warns when a declared name breaks its site's naming convention.
///
/// @note — types are PascalCase, `val` constants are
/// SCREAMING_SNAKE_CASE, every other binding is snake_case. Each
/// warning carries the convention-correct rename as its fix.
#[derive(Debug, Default)]
pub struct NameChecker;

impl NameChecker {
  /// Checks a type-position name against PascalCase.
  pub fn check_type_name(&self, name: &str, span: Span, file_id: u16) {
    let Some(name) = checkable(name) else { return };

    if !is!(pascal name) {
      report_error_with_rename(
        Error::with_file(ErrorKind::NonPascalCaseName, span, file_id),
        &to!(pascal name),
      );
    }
  }

  /// Checks a `val` constant name against SCREAMING_SNAKE_CASE.
  pub fn check_constant_name(&self, name: &str, span: Span, file_id: u16) {
    let Some(name) = checkable(name) else { return };

    if !is!(snake_screaming name) {
      report_error_with_rename(
        Error::with_file(ErrorKind::NonScreamingCaseName, span, file_id),
        &to!(snake_screaming name),
      );
    }
  }

  /// Checks a binding-position name against snake_case.
  pub fn check_binding_name(&self, name: &str, span: Span, file_id: u16) {
    let Some(name) = checkable(name) else { return };

    if !is!(snake name) {
      report_error_with_rename(
        Error::with_file(ErrorKind::NonSnakeCaseName, span, file_id),
        &to!(snake name),
      );
    }
  }
}

/// The convention-relevant part of a name; `None` opts it out.
///
/// @note — leading underscores (the deliberate-unused marker) and
/// the generic `$` sigil carry no case, so they are stripped; a name
/// of only those characters (`_`, `__`) is exempt.
fn checkable(name: &str) -> Option<&str> {
  let name = name.trim_start_matches(['_', '$']);

  if name.is_empty() { None } else { Some(name) }
}
