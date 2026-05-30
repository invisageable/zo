//! NDJSON renderer for agentic consumers.
//!
//! Emits one JSON object per line on stdout, flushed eagerly
//! so a tail-like consumer (an agent, an IDE wrapper) can
//! react to diagnostics as the compile streams — not after.
//!
//! ## Schema (v1)
//!
//! ```jsonc
//! {
//!   "$schema":  1,                          // bumped on incompatible shape change.
//!   "id":       "missing-main-function",    // frozen kebab-case identity.
//!   "code":     "E0800",                    // display alias, derived from id.
//!   "severity": "error",                    // "error" | "warning"
//!   "phase":    "analyzer",                 // "tokenizer"|"parser"|"analyzer"|"codegen"|"runtime"
//!   "message":  "`main` function not found",
//!   "fixes":    [
//!     {
//!       "kind":        "insert",            // "insert"|"replace"|"delete"
//!       "text":        "\nfun main() {\n}\n",
//!       "description": "Add an empty `main` entry point",
//!       "span": { "file": "foo.zo", "byte_start": 70, "byte_end": 70 }
//!     }
//!   ],
//!   "notes":    [                           // Elm-style attached context.
//!     "A `char` represents exactly one Unicode scalar."
//!   ],
//!   "snippet": {
//!     "before": ["fun baz() {}", "fun qux() {}"],
//!     "lines":  ["fun quux() {}"],
//!     "after":  []
//!   },
//!   "span": {
//!     "file":       "foo.zo",
//!     "byte_start": 70,
//!     "byte_end":   70,
//!     "line_start": 1,
//!     "line_end":   1,
//!     "col_start":  71,
//!     "col_end":    71
//!   }
//! }
//! ```
//!
//! Line and column are **1-indexed** (matches every IDE,
//! `cargo build`, `tsc`, etc.). `col_*` counts UTF-8 chars
//! (not bytes), so `é` advances `col` by 1.
//!
//! `fixes` is **always present** as an array — empty when
//! no machine-applicable fix exists for the variant.
//! Multiple fixes are emitted most-preferred-first; agents
//! auto-applying pick `fixes[0]`.
//!
//! `labels`, `related`, `tree_node` arrive in later phases
//! and only EXTEND the shape — consumers ignore unknown keys.
//!
//! ## Stability contract
//!
//! Same as the id registry: once shipped, `id`, `code`,
//! `severity`, and `phase` are frozen. The `$schema` field
//! is the only escape hatch for incompatible changes.

use crate::aggregator::{ErrorAggregator, Phase};
use crate::fixes::{FixIt, FixKind, fixes_for};
use crate::render::{error_message, error_note};

use zo_error::Error;
use zo_span::Span;

use serde_json::{Map, Value, json};

use std::io::{self, Write};
use std::path::PathBuf;

/// Schema version. Bump on incompatible shape changes
/// (removing a field, renaming a field, narrowing a closed
/// enum). Adding optional fields stays at the same version
/// — consumers ignore unknown keys.
const SCHEMA_VERSION: u32 = 1;

/// Renders every diagnostic in `aggregator` as one NDJSON
/// line per error on the given writer. Each line is flushed
/// individually so a streaming consumer sees diagnostics
/// while the compile is still in flight.
///
/// `source` is the file contents — required to materialise
/// line/col from byte offsets. `filename` is the user-facing
/// name placed in `span.file` (typically the basename).
pub fn to_json<W: Write>(
  aggregator: &ErrorAggregator,
  files: &[(PathBuf, String)],
  snippet_context: usize,
  out: &mut W,
) -> io::Result<()> {
  for phase_errors in aggregator.errors() {
    for error in &phase_errors.errors {
      let (source, filename) = file_for_error(error, files);
      let obj = encode(
        error,
        phase_errors.phase,
        source,
        &filename,
        snippet_context,
      );
      let line = serde_json::to_string(&obj)?;

      writeln!(out, "{line}")?;
      out.flush()?;
    }
  }

  Ok(())
}

/// Convenience: emit NDJSON to stdout. Mirrors
/// `render_errors_to_stderr`'s shape so the driver can
/// dispatch on `--format` with one branch.
pub fn to_stdout(
  aggregator: &ErrorAggregator,
  files: &[(PathBuf, String)],
  snippet_context: usize,
) -> io::Result<()> {
  let stdout = io::stdout();
  let mut handle = stdout.lock();

  to_json(aggregator, files, snippet_context, &mut handle)
}

/// Resolves an error's source text and display filename.
fn file_for_error<'a>(
  error: &Error,
  files: &'a [(PathBuf, String)],
) -> (&'a str, String) {
  let idx = error
    .file_id()
    .map(|id| id as usize)
    .unwrap_or(0)
    .min(files.len().saturating_sub(1));

  let (path, source) = &files[idx];

  let filename = path
    .file_name()
    .map(|n| n.to_string_lossy().into_owned())
    .unwrap_or_else(|| path.to_string_lossy().into_owned());

  (source.as_str(), filename)
}

/// Build the NDJSON object for one diagnostic. With the
/// `preserve_order` feature on `serde_json` (enabled by
/// `zo-provider-json` so user JSON keeps insertion order),
/// `Map` is IndexMap-backed and the wire field order is
/// **the insertion order below** — schema/identity first,
/// then severity/phase, then content (message → fixes →
/// notes → snippet → span). Stable across builds, the
/// determinism precondition for agents diffing diagnostic
/// streams. JSON itself imposes no ordering, but matching
/// bytes are friendlier to byte-level consumers.
fn encode(
  error: &Error,
  phase: Phase,
  source: &str,
  filename: &str,
  snippet_context: usize,
) -> Value {
  let kind = error.kind();
  let span = error.span();
  let byte_start = (span.start as usize).min(source.len());
  let byte_end = (span.end() as usize).min(source.len());
  let snippet = extract_snippet(source, byte_start, byte_end, snippet_context);
  let fixes = encode_fixes(fixes_for(kind), filename, span.start, span.end());

  // Always an array — empty when the kind has no attached
  // note — so consumers never need a presence check.
  let notes: Vec<Value> = match error_note(kind) {
    Some(text) => vec![json!(text)],
    None => Vec::new(),
  };

  let mut obj = Map::with_capacity(11);

  obj.insert("$schema".into(), json!(SCHEMA_VERSION));
  obj.insert("id".into(), json!(kind.id()));
  obj.insert("code".into(), json!(format!("E{:04}", kind.code())));
  obj.insert("severity".into(), json!(error.severity().as_str()));
  obj.insert("phase".into(), json!(phase.as_str()));
  obj.insert("message".into(), json!(error_message(kind)));
  obj.insert("fixes".into(), fixes);
  obj.insert("notes".into(), Value::Array(notes));
  obj.insert(
    "snippet".into(),
    json!({
      "before": snippet.0,
      "lines":  snippet.1,
      "after":  snippet.2,
    }),
  );
  obj.insert("span".into(), full_span_json(filename, span, source));

  // The conflicting value in a type mismatch (the green
  // secondary in the human render). Present only when the
  // diagnostic carries two spans.
  if let Some(secondary) = error.secondary_span() {
    obj.insert(
      "secondary".into(),
      full_span_json(filename, secondary, source),
    );
  }

  Value::Object(obj)
}

/// Full span object — byte offsets plus 1-indexed line/col —
/// for the primary span and the secondary (the value a
/// mismatch conflicts with).
fn full_span_json(filename: &str, span: Span, source: &str) -> Value {
  let byte_start = (span.start as usize).min(source.len());
  let byte_end = (span.end() as usize).min(source.len());
  let ((line_start, col_start), (line_end, col_end)) =
    line_col_pair(source, byte_start, byte_end);

  let mut obj = span_json(filename, span.start, span.end());

  if let Value::Object(map) = &mut obj {
    map.insert("line_start".into(), json!(line_start));
    map.insert("line_end".into(), json!(line_end));
    map.insert("col_start".into(), json!(col_start));
    map.insert("col_end".into(), json!(col_end));
  }

  obj
}

/// Builds the `span` sub-object shared by the diagnostic's
/// primary span and every fix-it's edit span. Carries only
/// `file`/`byte_start`/`byte_end` — callers extend with
/// line/col fields when relevant (the primary span needs
/// human-readable position info; fix-it spans are
/// programmatic and don't).
fn span_json(filename: &str, byte_start: u32, byte_end: u32) -> Value {
  json!({
    "file":       filename,
    "byte_start": byte_start,
    "byte_end":   byte_end,
  })
}

/// 1-indexed `(line, column)` for a byte position in
/// `source`. Counts `\n` for line and UTF-8 chars (not
/// bytes) on the current line for column — `é` advances
/// column by 1, matching every IDE's convention.
///
/// Clamps `byte_pos > source.len()` (the EOF-span case
/// when the file ends without a trailing newline). Returns
/// `(1, 1)` for an empty source or for `byte_pos == 0`.
fn line_col_for_byte(source: &str, byte_pos: usize) -> (u32, u32) {
  let pos = byte_pos.min(source.len());
  let mut line: u32 = 1;
  let mut col: u32 = 1;

  for (i, ch) in source.char_indices() {
    if i >= pos {
      break;
    }

    if ch == '\n' {
      line += 1;
      col = 1;
    } else {
      col += 1;
    }
  }

  (line, col)
}

/// 1-indexed `(line, col)` pair for two byte positions in a
/// single source walk. Equivalent to two `line_col_for_byte`
/// calls but iterates `char_indices` once when the second
/// position is ≥ the first — the common case for diagnostic
/// span endpoints. Falls back to the per-position helper
/// when `byte_end < byte_start`.
fn line_col_pair(
  source: &str,
  byte_start: usize,
  byte_end: usize,
) -> ((u32, u32), (u32, u32)) {
  if byte_end < byte_start {
    return (
      line_col_for_byte(source, byte_start),
      line_col_for_byte(source, byte_end),
    );
  }

  let start_pos = byte_start.min(source.len());
  let end_pos = byte_end.min(source.len());
  let mut start = (1u32, 1u32);
  let mut line: u32 = 1;
  let mut col: u32 = 1;
  let mut start_captured = false;

  for (i, ch) in source.char_indices() {
    if !start_captured && i >= start_pos {
      start = (line, col);
      start_captured = true;
    }
    if i >= end_pos {
      return (start, (line, col));
    }

    if ch == '\n' {
      line += 1;
      col = 1;
    } else {
      col += 1;
    }
  }

  if !start_captured {
    start = (line, col);
  }
  (start, (line, col))
}

/// Inline source context for a diagnostic. Returns
/// `(before, lines, after)`:
///
/// * `before` — up to `context` lines immediately preceding
///   the span's starting line.
/// * `lines` — every line the span touches (one entry for
///   single-line spans, more for multi-line spans).
/// * `after` — up to `context` lines immediately following
///   the span's ending line.
///
/// Lines are stored without trailing newlines. Saturates at
/// the source's bounds (when the span sits near the top or
/// bottom of the file, `before` / `after` shrink instead of
/// returning blank lines). Empty source returns three empty
/// vecs.
fn extract_snippet(
  source: &str,
  byte_start: usize,
  byte_end: usize,
  context: usize,
) -> (Vec<String>, Vec<String>, Vec<String>) {
  let lines: Vec<&str> = source.lines().collect();

  if lines.is_empty() {
    return (Vec::new(), Vec::new(), Vec::new());
  }

  // Single pass over the source's newlines determines both
  // line indices. Previous shape called `source[..b].matches`
  // twice — O(N) each — and re-scanned identical bytes for
  // every diagnostic. One walk now suffices for the common
  // `byte_start <= byte_end` case; the rare inverted case
  // falls back to two scans (correctness over micro-opt).
  let last = lines.len() - 1;
  let (start_line, end_line) =
    line_indices_for_range(source, byte_start, byte_end, last);

  let before_start = start_line.saturating_sub(context);
  let after_end = (end_line + 1 + context).min(lines.len());

  let before = lines[before_start..start_line]
    .iter()
    .map(|s| s.to_string())
    .collect();
  let span_lines = lines[start_line..=end_line]
    .iter()
    .map(|s| s.to_string())
    .collect();
  let after = lines[(end_line + 1).min(lines.len())..after_end]
    .iter()
    .map(|s| s.to_string())
    .collect();

  (before, span_lines, after)
}

/// 0-indexed line numbers for two byte positions, clamped to
/// `last_line`. Walks `source.bytes()` once when
/// `byte_end >= byte_start` (the diagnostic-span norm),
/// counting `\n` characters and snapshotting the start
/// position along the way.
fn line_indices_for_range(
  source: &str,
  byte_start: usize,
  byte_end: usize,
  last_line: usize,
) -> (usize, usize) {
  let len = source.len();
  let s_pos = byte_start.min(len);
  let e_pos = byte_end.min(len);

  if e_pos < s_pos {
    let count_nl = |b: usize| {
      source.as_bytes()[..b]
        .iter()
        .filter(|&&c| c == b'\n')
        .count()
    };
    return (
      count_nl(s_pos).min(last_line),
      count_nl(e_pos).min(last_line),
    );
  }

  let mut start_line = 0;
  let mut start_captured = false;
  let mut newlines = 0usize;

  for (i, &b) in source.as_bytes().iter().enumerate() {
    if !start_captured && i >= s_pos {
      start_line = newlines;
      start_captured = true;
    }
    if i >= e_pos {
      return (start_line.min(last_line), newlines.min(last_line));
    }
    if b == b'\n' {
      newlines += 1;
    }
  }

  if !start_captured {
    start_line = newlines;
  }
  (start_line.min(last_line), newlines.min(last_line))
}

/// Encode the `FixIt` slice for one diagnostic as a JSON
/// array. The fix's span is computed from its `FixKind`
/// relative to the diagnostic's primary span:
///
/// * `Insert` → zero-length point at `span_start`.
/// * `Replace` / `Delete` → the full `[span_start, span_end)`
///   range.
///
/// Empty input slice (variant has no machine-applicable
/// fix) → empty JSON array. Consumers can always assume
/// the `fixes` field exists and is an array.
fn encode_fixes(
  fixes: &[FixIt],
  filename: &str,
  span_start: u32,
  span_end: u32,
) -> Value {
  // Pre-sized: the typical case is 0 or 1 fix, and we know
  // the exact count up front. Skips Vec's doubling realloc.
  let mut entries: Vec<Value> = Vec::with_capacity(fixes.len());

  for fix in fixes {
    let (fix_start, fix_end) = match fix.kind {
      FixKind::Insert => (span_start, span_start),
      FixKind::Replace | FixKind::Delete => (span_start, span_end),
    };

    entries.push(json!({
      "kind":        fix.kind.as_str(),
      "text":        fix.text,
      "description": fix.description,
      "span":        span_json(filename, fix_start, fix_end),
    }));
  }

  Value::Array(entries)
}

#[cfg(test)]
mod tests {
  use super::*;

  use crate::aggregator::ErrorAggregator;

  use zo_error::{Error, ErrorKind};
  use zo_span::Span;

  fn aggregate(error: Error) -> ErrorAggregator {
    let mut agg = ErrorAggregator::new();
    agg.add_errors(&[error]);
    agg
  }

  fn files(name: &str, source: &str) -> Vec<(PathBuf, String)> {
    vec![(PathBuf::from(name), source.to_string())]
  }

  #[test]
  fn extract_snippet_returns_context_lines() {
    let source = "line1\nline2\nline3\nline4\nline5\n";

    // Span on line 3 (byte 12..17), context = 1: expect
    // 1 line before, the span line, 1 line after.
    let (before, lines, after) = extract_snippet(source, 12, 17, 1);
    assert_eq!(before, vec!["line2"]);
    assert_eq!(lines, vec!["line3"]);
    assert_eq!(after, vec!["line4"]);

    // Context = 2 expands to 2 each.
    let (before, lines, after) = extract_snippet(source, 12, 17, 2);
    assert_eq!(before, vec!["line1", "line2"]);
    assert_eq!(lines, vec!["line3"]);
    assert_eq!(after, vec!["line4", "line5"]);

    // Span at start of file saturates `before` to empty.
    let (before, lines, _) = extract_snippet(source, 0, 5, 2);
    assert_eq!(before, Vec::<String>::new());
    assert_eq!(lines, vec!["line1"]);

    // Span at end of file saturates `after` to empty.
    let (_, lines, after) = extract_snippet(source, 24, 29, 2);
    assert_eq!(lines, vec!["line5"]);
    assert_eq!(after, Vec::<String>::new());

    // Multi-line span: `lines` contains every line touched.
    let (_, lines, _) = extract_snippet(source, 6, 17, 0);
    assert_eq!(lines, vec!["line2", "line3"]);

    // Empty source — all three vecs empty.
    let (b, l, a) = extract_snippet("", 0, 0, 2);
    assert!(b.is_empty() && l.is_empty() && a.is_empty());
  }

  #[test]
  fn line_col_for_byte_handles_multibyte_and_newlines() {
    // ASCII single-line: column counts chars, 1-indexed.
    assert_eq!(line_col_for_byte("hello", 0), (1, 1));
    assert_eq!(line_col_for_byte("hello", 3), (1, 4));
    assert_eq!(line_col_for_byte("hello", 5), (1, 6)); // EOF
    // `\n` resets column.
    assert_eq!(line_col_for_byte("a\nbc", 2), (2, 1));
    assert_eq!(line_col_for_byte("a\nbc", 4), (2, 3));
    // Multi-byte char advances column by 1, not by byte
    // length. `é` is 2 bytes (U+00E9); after `é` the
    // column is 2.
    assert_eq!(line_col_for_byte("é", 2), (1, 2));
    // `byte_pos` past end clamps to source.len().
    assert_eq!(line_col_for_byte("ab", 999), (1, 3));
    // Empty source.
    assert_eq!(line_col_for_byte("", 0), (1, 1));
    assert_eq!(line_col_for_byte("", 999), (1, 1));
  }

  #[test]
  fn emits_one_line_per_diagnostic() {
    // 70-byte ASCII source so byte_start=70 lands at the
    // file-end column (1-indexed col = 71 since source has
    // 70 chars on line 1).
    let source = "a".repeat(70);
    let err = Error::new(ErrorKind::MissingMainFunction, Span::new(70, 0));
    let agg = aggregate(err);
    let mut buf = Vec::new();

    to_json(&agg, &files("foo.zo", &source), 2, &mut buf).unwrap();

    let text = String::from_utf8(buf).unwrap();
    assert_eq!(
      text.lines().count(),
      1,
      "one diagnostic must produce exactly one NDJSON line",
    );
    assert!(
      text.ends_with('\n'),
      "every line must be terminated for streaming consumers",
    );
  }

  #[test]
  fn schema_carries_frozen_identity() {
    let source = "a".repeat(70);
    let err = Error::new(ErrorKind::MissingMainFunction, Span::new(70, 0));
    let agg = aggregate(err);
    let mut buf = Vec::new();

    to_json(&agg, &files("foo.zo", &source), 2, &mut buf).unwrap();
    let line = String::from_utf8(buf).unwrap();
    let v: Value = serde_json::from_str(line.trim()).unwrap();

    assert_eq!(v["$schema"], json!(SCHEMA_VERSION));
    assert_eq!(v["id"], json!("missing-main-function"));
    assert_eq!(v["code"], json!("E0800"));
    assert_eq!(v["severity"], json!("error"));
    assert_eq!(v["phase"], json!("analyzer"));
    assert_eq!(v["span"]["file"], json!("foo.zo"));
    assert_eq!(v["span"]["byte_start"], json!(70));
    assert_eq!(v["span"]["byte_end"], json!(70));
    assert_eq!(v["span"]["line_start"], json!(1));
    assert_eq!(v["span"]["line_end"], json!(1));
    assert_eq!(v["span"]["col_start"], json!(71));
    assert_eq!(v["span"]["col_end"], json!(71));
    // No secondary span on a single-span diagnostic.
    assert!(v.get("secondary").is_none());
  }

  #[test]
  fn type_mismatch_emits_secondary_span() {
    // `1 + true`: primary caret on `true` (byte 4..8), the
    // secondary on `1` (byte 0..1) — both values lit.
    let source = "1 + true";
    let err = Error::with_secondary(
      ErrorKind::TypeMismatch,
      Span::new(4, 4),
      Span::new(0, 1),
    );
    let agg = aggregate(err);
    let mut buf = Vec::new();

    to_json(&agg, &files("foo.zo", source), 0, &mut buf).unwrap();
    let v: Value =
      serde_json::from_str(String::from_utf8(buf).unwrap().trim()).unwrap();

    assert_eq!(v["span"]["byte_start"], json!(4));
    assert_eq!(v["span"]["byte_end"], json!(8));

    let sec = &v["secondary"];
    assert_eq!(sec["file"], json!("foo.zo"));
    assert_eq!(sec["byte_start"], json!(0));
    assert_eq!(sec["byte_end"], json!(1));
    assert_eq!(sec["line_start"], json!(1));
    assert_eq!(sec["col_start"], json!(1));
  }

  #[test]
  fn fixes_emitted_for_immutable_variable() {
    // Span on a 4-char identifier `name` starting at byte 4.
    let source = "imu name = 1";
    let err = Error::new(ErrorKind::ImmutableVariable, Span::new(4, 4));
    let agg = aggregate(err);
    let mut buf = Vec::new();

    to_json(&agg, &files("foo.zo", source), 0, &mut buf).unwrap();
    let line = String::from_utf8(buf).unwrap();
    let v: Value = serde_json::from_str(line.trim()).unwrap();

    let fixes = v["fixes"].as_array().expect("`fixes` must be an array");
    assert_eq!(fixes.len(), 1, "ImmutableVariable ships one fix");
    let fix = &fixes[0];
    assert_eq!(fix["kind"], json!("insert"));
    assert_eq!(fix["text"], json!("mut "));
    // Insert anchors at the diagnostic span's start.
    assert_eq!(fix["span"]["byte_start"], json!(4));
    assert_eq!(fix["span"]["byte_end"], json!(4));
    assert_eq!(fix["span"]["file"], json!("foo.zo"));
  }

  /// Phase 6 acceptance: rendering the same aggregator
  /// against the same source twice must produce
  /// byte-identical bytes. Locks the determinism invariant
  /// — any future change that introduces non-deterministic
  /// iteration (HashMap, system time, random ordering) into
  /// the JSON path breaks this test loudly.
  #[test]
  fn same_input_produces_byte_identical_output() {
    let source = "fun helper() {}\n";
    let err_a = Error::new(ErrorKind::MissingMainFunction, Span::new(16, 0));
    let err_b = Error::new(ErrorKind::ImmutableVariable, Span::new(4, 6));
    let mut agg = ErrorAggregator::new();
    agg.add_errors(&[err_a, err_b]);

    let mut buf1 = Vec::new();
    let mut buf2 = Vec::new();
    to_json(&agg, &files("foo.zo", source), 2, &mut buf1).unwrap();
    to_json(&agg, &files("foo.zo", source), 2, &mut buf2).unwrap();

    assert_eq!(
      buf1, buf2,
      "JSON renderer must be byte-deterministic across calls",
    );
  }

  /// Multi-phase determinism: errors from different phases
  /// must emit in fixed compilation order (Tokenizer →
  /// Parser → Analyzer → Codegen → Runtime). Locks the
  /// aggregator's bucketing contract.
  #[test]
  fn multi_phase_emission_order_is_stable() {
    let source = "fun helper() {}\n";

    // One error per phase, registered in REVERSE order to
    // prove the aggregator re-orders by phase, not by
    // arrival.
    let analyzer = Error::new(ErrorKind::TypeMismatch, Span::new(0, 0));
    let parser = Error::new(ErrorKind::UnexpectedToken, Span::new(0, 0));
    let tokenizer = Error::new(ErrorKind::UnexpectedCharacter, Span::new(0, 0));

    let mut agg = ErrorAggregator::new();
    agg.add_errors(&[analyzer, parser, tokenizer]);

    let mut buf = Vec::new();
    to_json(&agg, &files("foo.zo", source), 0, &mut buf).unwrap();
    let text = String::from_utf8(buf).unwrap();
    let phases: Vec<String> = text
      .lines()
      .map(|line| {
        let v: Value = serde_json::from_str(line).unwrap();
        v["phase"].as_str().unwrap().to_string()
      })
      .collect();

    assert_eq!(
      phases,
      vec![
        "tokenizer".to_string(),
        "parser".to_string(),
        "analyzer".to_string(),
      ],
      "phases must emit in fixed compilation order regardless of arrival",
    );
  }

  #[test]
  fn notes_emitted_for_variants_with_attached_context() {
    // `EmptyCharLiteral` carries an attached note about
    // why empty chars aren't representable. The JSON
    // renderer must surface the same prose the human
    // renderer attaches via ariadne's `with_note()`.
    let err = Error::new(ErrorKind::EmptyCharLiteral, Span::new(0, 2));
    let agg = aggregate(err);
    let mut buf = Vec::new();

    to_json(&agg, &files("foo.zo", "''"), 0, &mut buf).unwrap();
    let v: Value =
      serde_json::from_str(String::from_utf8(buf).unwrap().trim()).unwrap();

    let notes = v["notes"].as_array().expect("`notes` must be an array");
    assert_eq!(notes.len(), 1, "EmptyCharLiteral has exactly one note");
    assert!(
      notes[0].as_str().unwrap().contains("Unicode scalar"),
      "note text mismatched: {notes:?}",
    );
  }

  #[test]
  fn notes_empty_array_for_uncovered_variants() {
    // No `error_note` arm for `MissingMainFunction` → field
    // is present as an empty array, never omitted.
    let err = Error::new(ErrorKind::MissingMainFunction, Span::new(0, 0));
    let agg = aggregate(err);
    let mut buf = Vec::new();

    to_json(&agg, &files("foo.zo", ""), 0, &mut buf).unwrap();
    let v: Value =
      serde_json::from_str(String::from_utf8(buf).unwrap().trim()).unwrap();

    assert_eq!(v["notes"], json!([]));
  }

  #[test]
  fn fixes_empty_array_for_uncovered_variants() {
    let err = Error::new(ErrorKind::DivisionByZero, Span::new(0, 0));
    let agg = aggregate(err);
    let mut buf = Vec::new();

    to_json(&agg, &files("foo.zo", ""), 0, &mut buf).unwrap();
    let v: Value =
      serde_json::from_str(String::from_utf8(buf).unwrap().trim()).unwrap();

    assert_eq!(
      v["fixes"],
      json!([]),
      "uncovered variants must emit `fixes: []`, never omit the field",
    );
  }

  #[test]
  fn field_order_is_deterministic() {
    // Two diagnostics with different kinds — the field
    // sequence must be identical so byte-level diffing
    // works for agents.
    let a = Error::new(ErrorKind::MissingMainFunction, Span::new(0, 0));
    let b = Error::new(ErrorKind::TypeMismatch, Span::new(0, 0));
    let mut agg_a = ErrorAggregator::new();
    let mut agg_b = ErrorAggregator::new();
    agg_a.add_errors(&[a]);
    agg_b.add_errors(&[b]);

    let mut buf_a = Vec::new();
    let mut buf_b = Vec::new();
    to_json(&agg_a, &files("f.zo", ""), 2, &mut buf_a).unwrap();
    to_json(&agg_b, &files("f.zo", ""), 2, &mut buf_b).unwrap();

    let keys_a: Vec<_> = serde_json::from_slice::<Map<String, Value>>(
      buf_a.split(|&c| c == b'\n').next().unwrap(),
    )
    .unwrap()
    .keys()
    .cloned()
    .collect();
    let keys_b: Vec<_> = serde_json::from_slice::<Map<String, Value>>(
      buf_b.split(|&c| c == b'\n').next().unwrap(),
    )
    .unwrap()
    .keys()
    .cloned()
    .collect();

    assert_eq!(keys_a, keys_b);
    // Insertion order via serde_json's IndexMap-backed
    // `Map` (preserve_order feature on, since zo-provider-
    // json needs it to keep user JSON ordering). Order is
    // chosen deliberately in `encode`: schema/identity →
    // severity/phase → message → content. Locks the wire
    // shape so agents byte-diffing builds aren't surprised
    // by reorders.
    assert_eq!(
      keys_a,
      vec![
        "$schema", "id", "code", "severity", "phase", "message", "fixes",
        "notes", "snippet", "span",
      ],
    );
  }
}
