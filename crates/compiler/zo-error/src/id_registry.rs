//! Stable identifier + numeric-code registry for every
//! `ErrorKind` variant.
//!
//! ## Stability contract
//!
//! Once a variant ships, **its `id` (kebab-case string) and
//! `code` (`u16` display alias) are frozen forever.** Agent
//! prompts, doc URLs, IDE quick-fix maps, and downstream
//! test snapshots all bind to these values.
//!
//! ### Adding a new variant
//!
//! Append a new arm to `entry()` with:
//! 1. A unique kebab-case `id` (no namespace prefix — `phase`
//!    travels in its own field).
//! 2. The next unused `code` integer **within the variant's
//!    phase range** (see ranges below).
//!
//! The match is exhaustive — the compiler refuses to build
//! when an `ErrorKind` variant ships without a registry entry.
//!
//! ### Renaming a variant
//!
//! Don't. If the kebab-case `id` needs to change, add a new
//! `ErrorKind` variant with the new id, leave the old one
//! emitting the same code under the old id, and route emission
//! through the new variant. After two releases, the old
//! variant can be deleted; the code stays reserved
//! permanently (record in `RESERVED_CODES` below).
//!
//! ## Numeric code ranges (display only)
//!
//! Codes group by phase for human readability — the agent
//! consumes `phase` as a separate field and never has to
//! decode the integer.
//!
//! | Range         | Phase                          |
//! |---------------|--------------------------------|
//! | E0001 .. E0099 | Tokenizer                     |
//! | E0100 .. E0299 | Parser                        |
//! | E0300 .. E0499 | Analyzer (semantic + types)   |
//! | E0500 .. E0599 | Constants & arithmetic        |
//! | E0600 .. E0699 | Codegen / Linker / Internal   |
//! | E0700 .. E0799 | Modules / FFI / Concurrency   |
//! | E0800 .. E0899 | Entry point & misc            |
//! | E0900 .. E0999 | Rationale notes (severity=Note) |

use crate::error::ErrorKind;

/// Single source of truth for `(id, code)` per variant.
/// Every `ErrorKind` arm appears exactly once; the compiler
/// enforces exhaustiveness.
const fn entry(kind: ErrorKind) -> (&'static str, u16) {
  match kind {
    // --- Tokenizer (E0001 .. E0099) ---
    ErrorKind::UnexpectedCharacter => ("unexpected-character", 1),
    ErrorKind::UnterminatedString => ("unterminated-string", 2),
    ErrorKind::UnterminatedBlockComment => ("unterminated-block-comment", 3),
    ErrorKind::InvalidNumericLiteral => ("invalid-numeric-literal", 4),
    ErrorKind::InvalidEscapeSequence => ("invalid-escape-sequence", 5),
    ErrorKind::UnterminatedChar => ("unterminated-char", 6),
    ErrorKind::UnterminatedBytes => ("unterminated-bytes", 7),
    ErrorKind::EmptyCharLiteral => ("empty-char-literal", 8),
    ErrorKind::EmptyCharLit => ("empty-char-lit", 9),
    ErrorKind::EmptyBytesLit => ("empty-bytes-lit", 10),
    ErrorKind::InvalidCharLiteral => ("invalid-char-literal", 11),
    ErrorKind::InvalidBinaryLiteral => ("invalid-binary-literal", 12),
    ErrorKind::InvalidOctalLiteral => ("invalid-octal-literal", 13),
    ErrorKind::InvalidHexLiteral => ("invalid-hex-literal", 14),
    ErrorKind::NumberTooLarge => ("number-too-large", 15),
    ErrorKind::InvalidByteSequence => ("invalid-byte-sequence", 16),
    ErrorKind::UnterminatedRawString => ("unterminated-raw-string", 17),
    ErrorKind::InvalidTemplateToken => ("invalid-template-token", 18),
    ErrorKind::UnexpectedEof => ("unexpected-eof", 19),
    ErrorKind::UnmatchedOpeningDelimiter => ("unmatched-opening-delimiter", 20),
    ErrorKind::UnmatchedClosingDelimiter => ("unmatched-closing-delimiter", 21),
    ErrorKind::MismatchedDelimiter => ("mismatched-delimiter", 22),
    ErrorKind::UnterminatedRegex => ("unterminated-regex", 23),

    // --- Parser (E0100 .. E0299) ---
    ErrorKind::UnexpectedToken => ("unexpected-token", 100),
    ErrorKind::ExpectedIdentifier => ("expected-identifier", 101),
    ErrorKind::ExpectedType => ("expected-type", 102),
    ErrorKind::ExpectedExpression => ("expected-expression", 103),
    ErrorKind::ExpectedStatement => ("expected-statement", 104),
    ErrorKind::ExpectedPattern => ("expected-pattern", 105),
    ErrorKind::InvalidTopLevelItem => ("invalid-top-level-item", 106),
    ErrorKind::InvalidFunctionSignature => ("invalid-function-signature", 107),
    ErrorKind::InvalidTemplate => ("invalid-template", 108),
    ErrorKind::ExpectedTemplate => ("expected-template", 109),
    ErrorKind::MismatchedTags => ("mismatched-tags", 110),
    ErrorKind::ExpectedAttributeValue => ("expected-attribute-value", 111),
    ErrorKind::ExpectedClosureBody => ("expected-closure-body", 112),
    ErrorKind::ExpectedToken => ("expected-token", 113),
    ErrorKind::ParserInfiniteLoop => ("parser-infinite-loop", 114),
    ErrorKind::UnclosedElement => ("unclosed-element", 115),
    ErrorKind::UnclosedFragment => ("unclosed-fragment", 116),
    ErrorKind::InvalidAttributeValue => ("invalid-attribute-value", 117),
    ErrorKind::ExpectedInteger => ("expected-integer", 118),
    ErrorKind::ExpectedFloat => ("expected-float", 119),
    ErrorKind::ExpectedBoolean => ("expected-boolean", 120),
    ErrorKind::ExpectedString => ("expected-string", 121),
    ErrorKind::ExpectedChar => ("expected-char", 122),
    ErrorKind::ExpectedBytes => ("expected-bytes", 123),
    ErrorKind::ExpectedAssignment => ("expected-assignment", 124),
    ErrorKind::ExpectedLParen => ("expected-lparen", 125),
    ErrorKind::ExpectedRParen => ("expected-rparen", 126),
    ErrorKind::ExpectedLBrace => ("expected-lbrace", 127),
    ErrorKind::ExpectedRBrace => ("expected-rbrace", 128),
    ErrorKind::ExpectedLBracket => ("expected-lbracket", 129),
    ErrorKind::ExpectedRBracket => ("expected-rbracket", 130),
    ErrorKind::ExpectedSemicolon => ("expected-semicolon", 131),
    ErrorKind::ExpectedComma => ("expected-comma", 132),
    ErrorKind::ExpectedColon => ("expected-colon", 133),
    ErrorKind::ExpectedArrow => ("expected-arrow", 134),
    ErrorKind::ExpectedPrefix => ("expected-prefix", 135),
    ErrorKind::ExpectedPostfix => ("expected-postfix", 136),
    ErrorKind::ParenthesizedCondition => ("parenthesized-condition", 137),
    ErrorKind::MixedLoopBodyForms => ("mixed-loop-body-forms", 138),
    ErrorKind::ReservedKeyword => ("reserved-keyword", 139),

    // --- Analyzer: semantic + types (E0300 .. E0499) ---
    ErrorKind::DuplicateDefinition => ("duplicate-definition", 300),
    ErrorKind::UndefinedVariable => ("undefined-variable", 301),
    ErrorKind::UndefinedType => ("undefined-type", 302),
    ErrorKind::UndefinedFunction => ("undefined-function", 303),
    ErrorKind::TypeMismatch => ("type-mismatch", 304),
    ErrorKind::InfiniteType => ("infinite-type", 305),
    ErrorKind::ArraySizeMismatch => ("array-size-mismatch", 306),
    ErrorKind::ArgumentCountMismatch => ("argument-count-mismatch", 307),
    ErrorKind::InvalidAssignment => ("invalid-assignment", 308),
    ErrorKind::ImmutableVariable => ("immutable-variable", 309),
    ErrorKind::UseAfterMove => ("use-after-move", 350),
    ErrorKind::DoubleFree => ("double-free", 351),
    ErrorKind::ConditionalMove => ("conditional-move", 352),
    ErrorKind::InvalidReturn => ("invalid-return", 310),
    ErrorKind::InvalidReturnType => ("invalid-return-type", 311),
    ErrorKind::InvalidBreak => ("invalid-break", 312),
    ErrorKind::InvalidContinue => ("invalid-continue", 313),
    ErrorKind::CyclicDependency => ("cyclic-dependency", 314),
    ErrorKind::InvalidFieldAccess => ("invalid-field-access", 315),
    ErrorKind::InvalidMethodCall => ("invalid-method-call", 316),
    ErrorKind::ArityMismatch => ("arity-mismatch", 317),
    ErrorKind::InvalidCast => ("invalid-cast", 318),
    ErrorKind::InvalidPattern => ("invalid-pattern", 319),
    ErrorKind::UnreachableCode => ("unreachable-code", 320),
    ErrorKind::UninitializedVariable => ("uninitialized-variable", 321),
    ErrorKind::InvalidSelfReference => ("invalid-self-reference", 322),
    ErrorKind::InvalidTypeAnnotation => ("invalid-type-annotation", 323),
    ErrorKind::ExpectedTypeAnnotation => ("expected-type-annotation", 324),
    ErrorKind::UndefinedLabel => ("undefined-label", 325),
    ErrorKind::ValRequiresTypeAnnotation => {
      ("val-requires-type-annotation", 326)
    }
    ErrorKind::ValRequiresConstantInit => ("val-requires-constant-init", 327),
    ErrorKind::UndefinedTypeParam => ("undefined-type-param", 328),
    ErrorKind::MissingDollarPrefix => ("missing-dollar-prefix", 329),
    ErrorKind::UnusedVariable => ("unused-variable", 330),
    ErrorKind::UnusedFunction => ("unused-function", 331),
    ErrorKind::InvalidIndex => ("invalid-index", 332),
    ErrorKind::NonExhaustiveMatch => ("non-exhaustive-match", 333),
    ErrorKind::StrSliceRequiresConstBounds => {
      ("str-slice-requires-const-bounds", 334)
    }
    ErrorKind::StrSliceRequiresStr => ("str-slice-requires-str", 335),
    ErrorKind::StrSliceOutOfBounds => ("str-slice-out-of-bounds", 336),
    ErrorKind::StrSliceInvalidRange => ("str-slice-invalid-range", 337),
    ErrorKind::RepeatRequiresKnownLength => {
      ("repeat-requires-known-length", 338)
    }
    ErrorKind::RepeatLengthMismatch => ("repeat-length-mismatch", 339),
    ErrorKind::RepeatCountNotConst => ("repeat-count-not-const", 340),
    ErrorKind::DeriveUnsupportedField => ("derive-unsupported-field", 341),
    ErrorKind::UnsupportedGenericLiteral => {
      ("unsupported-generic-literal", 342)
    }
    ErrorKind::CrossModuleGenericTooLarge => {
      ("cross-module-generic-too-large", 343)
    }
    ErrorKind::DuplicateAbstractImpl => ("duplicate-abstract-impl", 344),
    ErrorKind::DuplicatePublicName => ("duplicate-public-name", 345),
    ErrorKind::BoundNotSatisfied => ("bound-not-satisfied", 347),
    ErrorKind::AbstractInheritanceUnsupported => {
      ("abstract-inheritance-unsupported", 348)
    }
    ErrorKind::AbstractNotDynSafe => ("abstract-not-dyn-safe", 349),
    ErrorKind::NonPascalCaseName => ("non-pascal-case-name", 353),
    ErrorKind::NonScreamingCaseName => ("non-screaming-case-name", 354),
    ErrorKind::NonSnakeCaseName => ("non-snake-case-name", 355),
    ErrorKind::CircularComponent => ("circular-component", 356),
    ErrorKind::EventOnComponent => ("event-on-component", 357),

    // --- Constants & arithmetic (E0500 .. E0599) ---
    ErrorKind::DivisionByZero => ("division-by-zero", 500),
    ErrorKind::RemainderByZero => ("remainder-by-zero", 501),
    ErrorKind::IntegerOverflow => ("integer-overflow", 502),
    ErrorKind::ShiftAmountTooLarge => ("shift-amount-too-large", 503),
    ErrorKind::NegativeShiftAmount => ("negative-shift-amount", 504),
    ErrorKind::FloatNaN => ("float-nan", 505),
    ErrorKind::FloatInfinity => ("float-infinity", 506),
    ErrorKind::InvalidConstantOperation => ("invalid-constant-operation", 507),

    // --- Codegen / Linker / Internal (E0600 .. E0699) ---
    ErrorKind::StackUnderflow => ("stack-underflow", 600),
    ErrorKind::UnknownLocal => ("unknown-local", 601),
    ErrorKind::UnresolvedJump => ("unresolved-jump", 602),
    ErrorKind::CraneliftError => ("cranelift-error", 603),
    ErrorKind::LinkerError => ("linker-error", 604),
    ErrorKind::InternalCompilerError => ("internal-compiler-error", 605),

    // --- Modules / FFI / Concurrency (E0700 .. E0799) ---
    ErrorKind::PackFileNotFound => ("pack-file-not-found", 700),
    ErrorKind::ModuleNotDeclared => ("module-not-declared", 701),
    ErrorKind::UnresolvedModule => ("unresolved-module", 702),
    ErrorKind::CircularImport => ("circular-import", 703),
    ErrorKind::LinkResolutionFailed => ("link-resolution-failed", 704),
    ErrorKind::SpawnOutsideNursery => ("spawn-outside-nursery", 705),
    ErrorKind::AwaitOnNonTask => ("await-on-non-task", 706),
    ErrorKind::ChannelCapacityNotLiteral => {
      ("channel-capacity-not-literal", 707)
    }
    ErrorKind::PrivatePackInLoad => ("private-pack-in-load", 708),
    ErrorKind::PrivateItemInLoad => ("private-item-in-load", 709),
    ErrorKind::ModuleNotReachable => ("module-not-reachable", 710),
    ErrorKind::CapturingClosureAsFnPointer => {
      ("capturing-closure-as-fn-pointer", 711)
    }

    // --- Entry point & misc (E0800 .. E0899) ---
    ErrorKind::MissingMainFunction => ("missing-main-function", 800),
    ErrorKind::TestFnMustBeParameterless => {
      ("test-fn-must-be-parameterless", 801)
    }
    ErrorKind::TestFnMustReturnUnit => ("test-fn-must-return-unit", 802),

    // --- Rationale notes (E0900 .. E0999) ---
    //
    // Severity is `Note`, never `Error`. Emitted only with
    // `--explain-decisions`. The code range is reserved for
    // compiler-decision rationale entries — never reused for
    // hard errors.
    ErrorKind::DeadCodeEliminated => ("dead-code-eliminated", 900),
    ErrorKind::UnreachableMatchArm => ("unreachable-match-arm", 901),
  }
}

/// Stable kebab-case identifier for an `ErrorKind`.
///
/// Frozen contract — see module docs. Bound by agent prompts,
/// docs, snapshots; never renamed without a deprecation path.
#[inline]
pub const fn id(kind: ErrorKind) -> &'static str {
  entry(kind).0
}

/// Numeric display alias for an `ErrorKind`.
///
/// Rendered as `E{:04}` in human output. Same stability
/// contract as [`id`] — append-only, never renumbered.
#[inline]
pub const fn code(kind: ErrorKind) -> u16 {
  entry(kind).1
}
