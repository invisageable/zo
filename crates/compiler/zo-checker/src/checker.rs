//! The checker pilot — the one entry point the executor drives.

pub mod name_checker;

use name_checker::NameChecker;

use zo_span::Span;

/// Pilots every individual checker.
///
/// @note — the executor owns one and forwards declaration events
/// through these methods; each sub-checker decides whether a warning
/// is due and reports it through `zo-reporter`'s warning channel.
#[derive(Debug, Default)]
pub struct Checker {
  /// Naming-convention checks — PascalCase types,
  /// SCREAMING_SNAKE_CASE constants, snake_case bindings.
  name_checker: NameChecker,
}

impl Checker {
  /// Creates a new [`Checker`] instance.
  pub fn new() -> Self {
    Self::default()
  }

  /// Checks a `struct`/`enum`/`type`/generic name — PascalCase.
  pub fn check_type_name(&self, name: &str, span: Span, file_id: u16) {
    self.name_checker.check_type_name(name, span, file_id);
  }

  /// Checks a `val` constant name — SCREAMING_SNAKE_CASE.
  pub fn check_constant_name(&self, name: &str, span: Span, file_id: u16) {
    self.name_checker.check_constant_name(name, span, file_id);
  }

  /// Checks a binding-position name — snake_case.
  ///
  /// @note — binding positions: `imu`/`mut`, `fun` names and
  /// arguments, struct fields, `abstract` functions.
  pub fn check_binding_name(&self, name: &str, span: Span, file_id: u16) {
    self.name_checker.check_binding_name(name, span, file_id);
  }
}
