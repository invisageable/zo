//! Structured fix-its — machine-applicable text edits
//! attached to diagnostics.
//!
//! Unlike `error_help` (human prose), a `FixIt` is a
//! programmatic patch the agent can apply directly to the
//! source: insert N bytes at position P, replace span S
//! with text T, delete span S. The edit is expressed
//! relative to the diagnostic's primary span — no
//! file-wide reasoning required.
//!
//! v1 contract: every `FixIt` is a literal edit (no
//! capture-group templating from the diagnostic site).
//! Templating arrives when a concrete variant needs it.
//!
//! ## Mapping rules
//!
//! | `FixKind`  | Where the edit lands relative to the
//! |            | diagnostic's primary span
//! |------------|---------------------------------------
//! | `Insert`   | `[span.start, span.start)` — zero-length
//! |            | hole at the start of the span. The
//! |            | `text` is inserted there.
//! | `Replace`  | `[span.start, span.end)` — full span.
//! |            | The `text` replaces it.
//! | `Delete`   | `[span.start, span.end)` — full span.
//! |            | The range is removed.
//!
//! An agent applying the fix doesn't need to know which
//! variant produced it; it just needs the kind, the span,
//! and (for non-Delete) the replacement text.

use zo_error::ErrorKind;

/// Edit operation applied to the diagnostic's primary span.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FixKind {
  /// Insert `text` at the span's start position. The span
  /// itself is left untouched.
  Insert,
  /// Replace the span's content with `text`.
  Replace,
  /// Remove the span. `text` is ignored (always `""`).
  Delete,
}

impl FixKind {
  /// Stable wire string for the JSON `kind` field. Closed
  /// enum — adding a `FixKind` variant must update this
  /// match and bump `$schema`.
  #[inline]
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Insert => "insert",
      Self::Replace => "replace",
      Self::Delete => "delete",
    }
  }
}

/// One machine-applicable patch suggestion for a
/// diagnostic. Multiple `FixIt`s per `ErrorKind` are
/// emitted in **most-preferred-first** order: agents that
/// auto-apply pick `fixes[0]`; smarter consumers may rank
/// for themselves.
#[derive(Clone, Copy, Debug)]
pub struct FixIt {
  /// Edit operation.
  pub kind: FixKind,
  /// Literal text to insert or replace with. `""` for
  /// `Delete` (the field is required so the JSON shape
  /// stays uniform — the consumer keys off `kind`).
  pub text: &'static str,
  /// One-line human-readable summary of what this fix does.
  /// Surfaced verbatim in the JSON `description` field for
  /// IDE quick-fix menus. Should fit on one screen line.
  pub description: &'static str,
}

/// Suggested fixes for an `ErrorKind`. Empty slice means
/// "no machine-applicable fix exists yet for this variant"
/// — JSON consumers receive `"fixes": []` and should fall
/// back to `help` prose.
///
/// Coverage is intentionally partial in v1: only diagnostics
/// where a literal Insert/Replace/Delete at the primary
/// span produces a syntactically valid edit are populated.
/// Variants needing capture-group templating (e.g. "rename
/// `<name>` to `_<name>`") stay empty until a templating
/// engine lands.
pub fn fixes_for(kind: ErrorKind) -> &'static [FixIt] {
  match kind {
    // --- Mutability ---
    ErrorKind::ImmutableVariable => &[FixIt {
      kind: FixKind::Insert,
      text: "mut ",
      description: "Declare the variable as mutable with `mut`",
    }],

    // --- Entry point ---
    ErrorKind::MissingMainFunction => &[FixIt {
      kind: FixKind::Insert,
      text: "\nfun main() {\n}\n",
      description: "Add an empty `main` entry point",
    }],

    // --- Missing punctuation: parser ExpectedX variants
    //     where a single-char insertion at the span's start
    //     produces a syntactically-progressing edit.
    ErrorKind::ExpectedSemicolon => &[FixIt {
      kind: FixKind::Insert,
      text: ";",
      description: "Add a semicolon to end the statement",
    }],
    ErrorKind::ExpectedComma => &[FixIt {
      kind: FixKind::Insert,
      text: ",",
      description: "Add a comma to separate items",
    }],
    ErrorKind::ExpectedColon => &[FixIt {
      kind: FixKind::Insert,
      text: ":",
      description: "Add a colon",
    }],
    ErrorKind::ExpectedAssignment => &[FixIt {
      kind: FixKind::Insert,
      text: "=",
      description: "Add `=` to assign a value",
    }],
    ErrorKind::ExpectedArrow => &[FixIt {
      kind: FixKind::Insert,
      text: "->",
      description: "Add `->` for the return type",
    }],
    ErrorKind::ExpectedLParen => &[FixIt {
      kind: FixKind::Insert,
      text: "(",
      description: "Add the opening `(`",
    }],
    ErrorKind::ExpectedRParen => &[FixIt {
      kind: FixKind::Insert,
      text: ")",
      description: "Add the closing `)`",
    }],
    ErrorKind::ExpectedLBrace => &[FixIt {
      kind: FixKind::Insert,
      text: "{",
      description: "Add the opening `{`",
    }],
    ErrorKind::ExpectedRBrace => &[FixIt {
      kind: FixKind::Insert,
      text: "}",
      description: "Add the closing `}`",
    }],
    ErrorKind::ExpectedLBracket => &[FixIt {
      kind: FixKind::Insert,
      text: "[",
      description: "Add the opening `[`",
    }],
    ErrorKind::ExpectedRBracket => &[FixIt {
      kind: FixKind::Insert,
      text: "]",
      description: "Add the closing `]`",
    }],

    // --- Val/imu ---
    //
    // `:=` inference is forbidden for `val`. The primary
    // span sits on the `:=` token, so a literal replace
    // with `=` makes the declaration well-formed (modulo
    // the still-required type annotation — that's the
    // user's prose to supply, not ours to invent).
    ErrorKind::ValRequiresTypeAnnotation => &[FixIt {
      kind: FixKind::Replace,
      text: "=",
      description: "Replace `:=` with `=` (val forbids inference)",
    }],

    // --- Conditions ---
    //
    // `if (cond)` -> `if cond`. The primary span covers the
    // parenthesised condition including the outer parens;
    // replacing it with empty string drops both parens and
    // leaves the inner expression in place... but that
    // requires capture-group templating to extract the
    // inner. Skip for v1.

    // --- Defaults: no fix ---
    _ => &[],
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Acceptance smoke for Phase 5: variants we promised to
  /// cover return non-empty fix arrays. If a future PR
  /// removes a fix, this test breaks loudly.
  #[test]
  fn known_variants_have_fixes() {
    let covered = &[
      ErrorKind::ImmutableVariable,
      ErrorKind::MissingMainFunction,
      ErrorKind::ExpectedSemicolon,
      ErrorKind::ExpectedComma,
      ErrorKind::ExpectedColon,
      ErrorKind::ExpectedAssignment,
      ErrorKind::ExpectedArrow,
      ErrorKind::ExpectedLParen,
      ErrorKind::ExpectedRParen,
      ErrorKind::ExpectedLBrace,
      ErrorKind::ExpectedRBrace,
      ErrorKind::ExpectedLBracket,
      ErrorKind::ExpectedRBracket,
      ErrorKind::ValRequiresTypeAnnotation,
    ];

    for kind in covered {
      let fixes = fixes_for(*kind);

      assert!(
        !fixes.is_empty(),
        "Phase 5: {kind:?} promised at least one FixIt",
      );
    }
  }

  /// Variants outside the covered set return an empty slice
  /// (the consumer-visible `fixes: []`).
  #[test]
  fn uncovered_variants_return_empty() {
    assert!(fixes_for(ErrorKind::DivisionByZero).is_empty());
    assert!(fixes_for(ErrorKind::TypeMismatch).is_empty());
    assert!(fixes_for(ErrorKind::UnexpectedCharacter).is_empty());
  }

  #[test]
  fn fix_kind_wire_strings_are_stable() {
    // Frozen — bumping `$schema` is the only legitimate
    // way to change these.
    assert_eq!(FixKind::Insert.as_str(), "insert");
    assert_eq!(FixKind::Replace.as_str(), "replace");
    assert_eq!(FixKind::Delete.as_str(), "delete");
  }
}
