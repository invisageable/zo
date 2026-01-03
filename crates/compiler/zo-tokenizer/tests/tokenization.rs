use zo_reporter::collect_errors;
use zo_token::Token;
use zo_tokenizer::Tokenizer;

use proptest::prelude::*;
use proptest::test_runner::{Config, FileFailurePersistence};

/// Generates valid identifier patterns.
fn valid_identifier() -> impl Strategy<Value = String> {
  "[a-zA-Z_][a-zA-Z0-9_]{0,20}"
}

/// Generates potentially valid number literals.
fn number_literal() -> impl Strategy<Value = String> {
  prop_oneof![
    // Decimal
    "[0-9]{1,10}",
    // Binary
    "0b[01]{1,16}",
    // Octal
    "0o[0-7]{1,10}",
    // Hex
    "0x[0-9a-f]{1,8}",
    // Float
    "[0-9]{1,5}\\.[0-9]{1,5}",
  ]
}

/// Generates string literal patterns (may be valid or invalid).
fn string_pattern() -> impl Strategy<Value = String> {
  prop_oneof![
    // Valid strings
    "\"[^\"\\n]{0,50}\"",
    // Unterminated strings
    "\"[^\"\\n]{0,50}",
    // Raw strings ($ followed by quoted string)
    r#"\$"[^"]{0,50}""#,
  ]
}

/// Generates arbitrary source code fragments.
fn source_fragment() -> impl Strategy<Value = String> {
  prop_oneof![
    valid_identifier(),
    number_literal(),
    string_pattern(),
    prop::sample::select(vec![
      "fun", "if", "else", "while", "for", "mut", "imu", "struct", "enum",
      "return", "break", "continue"
    ])
    .prop_map(|s| s.to_string()),
    prop::sample::select(vec![
      "+", "-", "*", "/", "==", "!=", "&&", "||", "<<", ">>", "->", "=>", "::",
      ":="
    ])
    .prop_map(|s| s.to_string()),
    // Mixed content - just alphanumeric and basic punctuation
    "[a-zA-Z0-9 (){};,:=!<>]{0,100}",
  ]
}

proptest! {
  #![proptest_config(Config {
    failure_persistence: Some(Box::new(
      FileFailurePersistence::Off
    )),
    ..Config::default()
  })]

  #[test]
  fn tokenizer_never_panics(input in "\\PC{0,1000}") {
    let tokenizer = Tokenizer::new(&input);
    let _ = tokenizer.tokenize();
  }

  #[test]
  fn always_returns_result(input in source_fragment()) {
    let tokenizer = Tokenizer::new(&input);
    let tokenization = tokenizer.tokenize();

    assert!(!tokenization.tokens.kinds.is_empty());
    collect_errors();
  }

  #[test]
  fn valid_identifiers_tokenize(ident in valid_identifier()) {
    let tokenizer = Tokenizer::new(&ident);
    let tokenization = tokenizer.tokenize();

    if !tokenization.tokens.kinds.is_empty() {
      let first = tokenization.tokens.kinds[0];

      assert!(
        first == Token::Ident || is_keyword(first),
        "Expected identifier or keyword, got {:?} for input '{}'",
        first, ident
      );
    }
  }

  #[test]
  fn token_spans_within_bounds(input in source_fragment()) {
    let tokenizer = Tokenizer::new(&input);
    let source_len = input.len() as u32;
    let tokenization = tokenizer.tokenize();

    for (i, &start) in tokenization.tokens.starts.iter().enumerate() {
      let len = tokenization.tokens.lengths[i];

      assert!(
        start + len as u32 <= source_len,
        "Token {} has span [{}, {}) which exceeds source length {}",
        i, start, start + len as u32, source_len
      );
    }
  }

  #[test]
  fn tokenization_is_deterministic(input in source_fragment()) {
    let r1 = Tokenizer::new(&input).tokenize();
    let r2 = Tokenizer::new(&input).tokenize();

    assert_eq!(r1.tokens.kinds, r2.tokens.kinds);
    assert_eq!(r1.tokens.starts, r2.tokens.starts);
    assert_eq!(r1.tokens.lengths, r2.tokens.lengths);

  }

  #[test]
  fn number_literals_tokenize_correctly(num in number_literal()) {
    let tokenizer = Tokenizer::new(&num);
    let tokenization = tokenizer.tokenize();

    if !tokenization.tokens.kinds.is_empty() {
      let first = tokenization.tokens.kinds[0];

      assert!(
        first == Token::Int || first == Token::Float,
        "Expected Int or Float, got {:?} for input '{}'",
        first, num
      );
    }
  }

  #[test]
  fn whitespace_normalization(
    tokens in prop::collection::vec(
      prop::sample::select(vec![
        "(", ")", "{", "}", "[", "]", ";", ",",
        "==", "!=", "&&", "||", "<<", ">>",
        "+", "-", "*", "/", "=", "!"
      ]),
      1..10
    ),
    sep1 in prop::sample::select(vec![" ", "  ", "\t", "\n", "\r\n"]),
    sep2 in prop::sample::select(vec![" ", "  ", "\t", "\n", "\r\n"])
  ) {
    let input1 = tokens.join(sep1);
    let input2 = tokens.join(sep2);

    let result1 = Tokenizer::new(&input1).tokenize();
    let result2 = Tokenizer::new(&input2).tokenize();

    let tokens1 = result1.tokens.kinds.iter()
      .filter(|k| **k != Token::Eof)
      .collect::<Vec<_>>();

    let tokens2 = result2.tokens.kinds.iter()
      .filter(|k| **k != Token::Eof)
      .collect::<Vec<_>>();

    assert_eq!(tokens1, tokens2);
  }

  #[test]
  fn comments_are_ignored(
    code in "[a-zA-Z_][a-zA-Z0-9_]{0,10}",
    comment in prop::sample::select(vec![
      "-- comment\n",
      "-* block comment *-",
      "-* nested -* comment *- *-"
    ])
  ) {
    let with_comment = format!("{code} {comment} {code}");
    let without_comment = format!("{code} {code}");

    let result1 = Tokenizer::new(&with_comment).tokenize();
    let result2 = Tokenizer::new(&without_comment).tokenize();

    assert_eq!(result1.tokens.kinds, result2.tokens.kinds);
  }

  #[test]
  fn handles_deeply_nested_structures(depth in 1usize..100) {
    let nested = "{".repeat(depth) + "x" + &"}".repeat(depth);
    let tokenizer = Tokenizer::new(&nested);
    let _ = tokenizer.tokenize();
  }

  #[test]
  fn handles_pathological_patterns(n in 1usize..50) {
    let patterns = vec![
      "(".repeat(n) + &")".repeat(n),
      "\"".repeat(n),
      "-*".repeat(n),
      "<<".repeat(n),
    ];

    for pattern in patterns {
      let tokenizer = Tokenizer::new(&pattern);
      let _ = tokenizer.tokenize();
    }
  }
}

fn is_keyword(kind: Token) -> bool {
  matches!(
    kind,
    // keywords.
    Token::Fun | Token::Fn | Token::Mut | Token::Imu
    | Token::If | Token::Else | Token::While | Token::For
    | Token::Loop | Token::Pack | Token::Load | Token::Type
    | Token::Struct | Token::Enum | Token::Return | Token::Break
    | Token::Continue | Token::Match | Token::When | Token::As
    | Token::Is | Token::True | Token::False | Token::Pub
    | Token::Val | Token::Ext | Token::Abstract | Token::Apply
    | Token::State | Token::Group | Token::And | Token::Raw
    | Token::Wasm | Token::Nursery | Token::Spawn | Token::Await
    | Token::SelfUpper | Token::SelfLower |
    // type keywords.
    Token::IntType | Token::S8Type | Token::S16Type |
Token::S32Type     | Token::S64Type | Token::UintType | Token::U8Type | Token::U16Type
    | Token::U32Type | Token::U64Type | Token::FloatType | Token::F32Type
    | Token::F64Type | Token::BoolType | Token::BytesType | Token::CharType
    | Token::StrType
  )
}
