use crate::aggregator::ErrorAggregator;

use zo_error::{Error, ErrorKind, Severity};
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
        self.render_error(error, source, filename, &mut colors)?;
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
    source: &str,
    filename: &str,
    colors: &mut ColorGenerator,
  ) -> io::Result<()> {
    let span = error.span();
    let kind = error.kind();
    let range = span_to_range(span, source);

    // Severity drives the visual style: red `Error:` for hard
    // errors, yellow `Warning:` for soft diagnostics, blue
    // `Advice:` for compiler-decision rationale. ariadne owns
    // the colors per `ReportKind`.
    let report_kind = match error.severity() {
      Severity::Error => ReportKind::Error,
      Severity::Warning => ReportKind::Warning,
      Severity::Note => ReportKind::Advice,
    };

    let mut report = Report::build(report_kind, (filename, range.clone()));

    // Add error code if configured. Code derived from
    // `zo-error`'s frozen `id_registry` — phase-position
    // independent, stable across `ErrorKind` reorderings.
    if self.config.show_codes {
      report = report.with_code(format!("E{:04}", kind.code()));
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

    // Add secondary label if the error has two spans.
    if let Some(secondary) = error.secondary_span() {
      let sec_range = span_to_range(secondary, source);
      let sec_color = colors.next();

      report = report.with_label(
        Label::new((filename, sec_range))
          .with_message(secondary_label(kind))
          .with_color(sec_color),
      );
    }

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

/// Converts a byte-offset Span to a char-offset range
/// for ariadne. Multi-byte characters (like `—`) shift
/// byte positions relative to char positions.
///
/// Upstream spans occasionally land inside a multi-byte
/// char (e.g. a `len` that was measured in code points
/// instead of UTF-8 bytes). Slicing `source[..N]` at such
/// a position would panic — so we count by walking
/// `char_indices`, which tolerates any byte offset and
/// clamps to the next char boundary.
fn span_to_range(span: Span, source: &str) -> std::ops::Range<usize> {
  let byte_start = (span.start as usize).min(source.len());
  let byte_end = ((span.start + span.len as u32) as usize).min(source.len());
  let start = char_offset_for_byte(source, byte_start);
  let end = char_offset_for_byte(source, byte_end).max(start);

  start..end
}

/// Returns the count of complete chars that end at or
/// before `byte_pos`. Partial chars (byte_pos strictly
/// inside a multi-byte sequence) don't count — matches the
/// intuition "how many chars did we step through to get
/// here" without panicking on non-boundary offsets.
///
/// Matches `source[..byte_pos].chars().count()` for
/// byte positions that ARE valid char boundaries, but
/// tolerates positions that aren't.
fn char_offset_for_byte(source: &str, byte_pos: usize) -> usize {
  let mut n = 0usize;

  for (i, ch) in source.char_indices() {
    if i + ch.len_utf8() > byte_pos {
      break;
    }

    n += 1;
  }

  n
}

/// Returns the main error message for a given error kind.
/// Shared between the human renderer and `render_json` so
/// the JSON `message` field and the human Error line stay
/// in lockstep — single source of truth per variant.
pub(crate) fn error_message(kind: ErrorKind) -> &'static str {
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
    ErrorKind::UndefinedTypeParam => "Undefined type parameter",
    ErrorKind::MissingDollarPrefix => "Type parameter requires `$` prefix",
    ErrorKind::InvalidReturn => "Invalid return statement",
    ErrorKind::InvalidReturnType => "Invalid return type",
    ErrorKind::InvalidBreak => "Invalid break statement",
    ErrorKind::InvalidContinue => "Invalid continue statement",
    ErrorKind::CyclicDependency => "Cyclic dependency detected",
    ErrorKind::InvalidFieldAccess => "Invalid field access",
    ErrorKind::InvalidMethodCall => "Invalid method call",
    ErrorKind::ArityMismatch => "Arity mismatch",
    ErrorKind::InvalidCast => "Invalid cast",
    ErrorKind::InvalidPattern => "Invalid pattern",
    ErrorKind::NonExhaustiveMatch => "Non-exhaustive `match`",
    ErrorKind::UnreachableCode => "Unreachable code",
    ErrorKind::UnusedVariable => "Unused variable",
    ErrorKind::UnusedFunction => "Unused function",
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
    ErrorKind::MixedLoopBodyForms => {
      "loop body mixes the `=>` and `{ ... }` forms"
    }

    // Module system errors.
    ErrorKind::PackFileNotFound => "Pack file not found",
    ErrorKind::ModuleNotDeclared => "Module not declared in lib.zo",
    ErrorKind::UnresolvedModule => "Unresolved module",
    ErrorKind::CircularImport => "Circular import detected",
    ErrorKind::PrivatePackInLoad => {
      "Pack is private — declare it `pub pack` to load it"
    }
    ErrorKind::PrivateItemInLoad => {
      "Item is private — declare it `pub` to import it"
    }
    ErrorKind::ModuleNotReachable => {
      "Module not reachable through public re-export chain"
    }

    // FFI / `#link` errors.
    ErrorKind::LinkResolutionFailed => {
      "Cannot resolve `#link` — neither system nor vendor library found"
    }

    // Entry-point errors.
    ErrorKind::MissingMainFunction => "`main` function not found",

    // String slice errors (compile-time only in v1).
    ErrorKind::StrSliceRequiresConstBounds => {
      "String slice bounds must be compile-time constants"
    }
    ErrorKind::StrSliceRequiresStr => {
      "String slice requires a compile-time string receiver"
    }
    ErrorKind::StrSliceOutOfBounds => "String slice range is out of bounds",
    ErrorKind::StrSliceInvalidRange => "String slice `lo` must be <= `hi`",

    // Structured-concurrency errors.
    ErrorKind::SpawnOutsideNursery => {
      "`spawn` requires an enclosing `nursery { }` scope"
    }
    ErrorKind::AwaitOnNonTask => "`await` expects a `Task<T>` value",
    ErrorKind::ChannelCapacityNotLiteral => {
      "`channel(N)` capacity must be an integer literal"
    }

    // Repeat-array literal errors.
    ErrorKind::RepeatRequiresKnownLength => {
      "`[v...]` requires a `[N]T` type annotation to provide the length"
    }
    ErrorKind::RepeatLengthMismatch => {
      "repeat count does not match the array's declared length"
    }
    ErrorKind::RepeatCountNotConst => {
      "`[v...n]` count must be an integer literal"
    }

    // `%% serialize.` / `%% deserialize.` derive errors.
    ErrorKind::DeriveUnsupportedField => "field type cannot be derived to JSON",

    ErrorKind::UnsupportedGenericLiteral => {
      "interpolated string / regex literals not yet supported in cross-module \
       generic bodies"
    }
    ErrorKind::CrossModuleGenericTooLarge => {
      "cross-module generic body would push the importing tree past the \
       `u16::MAX` node cap"
    }
    ErrorKind::DuplicateAbstractImpl => {
      "conflicting `apply Abstract for Type` — two modules declared an \
       implementation of the same abstract for the same target type"
    }
    ErrorKind::DuplicatePublicName => {
      "two transitively-loaded modules expose a public item under the \
       same name — rename one of the items or use a selective \
       `load M::(specific_name);` so the consumer picks an explicit \
       winner"
    }
    ErrorKind::BoundNotSatisfied => {
      "a generic call site's concrete type does not satisfy the \
       abstract bound declared on the generic parameter"
    }
    ErrorKind::AbstractInheritanceUnsupported => {
      "abstract inheritance (`abstract X : Y`) is not supported — \
       abstracts are flat single-level declarations"
    }
    // Rationale notes (severity = Note, emitted only with
    // `--explain-decisions`).
    ErrorKind::DeadCodeEliminated => "dead code eliminated",
    ErrorKind::UnreachableMatchArm => "unreachable `match` arm",

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
    ErrorKind::UndefinedTypeParam => {
      "not declared in the type parameter list `<$T, ...>`"
    }
    ErrorKind::MissingDollarPrefix => {
      "type parameters must start with `$`, e.g. `<$T>`"
    }
    ErrorKind::ValRequiresConstantInit => {
      "initializer is not a compile-time constant"
    }
    ErrorKind::InvalidFieldAccess => "field not found on this type",
    ErrorKind::InvalidMethodCall => "method not found on this type",
    ErrorKind::ArityMismatch => "wrong number of arguments",
    ErrorKind::InvalidCast => "invalid cast between these types",
    ErrorKind::InvalidPattern => "invalid pattern here",
    ErrorKind::NonExhaustiveMatch => {
      "this `match` does not cover every possible value — \
       add the missing arms or a `_` wildcard"
    }
    ErrorKind::UnreachableCode => "this code will never execute",
    ErrorKind::UnusedVariable => "variable is never used",
    ErrorKind::UnusedFunction => "function is never called",
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
    ErrorKind::MixedLoopBodyForms => {
      "drop the `=>` (block form) or replace `{ ... }` with a single expression (line form)"
    }
    ErrorKind::UnterminatedChar => "unterminated character",
    ErrorKind::InvalidEscapeSequence => "unknown escape code",
    ErrorKind::EmptyCharLiteral | ErrorKind::EmptyCharLit => "empty here",
    ErrorKind::InvalidReturnType => "invalid return type",
    ErrorKind::UnterminatedBytes => "unterminated byte literal",

    // Structured-concurrency errors.
    ErrorKind::SpawnOutsideNursery => "no enclosing `nursery` for this spawn",
    ErrorKind::AwaitOnNonTask => "this is not a `Task<T>`",
    ErrorKind::ChannelCapacityNotLiteral => "expected an integer literal here",

    // FFI / `#link` errors.
    ErrorKind::LinkResolutionFailed => "library not found at this path",

    // Entry-point errors.
    ErrorKind::MissingMainFunction => {
      "expected `fun main() { ... }` somewhere in this file"
    }

    // Abstract bound errors.
    ErrorKind::BoundNotSatisfied => {
      "this value's type has no `apply <Abstract> for <Type>` impl"
    }
    ErrorKind::AbstractInheritanceUnsupported => {
      "drop the `: ParentAbstract` — abstracts cannot inherit"
    }

    // Rationale notes.
    ErrorKind::DeadCodeEliminated => {
      "this function is never reached from `main` and was removed"
    }
    ErrorKind::UnreachableMatchArm => "earlier arms already cover this case",

    _ => "here",
  }
}

/// Returns a help message for the error.
/// Label for the secondary span (e.g., the opening delimiter).
fn secondary_label(kind: ErrorKind) -> &'static str {
  match kind {
    ErrorKind::MismatchedDelimiter => "opened here",
    ErrorKind::UnmatchedOpeningDelimiter => {
      "this closing delimiter skipped over it"
    }
    ErrorKind::LinkResolutionFailed => {
      "vendor fallback also missing — expected under `<exe-dir>/../lib/vendor/`"
    }
    ErrorKind::BoundNotSatisfied => "bound declared here on this parameter",
    _ => "related location",
  }
}

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
    ErrorKind::UndefinedTypeParam => {
      Some("Add `$U` to the type parameter list: `<$T, $U>`")
    }
    ErrorKind::MissingDollarPrefix => {
      Some("Use `$T` instead of `T` in the parameter list")
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
    ErrorKind::MixedLoopBodyForms => Some(
      "Block form: `while cond { ... }`. Line form: `while cond => expr`. Pick one — don't mix them as `while cond => { ... }`.",
    ),
    ErrorKind::InvalidEscapeSequence => Some(
      "Check if you have a typo. If you want a literal backslash,\nuse the double escape `\\\\` instead",
    ),
    ErrorKind::EmptyCharLiteral | ErrorKind::EmptyCharLit => Some(
      "If you meant to use a space, try `' '`.\nIf you need an empty sequence of text, use a string `\"\"` instead",
    ),
    ErrorKind::UnterminatedBytes => {
      Some("Close the literal by adding a backtick at the end")
    }
    ErrorKind::MismatchedDelimiter => {
      Some("Change the closing delimiter to match the opening one")
    }
    ErrorKind::InvalidReturnType => {
      Some("Remove the return type or use `fun main() {}`")
    }

    // Structured-concurrency errors.
    ErrorKind::SpawnOutsideNursery => Some(
      "Wrap the call in `nursery { spawn ... }` so the task's lifetime is bound to a parent scope",
    ),
    ErrorKind::AwaitOnNonTask => Some(
      "`await` only unwraps values produced by `spawn`. Check that the awaited expression returns a task handle",
    ),
    ErrorKind::ChannelCapacityNotLiteral => Some(
      "Write the buffer size as a literal, e.g. `channel(4)`. Variable references are post-MVP",
    ),

    ErrorKind::MissingMainFunction => Some(
      "Every zo program needs a `fun main() { ... }` to be runnable. Add one as the entry point",
    ),

    ErrorKind::PrivatePackInLoad => {
      Some("Mark the pack as `pub pack` in `lib.zo` to expose it outside")
    }
    ErrorKind::PrivateItemInLoad => {
      Some("Mark the item as `pub` to expose it outside its module")
    }
    ErrorKind::ModuleNotReachable => Some(
      "Every link in a `pub load` chain must itself be `pub pack` and `pub load`",
    ),

    _ => None,
  }
}

/// Returns a note for the error.
///
/// Notes are Elm/rustc-style attached secondary context —
/// "here's *why* this is happening" prose that complements
/// the primary message and the action-oriented `help`. The
/// human renderer surfaces them via ariadne's `with_note()`;
/// the JSON renderer emits the same string in the
/// `notes: [...]` array so agents see the same context.
pub(crate) fn error_note(kind: ErrorKind) -> Option<&'static str> {
  match kind {
    ErrorKind::NumberTooLarge => {
      Some("The maximum value for integers is 2^64 - 1")
    }
    ErrorKind::EmptyCharLiteral | ErrorKind::EmptyCharLit => Some(
      "A `char` represents exactly one Unicode scalar.\nIt cannot be empty because it must have a value in memory",
    ),
    ErrorKind::InvalidEscapeSequence => Some(
      "A backslash `\\` starts a special sequence like `\\n` (newline).\nzo only supports: `\\n`, `\\r`, `\\t`, `\\\\`, `\\'`, `\\\"`, `\\0`",
    ),
    ErrorKind::UnterminatedBytes => Some(
      "A byte literal represents a single 8-bit value.\nIt must start and end with a backtick",
    ),
    ErrorKind::MismatchedDelimiter | ErrorKind::UnmatchedOpeningDelimiter => {
      Some(
        "Every opening delimiter must be closed by its matching partner.\n`(` with `)`, `[` with `]`, `{` with `}`",
      )
    }
    ErrorKind::InvalidReturnType => Some(
      "The `main` function is the program entry point.\nIt must return unit (no value)",
    ),
    ErrorKind::TypeMismatch => {
      Some("The types of both operands must be compatible")
    }
    ErrorKind::BoundNotSatisfied => Some(
      "Add an `apply <Abstract> for <ConcreteType> { ... }` block,\nor pass a value of a type that already implements the abstract.",
    ),
    ErrorKind::AbstractInheritanceUnsupported => Some(
      "Express the relationship as `apply <Parent> for <Type>` blocks alongside\n\
       the child impl. Each abstract stays a flat single-level declaration.",
    ),

    // Structured-concurrency notes.
    ErrorKind::SpawnOutsideNursery => Some(
      "Per zo's structured-concurrency model, every spawned task\nmust have a lexical parent nursery. Orphan spawns are rejected\nat compile time.",
    ),

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

#[cfg(test)]
mod span_to_range_tests {
  use super::{char_offset_for_byte, span_to_range};

  use zo_span::Span;

  /// Well-formed span over a multi-byte char — `¥` starts at
  /// byte 0, spans 2 bytes. Expected char range is 0..1.
  #[test]
  fn well_formed_multibyte_span() {
    let src = "¥";
    let range = span_to_range(Span { start: 0, len: 2 }, src);

    assert_eq!(range, 0..1);
  }

  /// Pathological span — `len` ends in the middle of a
  /// multi-byte char. Must NOT panic; must clamp. Before the
  /// fix, this produced the exact failure mode reported by
  /// `lit-char-utf8.zo`: "byte index 1 is not a char boundary".
  #[test]
  fn mid_codepoint_end_does_not_panic() {
    let src = "¥";
    let range = span_to_range(Span { start: 0, len: 1 }, src);

    // End clamps to a valid char boundary (>= start). Either 0
    // or 1 is acceptable — the invariant is: no panic, range
    // is well-ordered.
    assert!(range.start <= range.end);
  }

  /// Start ALSO in the middle of a codepoint (rarer but
  /// still possible if an upstream span is wrong on both
  /// sides).
  #[test]
  fn mid_codepoint_start_does_not_panic() {
    let src = "¥z";
    let range = span_to_range(Span { start: 1, len: 1 }, src);

    assert!(range.start <= range.end);
  }

  /// Span that overruns `source.len()` must clamp, not panic.
  #[test]
  fn out_of_bounds_span_does_not_panic() {
    let src = "abc";
    let range = span_to_range(
      Span {
        start: 10,
        len: 100,
      },
      src,
    );

    assert_eq!(range, 3..3);
  }

  /// Empty source — any span must degrade gracefully to
  /// `0..0`.
  #[test]
  fn empty_source() {
    let src = "";
    let range = span_to_range(Span { start: 5, len: 3 }, src);

    assert_eq!(range, 0..0);
  }

  /// `char_offset_for_byte` counts complete chars ending at
  /// or before `byte_pos`. Partial chars (`byte_pos` inside
  /// a multi-byte sequence) don't count — matches the
  /// "how many whole chars have we stepped past" semantic
  /// without panicking on non-boundary offsets.
  /// Anchored to `¥€` (bytes: 0xC2 0xA5 0xE2 0x82 0xAC,
  /// 2 chars).
  #[test]
  fn char_offset_counts_preceding_chars() {
    let src = "¥€";

    assert_eq!(char_offset_for_byte(src, 0), 0);
    assert_eq!(char_offset_for_byte(src, 2), 1);
    assert_eq!(char_offset_for_byte(src, 5), 2);
    // Mid-codepoint — partial char doesn't count.
    assert_eq!(char_offset_for_byte(src, 1), 0);
    assert_eq!(char_offset_for_byte(src, 3), 1);
    assert_eq!(char_offset_for_byte(src, 4), 1);
  }

  /// Behavior on ASCII matches `chars().count()` exactly for
  /// valid byte boundaries — the new walker can't regress
  /// the single-byte path.
  #[test]
  fn ascii_matches_chars_count() {
    let src = "hello";

    for i in 0..=src.len() {
      assert_eq!(char_offset_for_byte(src, i), i);
    }
  }
}
