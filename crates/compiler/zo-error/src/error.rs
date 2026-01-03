use zo_span::Span;

use serde::Serialize;

/// Compact error representation that fits in exactly 8 bytes.
///
/// Layout (64 bits total):
/// - bits 0-15:  ErrorKind (16 bits).
/// - bits 16-47: offset in source (32 bits).
/// - bits 48-63: additional data (16 bits).
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct Error {
  data: u64,
}

impl Error {
  /// Creates a new [`Error`] from a span and a kind.
  #[inline(always)]
  pub const fn new(kind: ErrorKind, span: Span) -> Self {
    let packed =
      (span.start as u64) | ((span.len as u64) << 32) | ((kind as u64) << 48);

    Self { data: packed }
  }

  /// Returns the logical span (start and length) of the error.
  #[inline(always)]
  pub fn span(&self) -> Span {
    let start = (self.data & 0xFFFFFFFF) as u32;
    let len = ((self.data >> 32) & 0xFFFF) as u16;

    Span { start, len }
  }

  /// Returns the error kind.
  #[inline(always)]
  pub fn kind(&self) -> ErrorKind {
    // This is a bit unsafe, but as long as we control Error creation, it's
    // fine. In a production compiler, you might add a check here.
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
  UndefinedLabel,

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

  // linker errors
  LinkerError,

  // Internal compiler errors (bugs in the compiler itself)
  InternalCompilerError,
}
