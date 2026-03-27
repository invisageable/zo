use crate::aggregator::{ErrorAggregator, Phase};

use zo_error::{Error, ErrorKind};
use zo_span::Span;

use ariadne::{ColorGenerator, Label, Report, ReportKind, Source};

use std::io;

/// Configuration for error rendering.
#[derive(Debug, Clone)]
pub struct RenderConfig {
  /// Maximum number of errors to display per phase.
  pub max_errors_per_phase: usize,
  /// Whether to use colored output.
  pub use_colors: bool,
  /// Whether to show error codes.
  pub show_codes: bool,
  /// Whether to show help messages.
  pub show_help: bool,
}

impl Default for RenderConfig {
  fn default() -> Self {
    Self {
      max_errors_per_phase: 10,
      use_colors: true,
      show_codes: true,
      show_help: true,
    }
  }
}

/// Renders errors using ariadne for beautiful output.
pub struct ErrorRenderer {
  config: RenderConfig,
}

impl ErrorRenderer {
  /// Creates a new error renderer with default config.
  pub fn new() -> Self {
    Self {
      config: RenderConfig::default(),
    }
  }

  /// Creates a new error renderer with custom config.
  pub const fn with_config(config: RenderConfig) -> Self {
    Self { config }
  }

  /// Renders all errors from the aggregator to stderr.
  pub fn render(
    &self,
    aggregator: &ErrorAggregator,
    source: &str,
    filename: &str,
  ) -> io::Result<()> {
    let mut colors = ColorGenerator::new();

    for phase_errors in aggregator.errors() {
      // Limit errors per phase
      let errors_to_show = phase_errors
        .errors
        .iter()
        .take(self.config.max_errors_per_phase);

      for error in errors_to_show {
        self.render_error(
          error,
          phase_errors.phase,
          source,
          filename,
          &mut colors,
        )?;
      }

      // Show message if we truncated errors
      if phase_errors.errors.len() > self.config.max_errors_per_phase {
        eprintln!(
          "... and {} more {} errors not shown",
          phase_errors.errors.len() - self.config.max_errors_per_phase,
          phase_errors.phase.name()
        );
      }
    }

    Ok(())
  }

  /// Renders an error.
  fn render_error(
    &self,
    error: &Error,
    phase: Phase,
    source: &str,
    filename: &str,
    colors: &mut ColorGenerator,
  ) -> io::Result<()> {
    let span = error.span();
    let kind = error.kind();
    let range = span_to_range(span);

    let mut report =
      Report::build(ReportKind::Error, (filename, range.clone()));

    // Add error code if configured
    if self.config.show_codes {
      report = report.with_code(format!("E{:04}", error_code(phase, kind)));
    }

    // Set the main error message
    report = report.with_message(error_message(kind));

    // Add the error label at the span location
    // For binary operations, this will point to the operator itself
    let label_msg = match kind {
      ErrorKind::TypeMismatch => "incompatible types for this operation",
      _ => error_label(kind),
    };
    let color = colors.next();
    report = report.with_label(
      Label::new((filename, range.clone()))
        .with_message(label_msg)
        .with_color(color),
    );

    // Add help message if configured
    if self.config.show_help
      && let Some(help) = error_help(kind)
    {
      report = report.with_help(help);
    }

    // Add notes for specific error kinds
    if let Some(note) = error_note(kind) {
      report = report.with_note(note);
    }

    // Finish and print the report
    let report = report.finish();

    report.eprint((filename, Source::from(source)))?;

    Ok(())
  }
}

/// Converts a Span to a range for ariadne.
fn span_to_range(span: Span) -> std::ops::Range<usize> {
  span.start as usize..(span.start + span.len as u32) as usize
}

/// Generates an error code based on phase and kind.
fn error_code(phase: Phase, kind: ErrorKind) -> u16 {
  let phase_offset = match phase {
    Phase::Tokenizer => 0x0000,
    Phase::Parser => 0x0100,
    Phase::Analyzer => 0x0200,
    Phase::Codegen => 0x0300,
    Phase::Runtime => 0x0400,
  };

  // Use discriminant value of the error kind
  phase_offset + (kind as u16 & 0xFF)
}

/// Returns the main error message for a given error kind.
fn error_message(kind: ErrorKind) -> &'static str {
  match kind {
    // Tokenizer errors
    ErrorKind::UnexpectedCharacter => "Unexpected character",
    ErrorKind::UnterminatedString => "Unterminated string literal",
    ErrorKind::UnterminatedBlockComment => "Unterminated block comment",
    ErrorKind::InvalidNumericLiteral => "Invalid numeric literal",
    ErrorKind::InvalidEscapeSequence => "Invalid escape sequence",
    ErrorKind::UnterminatedChar => "Unterminated character literal",
    ErrorKind::UnterminatedBytes => "Unterminated bytes literal",
    ErrorKind::EmptyCharLiteral => "Empty character literal",
    ErrorKind::EmptyCharLit => "Empty character literal",
    ErrorKind::EmptyBytesLit => "Empty bytes literal",
    ErrorKind::InvalidCharLiteral => "Invalid character literal",
    ErrorKind::InvalidBinaryLiteral => "Invalid binary literal",
    ErrorKind::InvalidOctalLiteral => "Invalid octal literal",
    ErrorKind::InvalidHexLiteral => "Invalid hexadecimal literal",
    ErrorKind::NumberTooLarge => "Number literal too large",
    ErrorKind::InvalidByteSequence => "Invalid byte sequence",
    ErrorKind::UnterminatedRawString => "Unterminated raw string literal",
    ErrorKind::InvalidTemplateToken => "Invalid template token",
    ErrorKind::UnexpectedEof => "Unexpected end of file",

    // Delimiter errors
    ErrorKind::UnmatchedOpeningDelimiter => "Unmatched opening delimiter",
    ErrorKind::UnmatchedClosingDelimiter => "Unmatched closing delimiter",
    ErrorKind::MismatchedDelimiter => "Mismatched delimiter",

    // Parser errors
    ErrorKind::UnexpectedToken => "Unexpected token",
    ErrorKind::ExpectedIdentifier => "Expected identifier",
    ErrorKind::ExpectedType => "Expected type annotation",
    ErrorKind::ExpectedExpression => "Expected expression",
    ErrorKind::ExpectedStatement => "Expected statement",
    ErrorKind::ExpectedPattern => "Expected pattern",
    ErrorKind::InvalidTopLevelItem => "Invalid top-level item",
    ErrorKind::InvalidFunctionSignature => "Invalid function signature",
    ErrorKind::InvalidTemplate => "Invalid template",
    ErrorKind::ExpectedTemplate => "Expected template",
    ErrorKind::MismatchedTags => "Mismatched tags",
    ErrorKind::ExpectedAttributeValue => "Expected attribute value",
    ErrorKind::ExpectedClosureBody => "Expected closure body",
    ErrorKind::ExpectedToken => "Expected token",
    ErrorKind::ParserInfiniteLoop => "Parser infinite loop detected",
    ErrorKind::UnclosedElement => "Unclosed element",
    ErrorKind::UnclosedFragment => "Unclosed fragment",
    ErrorKind::InvalidAttributeValue => "Invalid attribute value",

    // Specific token errors
    ErrorKind::ExpectedInteger => "Expected integer literal",
    ErrorKind::ExpectedFloat => "Expected float literal",
    ErrorKind::ExpectedBoolean => "Expected boolean literal",
    ErrorKind::ExpectedString => "Expected string literal",
    ErrorKind::ExpectedChar => "Expected character literal",
    ErrorKind::ExpectedBytes => "Expected bytes literal",
    ErrorKind::ExpectedAssignment => "Expected assignment operator (=)",
    ErrorKind::ExpectedLParen => "Expected opening parenthesis '('",
    ErrorKind::ExpectedRParen => "Expected closing parenthesis ')'",
    ErrorKind::ExpectedLBrace => "Expected opening brace '{'",
    ErrorKind::ExpectedRBrace => "Expected closing brace '}'",
    ErrorKind::ExpectedLBracket => "Expected opening bracket '['",
    ErrorKind::ExpectedRBracket => "Expected closing bracket ']'",
    ErrorKind::ExpectedSemicolon => "Expected semicolon ';'",
    ErrorKind::ExpectedComma => "Expected comma ','",
    ErrorKind::ExpectedColon => "Expected colon ':'",
    ErrorKind::ExpectedArrow => "Expected arrow '->'",
    ErrorKind::ExpectedPrefix => "Expected prefix operator",
    ErrorKind::ExpectedPostfix => "Expected postfix operator",

    // Semantic errors
    ErrorKind::DuplicateDefinition => "Duplicate definition",
    ErrorKind::UndefinedVariable => "Undefined variable",
    ErrorKind::UndefinedType => "Undefined type",
    ErrorKind::UndefinedFunction => "Undefined function",
    ErrorKind::TypeMismatch => "Type mismatch",
    ErrorKind::InfiniteType => "Infinite type (occurs check failed)",
    ErrorKind::ArraySizeMismatch => "Array size mismatch",
    ErrorKind::ArgumentCountMismatch => "Argument count mismatch",
    ErrorKind::InvalidAssignment => "Invalid assignment",
    ErrorKind::ImmutableVariable => "Cannot mutate immutable variable",
    ErrorKind::ValRequiresTypeAnnotation => {
      "`val` requires explicit type annotation"
    }
    ErrorKind::ValRequiresConstantInit => {
      "`val` requires a compile-time constant initializer"
    }
    ErrorKind::InvalidReturn => "Invalid return statement",
    ErrorKind::InvalidBreak => "Invalid break statement",
    ErrorKind::InvalidContinue => "Invalid continue statement",
    ErrorKind::CyclicDependency => "Cyclic dependency detected",
    ErrorKind::InvalidFieldAccess => "Invalid field access",
    ErrorKind::InvalidMethodCall => "Invalid method call",
    ErrorKind::ArityMismatch => "Arity mismatch",
    ErrorKind::InvalidCast => "Invalid cast",
    ErrorKind::InvalidPattern => "Invalid pattern",
    ErrorKind::UnreachableCode => "Unreachable code",
    ErrorKind::UninitializedVariable => "Uninitialized variable",
    ErrorKind::InvalidSelfReference => "Invalid `self` reference",
    ErrorKind::InvalidTypeAnnotation => "Invalid type annotation",
    ErrorKind::UndefinedLabel => "Undefined label",
    ErrorKind::ExpectedTypeAnnotation => {
      "Expected type annotation: use `: Type =` or `:=`"
    }

    // Constant folding errors
    ErrorKind::DivisionByZero => "Division by zero",
    ErrorKind::RemainderByZero => "Remainder by zero",
    ErrorKind::IntegerOverflow => "Integer overflow",
    ErrorKind::ShiftAmountTooLarge => "Shift amount too large",
    ErrorKind::NegativeShiftAmount => "Negative shift amount",
    ErrorKind::FloatNaN => "Floating-point NaN result",
    ErrorKind::FloatInfinity => "Floating-point infinity result",
    ErrorKind::InvalidConstantOperation => "Invalid constant operation",

    // Code generation errors
    ErrorKind::StackUnderflow => "Stack underflow",
    ErrorKind::UnknownLocal => "Unknown local variable",
    ErrorKind::UnresolvedJump => "Unresolved jump target",
    ErrorKind::CraneliftError => "Code generation error",
    ErrorKind::ParenthesizedCondition => {
      "Parentheses are not allowed around conditions"
    }

    _ => "Unknown error",
  }
}

/// Returns a label message for the error location.
fn error_label(kind: ErrorKind) -> &'static str {
  match kind {
    ErrorKind::UnexpectedCharacter => "this character is not valid here",
    ErrorKind::UnterminatedString => "string literal started here",
    ErrorKind::UnmatchedOpeningDelimiter => {
      "this delimiter has no matching closing delimiter"
    }
    ErrorKind::UnmatchedClosingDelimiter => {
      "no matching opening delimiter for this"
    }
    ErrorKind::MismatchedDelimiter => "delimiter type doesn't match opening",
    ErrorKind::ExpectedIdentifier => "expected an identifier here",
    ErrorKind::ExpectedLParen => "expected '(' here",
    ErrorKind::ExpectedRParen => "expected ')' here",
    ErrorKind::ExpectedLBrace => "expected '{' here",
    ErrorKind::ExpectedRBrace => "expected '}' here",
    ErrorKind::ExpectedLBracket => "expected '[' here",
    ErrorKind::ExpectedRBracket => "expected ']' here",
    ErrorKind::ExpectedSemicolon => "expected ';' here",
    ErrorKind::ExpectedComma => "expected ',' here",
    ErrorKind::ExpectedColon => "expected ':' here",
    ErrorKind::ExpectedArrow => "expected '->' here",
    ErrorKind::ExpectedAssignment => "expected '=' here",
    ErrorKind::UndefinedVariable => "variable not found in scope",
    ErrorKind::UndefinedType => "type not found in scope",
    ErrorKind::UndefinedFunction => "function not found in scope",
    ErrorKind::TypeMismatch => "types don't match here",
    ErrorKind::InfiniteType => "type references itself infinitely",
    ErrorKind::ArraySizeMismatch => "array sizes don't match",
    ErrorKind::ArgumentCountMismatch => "wrong number of arguments",
    ErrorKind::DuplicateDefinition => "already defined",
    ErrorKind::ImmutableVariable => "cannot assign to immutable variable",
    ErrorKind::ValRequiresTypeAnnotation => {
      "`val` requires `val x: Type = value`, not `:=`"
    }
    ErrorKind::ValRequiresConstantInit => {
      "initializer is not a compile-time constant"
    }
    ErrorKind::InvalidFieldAccess => "field not found on this type",
    ErrorKind::InvalidMethodCall => "method not found on this type",
    ErrorKind::ArityMismatch => "wrong number of arguments",
    ErrorKind::InvalidCast => "invalid cast between these types",
    ErrorKind::InvalidPattern => "invalid pattern here",
    ErrorKind::UnreachableCode => "this code will never execute",
    ErrorKind::UninitializedVariable => "used before initialization",
    ErrorKind::InvalidSelfReference => "`self` used outside of `apply` block",
    ErrorKind::InvalidTypeAnnotation => "invalid type here",
    ErrorKind::UndefinedLabel => "label not found",
    ErrorKind::CyclicDependency => "cycle detected here",
    ErrorKind::DivisionByZero => "division by zero here",
    ErrorKind::RemainderByZero => "remainder by zero here",
    ErrorKind::IntegerOverflow => "value overflows this type",
    ErrorKind::ShiftAmountTooLarge => "shift exceeds bit width",
    ErrorKind::NegativeShiftAmount => "negative shift amount",
    ErrorKind::FloatNaN => "operation produces NaN",
    ErrorKind::FloatInfinity => "operation produces infinity",
    ErrorKind::InvalidConstantOperation => "cannot evaluate at compile time",
    ErrorKind::StackUnderflow => "stack underflow occurred here",
    ErrorKind::ExpectedTypeAnnotation => {
      "`=` requires a type annotation; use `:=` to infer"
    }
    ErrorKind::ParenthesizedCondition => "remove these parentheses",
    _ => "here",
  }
}

/// Returns a help message for the error.
fn error_help(kind: ErrorKind) -> Option<&'static str> {
  match kind {
    ErrorKind::UnterminatedString => {
      Some("Add a closing quote to terminate the string")
    }
    ErrorKind::ExpectedSemicolon => {
      Some("Add a semicolon ';' to end the statement")
    }
    ErrorKind::UnmatchedOpeningDelimiter => {
      Some("Add the corresponding closing delimiter")
    }
    ErrorKind::UnmatchedClosingDelimiter => {
      Some("Remove this delimiter or add its opening pair")
    }
    ErrorKind::ExpectedIdentifier => {
      Some("Provide a valid identifier (e.g., variable or function name)")
    }
    ErrorKind::ImmutableVariable => {
      Some("Use 'mut' to declare a mutable variable")
    }
    ErrorKind::ValRequiresTypeAnnotation => Some(
      "Use `val X: int = 42;` — `:=` inference is not allowed for constants",
    ),
    ErrorKind::ValRequiresConstantInit => Some(
      "Only literals and constant expressions are allowed: `42`, `3.14`, `\"hello\"`, `true`",
    ),
    ErrorKind::InvalidBreak => Some("'break' can only be used inside a loop"),
    ErrorKind::InvalidContinue => {
      Some("'continue' can only be used inside a loop")
    }
    ErrorKind::InvalidReturn => {
      Some("'return' can only be used inside a function")
    }
    ErrorKind::ExpectedTypeAnnotation => Some(
      "Either add a type: `imu x: int = 42` or use `:=` to infer: `imu x := 42`",
    ),
    ErrorKind::ParenthesizedCondition => {
      Some("Write `if cond {` instead of `if (cond) {`")
    }
    _ => None,
  }
}

/// Returns a note for the error.
fn error_note(kind: ErrorKind) -> Option<&'static str> {
  match kind {
    ErrorKind::NumberTooLarge => {
      Some("The maximum value for integers is 2^64 - 1")
    }
    ErrorKind::EmptyCharLiteral => {
      Some("Character literals must contain exactly one character")
    }
    ErrorKind::TypeMismatch => {
      Some("The types of both operands must be compatible")
    }
    _ => None,
  }
}

impl Default for ErrorRenderer {
  fn default() -> Self {
    Self::new()
  }
}

/// Convenience function to render errors directly to stderr.
pub fn render_errors_to_stderr(
  aggregator: &ErrorAggregator,
  source: &str,
  filename: &str,
) -> io::Result<()> {
  let renderer = ErrorRenderer::new();

  renderer.render(aggregator, source, filename)
}
