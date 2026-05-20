use crate::id_registry;

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
  ///
  /// # Safety
  ///
  /// The u16 transmute is sound because every `Error` value
  /// is constructed via [`Error::new`] or
  /// [`Error::with_secondary`], both of which take a
  /// well-typed `ErrorKind` and pack its discriminant into
  /// the high 16 bits of `self.data`. `ErrorKind` is
  /// `#[repr(u16)]`; its discriminants form a contiguous
  /// `0..N` range (no explicit values), so any u16 we read
  /// back is guaranteed to be a valid variant.
  ///
  /// **Do not** call this on an `Error` reconstituted from
  /// untrusted bytes — the `Serialize` derive on `Error` is
  /// for *emitting* diagnostics over the JSON channel; no
  /// `Deserialize` impl exists. If one is added later, it
  /// must validate the high 16 bits before allowing
  /// `kind()` to be called.
  #[inline(always)]
  pub fn kind(&self) -> ErrorKind {
    // SAFETY: see above — high 16 bits always came from a
    // valid `ErrorKind` discriminant via the packed
    // constructors.
    unsafe { std::mem::transmute((self.data >> 48) as u16) }
  }

  /// Returns the diagnostic severity derived from the kind.
  ///
  /// Severity is NOT stored on `Error` — it is computed from
  /// the kind at classification time. This keeps `Error`
  /// at exactly 16 bytes, so the collector's
  /// `[Error; 128]` buffer stays at 2 KiB per thread.
  #[inline(always)]
  pub fn severity(&self) -> Severity {
    severity(self.kind())
  }
}

/// Diagnostic severity. Computed from `ErrorKind` via
/// [`severity`] — never stored on `Error` to preserve its
/// 16-byte packing.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum Severity {
  /// Hard error — fails the build, exit code non-zero.
  Error,
  /// Warning — surfaced in diagnostics but does not fail
  /// the build.
  Warning,
  /// Note — compiler-decision rationale (e.g. "this fn
  /// was DCE'd because no path reaches it from main").
  /// Only emitted when `--explain-decisions` is set;
  /// never fails the build, never surfaces in the default
  /// human render.
  Note,
}

impl Severity {
  /// Frozen wire string for the JSON `severity` field. Closed
  /// enum — adding a variant must update this match and bump
  /// `$schema`.
  #[inline]
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Error => "error",
      Self::Warning => "warning",
      Self::Note => "note",
    }
  }
}

/// Maps an `ErrorKind` to its `Severity`.
///
/// Const so the compiler can collapse it to a jump table.
/// Add new warning kinds to the match arm; everything else
/// defaults to `Severity::Error`.
pub const fn severity(kind: ErrorKind) -> Severity {
  match kind {
    ErrorKind::UnusedVariable
    | ErrorKind::UnusedFunction
    | ErrorKind::UnreachableCode => Severity::Warning,
    ErrorKind::DeadCodeEliminated | ErrorKind::UnreachableMatchArm => {
      Severity::Note
    }
    _ => Severity::Error,
  }
}

impl ErrorKind {
  /// Stable kebab-case identifier — see `id_registry` for
  /// the freeze contract. Bound by agent prompts, doc URLs,
  /// snapshots; the canonical name for this variant.
  #[inline]
  pub const fn id(self) -> &'static str {
    id_registry::id(self)
  }

  /// Numeric display alias for human renderers (rendered as
  /// `E{:04}`). Same freeze contract as [`Self::id`].
  #[inline]
  pub const fn code(self) -> u16 {
    id_registry::code(self)
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
  UnterminatedRegex,
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

  // Indexing errors (appended to preserve existing error codes).
  InvalidIndex,

  // Module system errors.
  PackFileNotFound,
  ModuleNotDeclared,
  UnresolvedModule,
  CircularImport,

  // String slicing errors (compile-time only in v1).
  StrSliceRequiresConstBounds,
  StrSliceRequiresStr,
  StrSliceOutOfBounds,
  StrSliceInvalidRange,

  // Structured-concurrency errors — appended at the end
  // of the enum to preserve the numeric error codes of
  // variants above.
  SpawnOutsideNursery, // `spawn` without enclosing `nursery { }`
  AwaitOnNonTask,      // `await expr` where expr is not `Ty::Task(_)`
  ChannelCapacityNotLiteral, // `channel(N)` with non-literal N

  // Repeat-array literal `[v...]` / `[v...n]` errors.
  // `[v...]` needs `[N]T` annotation to provide N; `[]T`
  // can't drive the count.
  RepeatRequiresKnownLength,
  // `[v...n]` where n disagrees with `[N]T`'s N.
  RepeatLengthMismatch,
  // `[v...n]` where n isn't an integer literal in v1.
  RepeatCountNotConst,

  // Match exhaustiveness — appended at the end to preserve
  // the numeric error codes of variants above.
  NonExhaustiveMatch,

  // FFI / `#link` errors — appended at the end so insert
  // doesn't shift the numeric error codes of variants
  // above. Emitted when a pack's `#link { ... }`
  // declares a host entry but neither the `system` path
  // nor the `vendor` fallback resolves at codegen time.
  // Without this, the failure surfaces as a runtime
  // `dyld: Symbol not found` after the binary already
  // builds and runs.
  LinkResolutionFailed,

  // Emitted by `zo-analyzer` at the top of `analyze` when
  // the entry tree carries no top-level `fun main`
  // declaration. Caught before SIR construction so an
  // entry-point-less program never reaches the executor /
  // codegen / linker — same fail-fast posture rustc's
  // E0601 takes.
  MissingMainFunction,

  // Emitted by the `%% serialize.` / `%% deserialize.`
  // derive synthesizer when a struct or enum-payload
  // field's type can't be lowered to JSON. Span anchored
  // at the field declaration so the user can see exactly
  // which field is blocking the derive.
  DeriveUnsupportedField,

  /// Emitted at body-record time when a `pub apply <Type<$T>>
  /// { fun ... }` body carries a literal kind whose
  /// `LiteralStore` representation requires cross-store
  /// range tables (interpolated strings, regex patterns).
  /// v1 of cross-module generics carries primitive literal
  /// payloads only; bodies using interp / regex stay in-file
  /// until a follow-up handles the range remap.
  UnsupportedGenericLiteral,
  /// Emitted at splice time when grafting a cross-module
  /// generic body would push the importing module's tree
  /// past `u16::MAX` nodes — `NodeHeader.child_start` is
  /// `u16`, so larger offsets silently truncate. v1 caps
  /// hard; widening to `u32` is a separate refactor.
  CrossModuleGenericTooLarge,

  // --- Rationale-channel variants (severity = Note) ---
  //
  // Emitted only when the driver passes `--explain-decisions`.
  // The pass that made the decision (DCE, exhaustiveness
  // prover, …) calls `report_rationale` with one of these
  // kinds; the rationale channel short-circuits to a no-op
  // when the flag is off, so the hot path stays unaffected.
  //
  /// Emitted by `zo-dce` when a function is eliminated
  /// because no reachable path from `main` (or another
  /// root) calls it.
  DeadCodeEliminated,
  /// Emitted by the exhaustiveness prover when a `match`
  /// arm is shadowed by earlier arms. Future hook — variant
  /// reserved so the schema doesn't bump when it lands.
  UnreachableMatchArm,

  // Privacy errors — appended at the end of the enum so
  // insert doesn't shift the numeric error codes of variants
  // above.
  /// `load my_pack::*;` where `my_pack` is declared as
  /// non-pub `pack` in the surrounding `lib.zo`.
  PrivatePackInLoad,
  /// `load M::foo;` (selective) where `foo` is non-pub in
  /// M — present in M's SIR but absent from its `exported`
  /// scope.
  PrivateItemInLoad,
  /// `pub load X::*;` where the re-export chain reaches a
  /// module that isn't `pub pack`-reachable, so the chain
  /// can't be folded into the re-exporter's `exported`
  /// scope.
  ModuleNotReachable,
}
