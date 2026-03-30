use zo_span::Span;

use serde::Serialize;

/// Compact error representation — 16 bytes.
///
/// Primary span (64 bits):
/// - bits 0-31:  start offset (32 bits).
/// - bits 32-47: length (16 bits).
/// - bits 48-63: ErrorKind (16 bits).
///
/// Secondary span (64 bits, optional):
/// - Same layout as primary. `u64::MAX` means absent.
///
/// Used for errors that reference two locations (e.g., mismatched delimiters:
/// opening + closing).
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct Error {
  data: u64,
  extra: u64,
}

impl Error {
  /// Creates a new [`Error`] from a span and a kind.
  #[inline(always)]
  pub const fn new(kind: ErrorKind, span: Span) -> Self {
    let packed =
      (span.start as u64) | ((span.len as u64) << 32) | ((kind as u64) << 48);

    Self {
      data: packed,
      extra: u64::MAX,
    }
  }

  /// Creates an [`Error`] with a secondary span.
  #[inline(always)]
  pub const fn with_secondary(
    kind: ErrorKind,
    span: Span,
    secondary: Span,
  ) -> Self {
    let packed =
      (span.start as u64) | ((span.len as u64) << 32) | ((kind as u64) << 48);

    let extra = (secondary.start as u64) | ((secondary.len as u64) << 32);

    Self {
      data: packed,
      extra,
    }
  }

  /// Returns the logical span (start and length) of the error.
  #[inline(always)]
  pub fn span(&self) -> Span {
    let start = (self.data & 0xFFFFFFFF) as u32;
    let len = ((self.data >> 32) & 0xFFFF) as u16;

    Span { start, len }
  }

  /// Returns the secondary span, if present.
  #[inline(always)]
  pub fn secondary_span(&self) -> Option<Span> {
    if self.extra == u64::MAX {
      return None;
    }

    let start = (self.extra & 0xFFFFFFFF) as u32;
    let len = ((self.extra >> 32) & 0xFFFF) as u16;

    Some(Span { start, len })
  }

  /// Returns the error kind.
  #[inline(always)]
  pub fn kind(&self) -> ErrorKind {
    unsafe { std::mem::transmute((self.data >> 48) as u16) }
  }
}

/// Error kinds for tokenizer and parser stages.
#[repr(u16)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum ErrorKind {
  // Catch-all for unexpected situations.
  UnexpectedCharacter,

  // tokenizer errors
  UnterminatedString,
  UnterminatedBlockComment,
  InvalidNumericLiteral,
  InvalidEscapeSequence,
  UnterminatedChar,
  UnterminatedBytes,
  EmptyCharLiteral,
  EmptyCharLit,
  EmptyBytesLit,
  InvalidCharLiteral,
  InvalidBinaryLiteral,
  InvalidOctalLiteral,
  InvalidHexLiteral,
  NumberTooLarge,
  InvalidByteSequence,
  UnterminatedRawString,
  InvalidTemplateToken,
  UnexpectedEof,
  UnmatchedOpeningDelimiter,
  UnmatchedClosingDelimiter,
  MismatchedDelimiter,

  // Parser errors
  UnexpectedToken,
  ExpectedIdentifier,
  ExpectedType,
  ExpectedExpression,
  ExpectedStatement,
  ExpectedPattern,
  InvalidTopLevelItem,
  InvalidFunctionSignature,
  InvalidTemplate,
  ExpectedTemplate,
  MismatchedTags,
  ExpectedAttributeValue,
  ExpectedClosureBody,
  ExpectedToken,
  ParserInfiniteLoop,
  UnclosedElement,
  UnclosedFragment,
  InvalidAttributeValue,

  // Specific token errors
  ExpectedInteger,
  ExpectedFloat,
  ExpectedBoolean,
  ExpectedString,
  ExpectedChar,
  ExpectedBytes,
  ExpectedAssignment, // =
  ExpectedLParen,     // (
  ExpectedRParen,     // )
  ExpectedLBrace,     // {
  ExpectedRBrace,     // }
  ExpectedLBracket,   // [
  ExpectedRBracket,   // ]
  ExpectedSemicolon,  // ;
  ExpectedComma,      // ,
  ExpectedColon,      // :
  ExpectedArrow,      // ->
  ExpectedPrefix,     // prefix operators
  ExpectedPostfix,    // postfix operators

  // Semantic analysis errors
  DuplicateDefinition,
  UndefinedVariable,
  UndefinedType,
  UndefinedFunction,
  TypeMismatch,
  InfiniteType,      // Occurs check failed (α = List<α>)
  ArraySizeMismatch, // Array sizes don't match
  ArgumentCountMismatch,
  InvalidAssignment,
  ImmutableVariable,
  InvalidReturn,
  InvalidReturnType,
  InvalidBreak,
  InvalidContinue,
  CyclicDependency,
  InvalidFieldAccess,
  InvalidMethodCall,
  ArityMismatch,
  InvalidCast,
  InvalidPattern,
  UnreachableCode,
  UninitializedVariable,
  InvalidSelfReference,
  InvalidTypeAnnotation,
  ExpectedTypeAnnotation,
  UndefinedLabel,

  // val (compile-time constant) errors
  ValRequiresTypeAnnotation,
  ValRequiresConstantInit,

  // Generic type parameter errors
  UndefinedTypeParam,
  MissingDollarPrefix,

  // Constant folding errors
  DivisionByZero,
  RemainderByZero,
  IntegerOverflow,
  ShiftAmountTooLarge,
  NegativeShiftAmount,
  FloatNaN,
  FloatInfinity,
  InvalidConstantOperation,

  // Code generation errors
  StackUnderflow,
  UnknownLocal,
  UnresolvedJump,
  CraneliftError,

  // Syntax style errors
  ParenthesizedCondition,

  // linker errors
  LinkerError,

  // Internal compiler errors (bugs in the compiler itself)
  InternalCompilerError,

  // DCE warnings (appended to preserve existing error codes).
  UnusedVariable,
  UnusedFunction,
}
