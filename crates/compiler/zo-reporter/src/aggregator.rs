use crate::collector;

use zo_error::{Error, ErrorKind};

/// Compilation phase identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
  Tokenizer,
  Parser,
  Analyzer,
  Codegen,
  Runtime,
}
impl Phase {
  /// Returns a human-readable name for the phase.
  pub fn name(&self) -> &'static str {
    match self {
      Phase::Tokenizer => "Tokenizer",
      Phase::Parser => "Parser",
      Phase::Analyzer => "Semantic Analysis",
      Phase::Codegen => "Code Generation",
      Phase::Runtime => "Runtime",
    }
  }
}

/// Errors collected during a specific compilation phase.
#[derive(Debug)]
pub struct PhaseErrors {
  /// The phase these errors belong to.
  pub phase: Phase,
  /// The errors collected during this phase.
  pub errors: Vec<Error>,
}
impl PhaseErrors {
  /// Creates a new PhaseErrors collection.
  pub const fn new(phase: Phase, errors: Vec<Error>) -> Self {
    Self { phase, errors }
  }

  /// Returns the number of errors in this phase.
  pub fn count(&self) -> usize {
    self.errors.len()
  }

  /// Returns true if this phase has no errors.
  pub fn is_empty(&self) -> bool {
    self.errors.is_empty()
  }
}

/// Global error aggregator that collects errors from all phases.
/// This is only used after compilation completes.
#[derive(Debug, Default)]
pub struct ErrorAggregator {
  /// All phase errors collected.
  phase_errors: Vec<PhaseErrors>,
}
impl ErrorAggregator {
  /// Creates a new empty aggregator.
  pub fn new() -> Self {
    Self::default()
  }

  /// Adds errors directly to the aggregator.
  /// The errors are grouped by phase based on their ErrorKind.
  pub fn add_errors(&mut self, errors: &[Error]) {
    self.group_errors_by_phase(errors);
  }

  /// Collects all errors from thread-local storage and groups them by phase.
  /// This should be called once after compilation completes.
  pub fn collect_all(&mut self) {
    self.group_errors_by_phase(&collector::collect_errors());
  }

  fn group_errors_by_phase(&mut self, errors: &[Error]) {
    let mut tokenizer_errors = Vec::new();
    let mut parser_errors = Vec::new();
    let mut analyzer_errors = Vec::new();
    let mut codegen_errors = Vec::new();

    for error in errors {
      let kind = error.kind();
      match kind {
        // Tokenizer errors
        ErrorKind::UnexpectedCharacter
        | ErrorKind::UnterminatedString
        | ErrorKind::UnterminatedBlockComment
        | ErrorKind::InvalidNumericLiteral
        | ErrorKind::InvalidEscapeSequence
        | ErrorKind::UnterminatedChar
        | ErrorKind::EmptyCharLiteral
        | ErrorKind::InvalidCharLiteral
        | ErrorKind::InvalidBinaryLiteral
        | ErrorKind::InvalidOctalLiteral
        | ErrorKind::InvalidHexLiteral
        | ErrorKind::NumberTooLarge
        | ErrorKind::InvalidByteSequence
        | ErrorKind::UnterminatedRawString
        | ErrorKind::InvalidTemplateToken
        | ErrorKind::UnmatchedOpeningDelimiter
        | ErrorKind::UnmatchedClosingDelimiter
        | ErrorKind::MismatchedDelimiter => {
          tokenizer_errors.push(*error);
        }

        // Parser errors
        ErrorKind::UnexpectedToken
        | ErrorKind::ExpectedIdentifier
        | ErrorKind::ExpectedType
        | ErrorKind::ExpectedExpression
        | ErrorKind::ExpectedStatement
        | ErrorKind::ExpectedPattern
        | ErrorKind::InvalidTopLevelItem
        | ErrorKind::InvalidFunctionSignature
        | ErrorKind::InvalidTemplate
        | ErrorKind::ExpectedTemplate
        | ErrorKind::MismatchedTags
        | ErrorKind::ExpectedAttributeValue
        | ErrorKind::ExpectedClosureBody
        | ErrorKind::ExpectedToken
        | ErrorKind::ParserInfiniteLoop
        | ErrorKind::UnclosedElement
        | ErrorKind::UnclosedFragment
        | ErrorKind::InvalidAttributeValue
        | ErrorKind::ExpectedInteger
        | ErrorKind::ExpectedFloat
        | ErrorKind::ExpectedBoolean
        | ErrorKind::ExpectedString
        | ErrorKind::ExpectedChar
        | ErrorKind::ExpectedBytes
        | ErrorKind::ExpectedAssignment
        | ErrorKind::ExpectedLParen
        | ErrorKind::ExpectedRParen
        | ErrorKind::ExpectedLBrace
        | ErrorKind::ExpectedRBrace
        | ErrorKind::ExpectedLBracket
        | ErrorKind::ExpectedRBracket
        | ErrorKind::ExpectedSemicolon
        | ErrorKind::ExpectedComma
        | ErrorKind::ExpectedColon
        | ErrorKind::ExpectedArrow
        | ErrorKind::ExpectedPrefix
        | ErrorKind::ExpectedPostfix => {
          parser_errors.push(*error);
        }

        // Semantic/Analyzer errors
        ErrorKind::DuplicateDefinition
        | ErrorKind::UndefinedVariable
        | ErrorKind::UndefinedType
        | ErrorKind::UndefinedFunction
        | ErrorKind::TypeMismatch
        | ErrorKind::ArgumentCountMismatch
        | ErrorKind::InvalidAssignment
        | ErrorKind::ImmutableVariable
        | ErrorKind::InvalidReturn
        | ErrorKind::InvalidBreak
        | ErrorKind::InvalidContinue
        | ErrorKind::CyclicDependency
        | ErrorKind::InvalidFieldAccess
        | ErrorKind::InvalidMethodCall
        | ErrorKind::ArityMismatch
        | ErrorKind::InvalidCast
        | ErrorKind::InvalidPattern
        | ErrorKind::UnreachableCode
        | ErrorKind::UninitializedVariable
        | ErrorKind::InvalidSelfReference
        | ErrorKind::InvalidTypeAnnotation => {
          analyzer_errors.push(*error);
        }

        // codegen errors.
        ErrorKind::InternalCompilerError => {
          codegen_errors.push(*error);
        }

        _ => {
          // Unknown error, add to analyzer by default
          analyzer_errors.push(*error);
        }
      }
    }

    // Add phase errors if any exist
    if !tokenizer_errors.is_empty() {
      self
        .phase_errors
        .push(PhaseErrors::new(Phase::Tokenizer, tokenizer_errors));
    }
    if !parser_errors.is_empty() {
      self
        .phase_errors
        .push(PhaseErrors::new(Phase::Parser, parser_errors));
    }
    if !analyzer_errors.is_empty() {
      self
        .phase_errors
        .push(PhaseErrors::new(Phase::Analyzer, analyzer_errors));
    }
    if !codegen_errors.is_empty() {
      self
        .phase_errors
        .push(PhaseErrors::new(Phase::Codegen, codegen_errors));
    }
  }

  /// Adds errors from an existing Vec<Error> for a specific phase.
  /// This is useful for integrating with existing error collection.
  pub fn add_phase_errors(&mut self, phase: Phase, errors: Vec<Error>) {
    if !errors.is_empty() {
      self.phase_errors.push(PhaseErrors::new(phase, errors));
    }
  }

  /// Returns the total error count across all phases.
  pub fn total_errors(&self) -> usize {
    self.phase_errors.iter().map(|p| p.count()).sum()
  }

  /// Returns true if there are no errors in any phase.
  pub fn is_empty(&self) -> bool {
    self.phase_errors.is_empty()
  }

  /// Checks if compilation should stop due to critical errors.
  pub fn has_critical_errors(&self) -> bool {
    // Parser errors are critical - can't continue compilation
    self
      .phase_errors
      .iter()
      .any(|p| p.phase == Phase::Parser && !p.errors.is_empty())
  }

  /// Returns errors for a specific phase.
  pub fn phase_errors(&self, phase: Phase) -> Option<&PhaseErrors> {
    self.phase_errors.iter().find(|p| p.phase == phase)
  }

  /// Returns all phase errors in compilation order.
  pub fn errors(&self) -> &[PhaseErrors] {
    &self.phase_errors
  }

  /// Consumes the aggregator and returns all errors as a flat array.
  pub fn into_flat_errors(self) -> Vec<Error> {
    let mut result = Vec::new();

    for phase in self.phase_errors {
      result.extend(phase.errors);
    }

    result
  }

  /// Clears all collected errors.
  pub fn clear(&mut self) {
    self.phase_errors.clear();
    self.phase_errors.shrink_to_fit();
  }

  /// Returns a summary of errors by phase.
  pub fn summary(&self) -> String {
    if self.is_empty() {
      return "No errors found".into();
    }

    let mut summary = format!("Found {} error(s):\n", self.total_errors());

    for phase_errors in &self.phase_errors {
      summary.push_str(&format!(
        "  {}: {} error(s)\n",
        phase_errors.phase.name(),
        phase_errors.count()
      ));
    }

    summary
  }
}
