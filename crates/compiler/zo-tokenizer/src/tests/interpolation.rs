use crate::tokenizer::is_valid_ident;
use crate::Tokenizer;

use zo_interner::Interner;
use zo_token::{InterpSegment, Token};

use proptest::prelude::*;
use proptest::test_runner::{Config, FileFailurePersistence};

fn ident_strategy() -> impl Strategy<Value = String> {
  "[a-zA-Z_][a-zA-Z0-9_]{0,20}"
}

fn non_ident_strategy() -> impl Strategy<Value = String> {
  prop_oneof![
    "[0-9][a-z]{0,10}",
    "[a-z]+ [a-z]+",
    "[a-z]+[^a-zA-Z0-9_]+[a-z]*",
    Just(String::new()),
  ]
}

fn safe_literal_strategy() -> impl Strategy<Value = String> {
  "[a-zA-Z0-9 ,.:!?_=\\-]{0,30}"
}

// -------------------------------------------------------
// Property 1: `is_valid_ident` matches
// `^[a-zA-Z_][a-zA-Z0-9_]*$` exactly.
// -------------------------------------------------------

proptest! {
  #![proptest_config(Config {
    failure_persistence: Some(Box::new(
      FileFailurePersistence::Off,
    )),
    cases: 2000,
    ..Config::default()
  })]

  #[test]
  fn ident_accepts_valid(s in ident_strategy()) {
    prop_assert!(
      is_valid_ident(&s),
      "rejected valid ident: {:?}", s
    );
  }

  #[test]
  fn ident_rejects_invalid(s in non_ident_strategy()) {
    prop_assert!(
      !is_valid_ident(&s),
      "accepted invalid ident: {:?}", s
    );
  }

  #[test]
  fn ident_rejects_arbitrary_ascii(
    s in "[[:ascii:]]{1,30}"
  ) {
    let expected = !s.is_empty()
      && (s.as_bytes()[0].is_ascii_alphabetic()
        || s.as_bytes()[0] == b'_')
      && s.bytes()
        .skip(1)
        .all(|b| b.is_ascii_alphanumeric() || b == b'_');

    prop_assert_eq!(
      is_valid_ident(&s),
      expected,
      "mismatch for {:?}", s
    );
  }
}

// -------------------------------------------------------
// Property 2: Segment roundtrip — concatenating all
// segments (with Variable names wrapped in `{}`)
// reproduces the original string content.
// -------------------------------------------------------

fn segments_for(source: &str) -> Vec<InterpSegment> {
  let quoted = format!("\"{source}\"");
  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(&quoted, &mut interner);
  let result = tokenizer.tokenize();

  if result.tokens.kinds.is_empty() {
    return Vec::new();
  }

  let kind = result.tokens.kinds[0];

  if kind != Token::InterpString {
    return Vec::new();
  }

  let packed = result.tokens.literal_indices[0];
  let interp_id = packed >> 16;

  result.literals.interp_segs(interp_id).to_vec()
}

fn reconstruct(
  segments: &[InterpSegment],
  interner: &Interner,
) -> String {
  let mut out = String::new();

  for seg in segments {
    match seg {
      InterpSegment::Literal(sym) => {
        out.push_str(interner.get(*sym));
      }
      InterpSegment::Variable(sym) => {
        out.push('{');
        out.push_str(interner.get(*sym));
        out.push('}');
      }
    }
  }

  out
}

fn roundtrip(content: &str) -> bool {
  let quoted = format!("\"{content}\"");
  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(&quoted, &mut interner);
  let result = tokenizer.tokenize();

  if result.tokens.kinds.is_empty() {
    return content.is_empty();
  }

  let kind = result.tokens.kinds[0];

  if kind == Token::String {
    return !content.contains('{')
      || content.contains("\\{");
  }

  if kind != Token::InterpString {
    return false;
  }

  let packed = result.tokens.literal_indices[0];
  let interp_id = packed >> 16;
  let segments = result.literals.interp_segs(interp_id);

  reconstruct(segments, &interner) == content
}

proptest! {
  #![proptest_config(Config {
    failure_persistence: Some(Box::new(
      FileFailurePersistence::Off,
    )),
    cases: 1000,
    ..Config::default()
  })]

  #[test]
  fn roundtrip_literal_only(
    content in safe_literal_strategy()
  ) {
    prop_assert!(
      roundtrip(&content),
      "roundtrip failed for: {:?}", content
    );
  }

  #[test]
  fn roundtrip_single_variable(
    prefix in safe_literal_strategy(),
    var in ident_strategy(),
    suffix in safe_literal_strategy(),
  ) {
    let content = format!("{prefix}{{{var}}}{suffix}");

    prop_assert!(
      roundtrip(&content),
      "roundtrip failed for: {:?}", content
    );
  }

  #[test]
  fn roundtrip_multi_variable(
    a in safe_literal_strategy(),
    v1 in ident_strategy(),
    b in safe_literal_strategy(),
    v2 in ident_strategy(),
    c in safe_literal_strategy(),
  ) {
    let content =
      format!("{a}{{{v1}}}{b}{{{v2}}}{c}");

    prop_assert!(
      roundtrip(&content),
      "roundtrip failed for: {:?}", content
    );
  }
}

// -------------------------------------------------------
// Property 3: Non-identifier `{...}` NEVER produces a
// Variable segment — only Literal segments.
// -------------------------------------------------------

proptest! {
  #![proptest_config(Config {
    failure_persistence: Some(Box::new(
      FileFailurePersistence::Off,
    )),
    cases: 1000,
    ..Config::default()
  })]

  #[test]
  fn non_ident_braces_stay_literal(
    inner in non_ident_strategy()
  ) {
    let content = format!("{{{inner}}}");
    let segments = segments_for(&content);

    for seg in &segments {
      if let InterpSegment::Variable(sym) = seg {
        let _ = sym;

        prop_assert!(
          false,
          "non-ident {:?} produced Variable segment", inner
        );
      }
    }
  }

  #[test]
  fn json_like_strings_no_variables(
    key in "[a-z]{1,8}",
    value in "[0-9]{1,5}",
  ) {
    let content =
      format!("{{\\\"{}\\\":{}}}", key, value);
    let segments = segments_for(&content);

    for seg in &segments {
      if let InterpSegment::Variable(_) = seg {
        prop_assert!(
          false,
          "JSON-like string produced Variable: {:?}", content
        );
      }
    }
  }
}

// -------------------------------------------------------
// Property 4: Unmatched `{` preserves all content.
// -------------------------------------------------------

proptest! {
  #![proptest_config(Config {
    failure_persistence: Some(Box::new(
      FileFailurePersistence::Off,
    )),
    cases: 500,
    ..Config::default()
  })]

  #[test]
  fn unmatched_brace_preserves_content(
    prefix in "[a-zA-Z ]{0,10}",
    tail in "[a-zA-Z0-9 ]{0,10}",
  ) {
    let content = format!("{prefix}{{{tail}");

    prop_assert!(
      roundtrip(&content),
      "unmatched brace lost content: {:?}", content
    );
  }
}

// -------------------------------------------------------
// Deterministic edge cases.
// -------------------------------------------------------

#[test]
fn empty_braces_not_variable() {
  let segments = segments_for("{}");

  for seg in &segments {
    assert!(
      !matches!(seg, InterpSegment::Variable(_)),
      "empty braces produced Variable"
    );
  }
}

#[test]
fn escaped_brace_not_interpolation() {
  let quoted = r#""\{name\}""#;
  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(quoted, &mut interner);
  let result = tokenizer.tokenize();

  assert_eq!(
    result.tokens.kinds[0],
    Token::String,
    "escaped braces should produce Token::String"
  );
}

#[test]
fn json_object_string() {
  let quoted = r#""{\"key\":\"val\"}""#;
  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(quoted, &mut interner);
  let result = tokenizer.tokenize();

  let kind = result.tokens.kinds[0];

  if kind == Token::InterpString {
    let packed = result.tokens.literal_indices[0];
    let interp_id = packed >> 16;
    let segments = result.literals.interp_segs(interp_id);

    for seg in segments {
      assert!(
        !matches!(seg, InterpSegment::Variable(_)),
        "JSON object produced Variable segment"
      );
    }
  }
}

#[test]
fn adjacent_variables() {
  assert!(
    roundtrip("{x}{y}"),
    "adjacent variables failed roundtrip"
  );
}

#[test]
fn variable_at_boundaries() {
  assert!(roundtrip("{x}"), "lone variable failed");
  assert!(roundtrip("a{x}"), "prefix + var failed");
  assert!(roundtrip("{x}b"), "var + suffix failed");
}

#[test]
fn unmatched_open_brace() {
  assert!(
    roundtrip("{not_closed"),
    "unmatched brace failed roundtrip"
  );
}

#[test]
fn mixed_valid_and_invalid_braces() {
  assert!(
    roundtrip("{x} and {not valid} end"),
    "mixed valid/invalid failed roundtrip"
  );
}