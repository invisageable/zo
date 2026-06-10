//! XML renderer for agentic consumers.
//!
//! Emits one well-formed `<diagnostics>` document on stdout.
//! Where the JSON renderer streams NDJSON for a `tail`-like
//! reader, XML is built as a single complete block — the form
//! an agent drops straight into a prompt, where the tag
//! boundaries read as explicit structural barriers.
//!
//! The XML and JSON shapes are **isomorphic**: same identity,
//! same fields, same line/col geometry. An agent that knows
//! one knows the other — a JSON key maps 1:1 onto the XML
//! element/attribute of the same name. The schema version is
//! shared ([`DIAGNOSTIC_SCHEMA_VERSION`]) and the two bump in
//! lockstep.
//!
//! ## Schema (v1)
//!
//! ```xml
//! <diagnostics schema="1">
//!   <diagnostic id="missing-main-function" code="E0800"
//!               severity="error" phase="analyzer">
//!     <message>`main` function not found</message>
//!     <fixes>
//!       <fix kind="insert" file="foo.zo"
//!            byte_start="70" byte_end="70">
//!         <text>&#10;fun main() {&#10;}&#10;</text>
//!         <description>Add an empty `main` entry point</description>
//!       </fix>
//!     </fixes>
//!     <notes/>
//!     <snippet>
//!       <before><line>fun qux() {}</line></before>
//!       <lines><line>fun quux() {}</line></lines>
//!       <after/>
//!     </snippet>
//!     <span file="foo.zo" byte_start="70" byte_end="70"
//!           line_start="1" line_end="1"
//!           col_start="71" col_end="71"/>
//!   </diagnostic>
//! </diagnostics>
//! ```
//!
//! `schema` lives once on the root — it is global, not
//! per-diagnostic. `fixes` and `notes` are **always present**
//! (self-closing when empty) so a consumer never needs a
//! presence check. Element order within a `<diagnostic>` is
//! fixed (message → fixes → notes → snippet → span →
//! secondary → detail) so the byte output is deterministic:
//! the same aggregator over the same source renders identical
//! bytes, the precondition for agents diffing diagnostic
//! streams.
//!
//! Line and column are **1-indexed** and count UTF-8 chars,
//! matching the JSON renderer (see [`crate::locate`]).

use crate::aggregator::{ErrorAggregator, Phase};
use crate::collector::Detail;
use crate::fixes::{FixIt, fixes_for};
use crate::format::DIAGNOSTIC_SCHEMA_VERSION;
use crate::locate::{extract_snippet, file_for_error, fix_span, line_col_pair};
use crate::render::{error_message, error_note};

use zo_buffer::Buffer;
use zo_error::Error;
use zo_span::Span;

use std::io;
use std::io::Write;
use std::path::PathBuf;

/// Renders every diagnostic in `aggregator` as a single
/// well-formed `<diagnostics>` document on the given writer.
/// The whole document is built in one buffer and written
/// once — XML is a tree, so it cannot stream a valid document
/// incrementally the way NDJSON can.
///
/// `files` supplies the source text per file id — required to
/// materialise line/col from byte offsets and to inline
/// snippet context. `snippet_context` is the number of source
/// lines kept before and after each span.
pub fn to_xml<W: Write>(
  aggregator: &ErrorAggregator,
  files: &[(PathBuf, String)],
  snippet_context: usize,
  out: &mut W,
) -> io::Result<()> {
  let mut buf = Buffer::new();

  buf.str("<diagnostics schema=\"");
  buf.u32(DIAGNOSTIC_SCHEMA_VERSION);
  buf.str("\">");
  buf.newline();

  for phase_errors in aggregator.errors() {
    for error in &phase_errors.errors {
      let (source, filename) = file_for_error(error, files);

      encode_diagnostic(
        &mut buf,
        error,
        phase_errors.phase,
        source,
        &filename,
        snippet_context,
        aggregator.detail_for(error),
      );
    }
  }

  buf.str("</diagnostics>");
  buf.newline();

  out.write_all(&buf.finish())?;
  out.flush()
}

/// Convenience: emit the XML document to stdout. Mirrors
/// `json::to_stdout` so the driver dispatches on `--format`
/// with one uniform branch per renderer.
pub fn to_stdout(
  aggregator: &ErrorAggregator,
  files: &[(PathBuf, String)],
  snippet_context: usize,
) -> io::Result<()> {
  let stdout = io::stdout();
  let mut handle = stdout.lock();

  to_xml(aggregator, files, snippet_context, &mut handle)
}

/// Emit one `<diagnostic>` element. Field selection and order
/// mirror `json::encode` exactly so the two wire formats stay
/// isomorphic: identity attributes, then message → fixes →
/// notes → snippet → span → secondary → dynamic detail.
fn encode_diagnostic(
  buf: &mut Buffer,
  error: &Error,
  phase: Phase,
  source: &str,
  filename: &str,
  snippet_context: usize,
  detail: Option<&Detail>,
) {
  let kind = error.kind();
  let span = error.span();
  let byte_start = (span.start as usize).min(source.len());
  let byte_end = (span.end() as usize).min(source.len());
  let snippet = extract_snippet(source, byte_start, byte_end, snippet_context);

  // Where the detail's own fields already explain the
  // mismatch, the generic per-kind note is redundant — same
  // rule the JSON encoder and human renderer share.
  let suppress_note = detail.is_some_and(Detail::suppresses_note);
  let note = match error_note(kind) {
    Some(text) if !suppress_note => Some(text),
    _ => None,
  };

  indent(buf, 1);
  buf.str("<diagnostic");
  str_attr(buf, "id", kind.id());
  str_attr(buf, "code", &format!("E{:04}", kind.code()));
  str_attr(buf, "severity", error.severity().as_str());
  str_attr(buf, "phase", phase.as_str());
  buf.str(">");
  buf.newline();

  text_element(buf, 2, "message", error_message(kind));
  encode_fixes(buf, fixes_for(kind), filename, span, detail);
  encode_notes(buf, note);
  encode_snippet(buf, &snippet);
  span_element(buf, 2, "span", filename, span, source);

  // The conflicting value in a type mismatch (the green
  // secondary in the human render). Present only when the
  // diagnostic carries two spans.
  if let Some(secondary) = error.secondary_span() {
    span_element(buf, 2, "secondary", filename, secondary, source);
  }

  encode_detail(buf, detail);

  indent(buf, 1);
  buf.str("</diagnostic>");
  buf.newline();
}

/// Emit the `<fixes>` block. Always present — self-closing
/// when empty. A `Detail::Suggestion` appends one extra
/// replace fix (replace the undefined name with the closest
/// in-scope name), matching the JSON encoder.
fn encode_fixes(
  buf: &mut Buffer,
  fixes: &[FixIt],
  filename: &str,
  span: Span,
  detail: Option<&Detail>,
) {
  let suggestion = match detail {
    Some(Detail::Suggestion(name)) | Some(Detail::Rename(name)) => {
      Some(&**name)
    }
    _ => None,
  };

  if fixes.is_empty() && suggestion.is_none() {
    indent(buf, 2);
    buf.str("<fixes/>");
    buf.newline();
    return;
  }

  indent(buf, 2);
  buf.str("<fixes>");
  buf.newline();

  for fix in fixes {
    let (fix_start, fix_end) = fix_span(fix.kind, span.start, span.end());
    fix_element(
      buf,
      fix.kind.as_str(),
      filename,
      fix_start,
      fix_end,
      fix.text,
      fix.description,
    );
  }

  // The suggestion's replace fix spans the offending name.
  if let Some(name) = suggestion {
    fix_element(
      buf,
      "replace",
      filename,
      span.start,
      span.end(),
      name,
      &format!("replace with `{name}`"),
    );
  }

  indent(buf, 2);
  buf.str("</fixes>");
  buf.newline();
}

/// Emit one `<fix>` element with its edit span and the
/// replacement text plus human description as child elements.
fn fix_element(
  buf: &mut Buffer,
  kind: &str,
  filename: &str,
  byte_start: u32,
  byte_end: u32,
  text: &str,
  description: &str,
) {
  indent(buf, 3);
  buf.str("<fix");
  str_attr(buf, "kind", kind);
  str_attr(buf, "file", filename);
  u32_attr(buf, "byte_start", byte_start);
  u32_attr(buf, "byte_end", byte_end);
  buf.str(">");
  buf.newline();

  text_element(buf, 4, "text", text);
  text_element(buf, 4, "description", description);

  indent(buf, 3);
  buf.str("</fix>");
  buf.newline();
}

/// Emit the `<notes>` block. Always present — self-closing
/// when the variant carries no attached note.
fn encode_notes(buf: &mut Buffer, note: Option<&str>) {
  match note {
    Some(text) => {
      indent(buf, 2);
      buf.str("<notes>");
      buf.newline();
      text_element(buf, 3, "note", text);
      indent(buf, 2);
      buf.str("</notes>");
      buf.newline();
    }
    None => {
      indent(buf, 2);
      buf.str("<notes/>");
      buf.newline();
    }
  }
}

/// Emit the `<snippet>` block: `<before>`, `<lines>`, and
/// `<after>`, each wrapping zero or more `<line>` elements
/// (self-closing when that group is empty).
fn encode_snippet(
  buf: &mut Buffer,
  snippet: &(Vec<String>, Vec<String>, Vec<String>),
) {
  let (before, lines, after) = snippet;

  indent(buf, 2);
  buf.str("<snippet>");
  buf.newline();

  line_group(buf, "before", before);
  line_group(buf, "lines", lines);
  line_group(buf, "after", after);

  indent(buf, 2);
  buf.str("</snippet>");
  buf.newline();
}

/// Emit one snippet group (`before` / `lines` / `after`) as a
/// `<name>` element wrapping a `<line>` per source line.
/// Self-closing when the group holds no lines.
fn line_group(buf: &mut Buffer, name: &str, lines: &[String]) {
  if lines.is_empty() {
    indent(buf, 3);
    buf.char(b'<');
    buf.str(name);
    buf.str("/>");
    buf.newline();
    return;
  }

  indent(buf, 3);
  buf.char(b'<');
  buf.str(name);
  buf.char(b'>');
  buf.newline();

  for line in lines {
    text_element(buf, 4, "line", line);
  }

  indent(buf, 3);
  buf.str("</");
  buf.str(name);
  buf.char(b'>');
  buf.newline();
}

/// Emit the dynamic, machine-readable detail as loose child
/// elements — the same field names the JSON encoder uses, so
/// an agent maps the two formats 1:1. `Suggestion` emits only
/// its name here; its replace fix is already in `<fixes>`.
fn encode_detail(buf: &mut Buffer, detail: Option<&Detail>) {
  match detail {
    Some(Detail::Types(names)) => {
      text_element(buf, 2, "primary_type", &names.primary);
      text_element(buf, 2, "secondary_type", &names.secondary);
    }
    Some(Detail::Suggestion(name)) | Some(Detail::Rename(name)) => {
      text_element(buf, 2, "suggestion", name);
    }
    Some(Detail::ArgCount {
      callee,
      expected,
      given,
      signature,
    }) => {
      text_element(buf, 2, "callee", callee);
      u32_element(buf, 2, "expected_count", *expected as u32);
      u32_element(buf, 2, "given_count", *given as u32);
      text_element(buf, 2, "signature", signature);
    }
    Some(Detail::ArgType {
      callee,
      found,
      expected,
      signature,
    }) => {
      text_element(buf, 2, "callee", callee);
      text_element(buf, 2, "primary_type", found);
      text_element(buf, 2, "secondary_type", expected);
      text_element(buf, 2, "signature", signature);
    }
    Some(Detail::ReturnType { found, expected }) => {
      text_element(buf, 2, "found_type", found);
      text_element(buf, 2, "expected_type", expected);
    }
    Some(Detail::DiscardedValue { found }) => {
      text_element(buf, 2, "found_type", found);
      text_element(buf, 2, "expected_type", "unit");
    }
    None => {}
  }
}

/// Emit a self-closing span element (`<span>` / `<secondary>`)
/// carrying byte offsets plus 1-indexed line/col — the same
/// seven fields the JSON renderer's full span object holds.
fn span_element(
  buf: &mut Buffer,
  depth: usize,
  tag: &str,
  filename: &str,
  span: Span,
  source: &str,
) {
  let byte_start = (span.start as usize).min(source.len());
  let byte_end = (span.end() as usize).min(source.len());
  let ((line_start, col_start), (line_end, col_end)) =
    line_col_pair(source, byte_start, byte_end);

  indent(buf, depth);
  buf.char(b'<');
  buf.str(tag);
  str_attr(buf, "file", filename);
  u32_attr(buf, "byte_start", span.start);
  u32_attr(buf, "byte_end", span.end());
  u32_attr(buf, "line_start", line_start);
  u32_attr(buf, "line_end", line_end);
  u32_attr(buf, "col_start", col_start);
  u32_attr(buf, "col_end", col_end);
  buf.str("/>");
  buf.newline();
}

/// Emit `<tag>escaped text</tag>` at the given depth.
fn text_element(buf: &mut Buffer, depth: usize, tag: &str, text: &str) {
  indent(buf, depth);
  buf.char(b'<');
  buf.str(tag);
  buf.char(b'>');
  push_escaped(buf, text, false);
  buf.str("</");
  buf.str(tag);
  buf.char(b'>');
  buf.newline();
}

/// Emit `<tag>n</tag>` at the given depth for a numeric value.
fn u32_element(buf: &mut Buffer, depth: usize, tag: &str, n: u32) {
  indent(buf, depth);
  buf.char(b'<');
  buf.str(tag);
  buf.char(b'>');
  buf.u32(n);
  buf.str("</");
  buf.str(tag);
  buf.char(b'>');
  buf.newline();
}

/// Append ` name="escaped value"` — a string attribute.
fn str_attr(buf: &mut Buffer, name: &str, value: &str) {
  buf.char(b' ');
  buf.str(name);
  buf.str("=\"");
  push_escaped(buf, value, true);
  buf.char(b'"');
}

/// Append ` name="n"` — a numeric attribute (never needs
/// escaping).
fn u32_attr(buf: &mut Buffer, name: &str, n: u32) {
  buf.char(b' ');
  buf.str(name);
  buf.str("=\"");
  buf.u32(n);
  buf.char(b'"');
}

/// Write `depth` levels of two-space indentation.
fn indent(buf: &mut Buffer, depth: usize) {
  for _ in 0..depth {
    buf.indent();
  }
}

/// Escape XML metacharacters, writing the result into `buf`.
/// `&`, `<`, `>` are escaped everywhere; `"` is escaped only
/// inside an attribute value (`in_attr`). Iterating bytes is
/// safe for UTF-8: the escaped bytes are all ASCII and never
/// occur inside a multi-byte sequence, so every other byte
/// passes through unchanged.
fn push_escaped(buf: &mut Buffer, text: &str, in_attr: bool) {
  for &byte in text.as_bytes() {
    match byte {
      b'&' => buf.str("&amp;"),
      b'<' => buf.str("&lt;"),
      b'>' => buf.str("&gt;"),
      b'"' if in_attr => buf.str("&quot;"),
      _ => buf.char(byte),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  use crate::collector::TyNames;

  use zo_error::ErrorKind;

  fn files(name: &str, source: &str) -> Vec<(PathBuf, String)> {
    vec![(PathBuf::from(name), source.to_string())]
  }

  fn aggregate(error: Error) -> ErrorAggregator {
    let mut agg = ErrorAggregator::new();
    agg.add_errors(&[error]);
    agg
  }

  fn render(agg: &ErrorAggregator, name: &str, source: &str) -> String {
    let mut buf = Vec::new();
    to_xml(agg, &files(name, source), 2, &mut buf).unwrap();
    String::from_utf8(buf).unwrap()
  }

  #[test]
  fn document_is_well_formed_and_carries_identity() {
    let source = "a".repeat(70);
    let err = Error::new(ErrorKind::MissingMainFunction, Span::new(70, 0));
    let xml = render(&aggregate(err), "foo.zo", &source);

    assert!(xml.starts_with("<diagnostics schema=\"1\">\n"));
    assert!(xml.trim_end().ends_with("</diagnostics>"));
    assert!(xml.contains("<diagnostic id=\"missing-main-function\""));
    assert!(xml.contains("code=\"E0800\""));
    assert!(xml.contains("severity=\"error\""));
    assert!(xml.contains("phase=\"analyzer\""));
    // The span carries byte offsets and 1-indexed line/col.
    assert!(xml.contains(
      "<span file=\"foo.zo\" byte_start=\"70\" byte_end=\"70\" \
       line_start=\"1\" line_end=\"1\" col_start=\"71\" col_end=\"71\"/>"
    ));
    // No secondary on a single-span diagnostic.
    assert!(!xml.contains("<secondary"));
  }

  #[test]
  fn empty_aggregator_renders_empty_document() {
    let agg = ErrorAggregator::new();
    let xml = render(&agg, "foo.zo", "");

    assert_eq!(xml, "<diagnostics schema=\"1\">\n</diagnostics>\n");
  }

  #[test]
  fn fixes_and_notes_always_present_self_closing_when_empty() {
    // DivisionByZero has no fix and no note → both groups
    // render self-closing, never omitted.
    let err = Error::new(ErrorKind::DivisionByZero, Span::new(0, 0));
    let xml = render(&aggregate(err), "f.zo", "");

    assert!(xml.contains("<fixes/>"));
    assert!(xml.contains("<notes/>"));
  }

  #[test]
  fn immutable_variable_emits_insert_fix() {
    let source = "imu name = 1";
    let err = Error::new(ErrorKind::ImmutableVariable, Span::new(4, 4));
    let xml = render(&aggregate(err), "foo.zo", source);

    assert!(xml.contains("<fix kind=\"insert\" file=\"foo.zo\""));
    // Insert anchors at the span start (zero-width).
    assert!(xml.contains("byte_start=\"4\" byte_end=\"4\""));
    assert!(xml.contains("<text>mut </text>"));
  }

  #[test]
  fn type_mismatch_emits_secondary_and_type_names() {
    let source = "1 + true";
    let err = Error::with_secondary(
      ErrorKind::TypeMismatch,
      Span::new(4, 4),
      Span::new(0, 1),
    );
    let detail = Detail::Types(TyNames {
      primary: "bool".into(),
      secondary: "int".into(),
    });
    let mut agg = ErrorAggregator::new();
    agg.add_errors(&[err]);
    agg.add_details(&[(err, detail)]);

    let xml = render(&agg, "foo.zo", source);

    assert!(xml.contains("<secondary file=\"foo.zo\" byte_start=\"0\""));
    assert!(xml.contains("<primary_type>bool</primary_type>"));
    assert!(xml.contains("<secondary_type>int</secondary_type>"));
  }

  #[test]
  fn suggestion_emits_replace_fix_and_name() {
    let source = "showln(cont)";
    let err = Error::new(ErrorKind::UndefinedVariable, Span::new(7, 4));
    let mut agg = ErrorAggregator::new();
    agg.add_errors(&[err]);
    agg.add_details(&[(err, Detail::Suggestion("count".into()))]);

    let xml = render(&agg, "foo.zo", source);

    assert!(xml.contains("<suggestion>count</suggestion>"));
    assert!(xml.contains("<fix kind=\"replace\" file=\"foo.zo\""));
    assert!(xml.contains("byte_start=\"7\" byte_end=\"11\""));
    assert!(xml.contains("<text>count</text>"));
  }

  #[test]
  fn metacharacters_are_escaped() {
    // A note containing `&`, `<`, `>` must escape in text;
    // an attribute value with `"` must escape too. Use a
    // filename carrying a quote to exercise attribute escaping.
    let source = "''";
    let err = Error::new(ErrorKind::EmptyCharLiteral, Span::new(0, 2));
    let xml = render(&aggregate(err), "a\"b.zo", source);

    // Attribute escaping: the quote in the filename.
    assert!(xml.contains("file=\"a&quot;b.zo\""));
    // The note text is present and any `<`/`>`/`&` are escaped
    // (no raw metacharacter leaks into element content).
    let note_open = xml.find("<note>").unwrap() + "<note>".len();
    let note_close = xml[note_open..].find("</note>").unwrap();
    let note_body = &xml[note_open..note_open + note_close];
    assert!(!note_body.contains('<'));
    assert!(!note_body.contains('>'));
    assert!(!note_body.contains('&') || note_body.contains("&amp;"));
  }

  #[test]
  fn snippet_groups_render_lines() {
    let source = "line1\nline2\nline3\nline4\nline5\n";
    // Span on line 3 (byte 12..17) with context 2.
    let err = Error::new(ErrorKind::TypeMismatch, Span::new(12, 5));
    let xml = render(&aggregate(err), "f.zo", source);

    assert!(xml.contains("<before>"));
    assert!(xml.contains("<line>line2</line>"));
    assert!(xml.contains("<lines>"));
    assert!(xml.contains("<line>line3</line>"));
    assert!(xml.contains("<after>"));
    assert!(xml.contains("<line>line4</line>"));
  }

  /// Same aggregator over the same source must render
  /// byte-identical XML. Locks determinism — any future
  /// non-deterministic iteration (HashMap, time, random)
  /// breaks this loudly.
  #[test]
  fn same_input_produces_byte_identical_output() {
    let source = "fun helper() {}\n";
    let err_a = Error::new(ErrorKind::MissingMainFunction, Span::new(16, 0));
    let err_b = Error::new(ErrorKind::ImmutableVariable, Span::new(4, 6));
    let mut agg = ErrorAggregator::new();
    agg.add_errors(&[err_a, err_b]);

    let first = render(&agg, "foo.zo", source);
    let second = render(&agg, "foo.zo", source);

    assert_eq!(first, second, "XML renderer must be byte-deterministic");
  }
}
