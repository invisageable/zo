pub(crate) mod common;
pub(crate) mod errors;

use crate::tests::common::assert_tokens_stream;

use zo_token::Token;

#[test]
fn test_whitespaces() {
  assert_tokens_stream(
    "\n \t a \n b \t",
    &[(Token::Ident, "a"), (Token::Ident, "b"), (Token::Eof, "")],
  );
}

#[test]
fn test_comments() {
  assert_tokens_stream(
    r#"
      a -- line comments
      b
      a -* block comments *- b
    "#,
    &[
      (Token::Ident, "a"),
      (Token::Ident, "b"),
      (Token::Ident, "a"),
      (Token::Ident, "b"),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn test_punctuation() {
  assert_tokens_stream(
    ", . : ; -> => -* ->> *- ::",
    &[
      (Token::Comma, ","),
      (Token::Dot, "."),
      (Token::Colon, ":"),
      (Token::Semicolon, ";"),
      (Token::Arrow, "->"),
      (Token::FatArrow, "=>"),
      (Token::ColonColon, "::"),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn test_delimiters() {
  assert_tokens_stream(
    "() {} []",
    &[
      (Token::LParen, "("),
      (Token::RParen, ")"),
      (Token::LBrace, "{"),
      (Token::RBrace, "}"),
      (Token::LBracket, "["),
      (Token::RBracket, "]"),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn test_arithmetic_operators() {
  assert_tokens_stream(
    "+ - * / %",
    &[
      (Token::Plus, "+"),
      (Token::Minus, "-"),
      (Token::Star, "*"),
      (Token::Slash, "/"),
      (Token::Percent, "%"),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn test_assignment_operators() {
  assert_tokens_stream(
    "= := += -= *= /= %= &= |= ^= <<= >>=",
    &[
      (Token::Eq, "="),
      (Token::ColonEq, ":="),
      (Token::PlusEq, "+="),
      (Token::MinusEq, "-="),
      (Token::StarEq, "*="),
      (Token::SlashEq, "/="),
      (Token::PercentEq, "%="),
      (Token::AmpEq, "&="),
      (Token::PipeEq, "|="),
      (Token::CaretEq, "^="),
      (Token::LShiftEq, "<<="),
      (Token::RShiftEq, ">>="),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn test_comparison_operators() {
  assert_tokens_stream(
    "== != < > <= >=",
    &[
      (Token::EqEq, "=="),
      (Token::BangEq, "!="),
      (Token::Lt, "<"),
      (Token::Gt, ">"),
      (Token::LtEq, "<="),
      (Token::GtEq, ">="),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn test_logical_operators() {
  assert_tokens_stream(
    "&& || !",
    &[
      (Token::AmpAmp, "&&"),
      (Token::PipePipe, "||"),
      (Token::Bang, "!"),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn test_bitwise_operators() {
  assert_tokens_stream(
    "& | ^ << >>",
    &[
      (Token::Amp, "&"),
      (Token::Pipe, "|"),
      (Token::Caret, "^"),
      (Token::LShift, "<<"),
      (Token::RShift, ">>"),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn test_special_operators() {
  assert_tokens_stream(
    "? @ # $ %% .. ..= ...",
    &[
      (Token::Question, "?"),
      (Token::At, "@"),
      (Token::Hash, "#"),
      (Token::Dollar, "$"),
      (Token::Attribute, "%%"),
      (Token::DotDot, ".."),
      (Token::DotDotEq, "..="),
      (Token::Ellipsis, "..."),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn test_identifiers() {
  assert_tokens_stream(
    "_ foo _bar oof123 if_then FooBar",
    &[
      (Token::Ident, "_"),
      (Token::Ident, "foo"),
      (Token::Ident, "_bar"),
      (Token::Ident, "oof123"),
      (Token::Ident, "if_then"),
      (Token::Ident, "FooBar"),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn test_declarations_keywords() {
  assert_tokens_stream(
    r#"
      abstract and apply enum ext fn fun group
      imu load mut pack state struct type val
    "#,
    &[
      (Token::Abstract, "abstract"),
      (Token::And, "and"),
      (Token::Apply, "apply"),
      (Token::Enum, "enum"),
      (Token::Ext, "ext"),
      (Token::Fn, "fn"),
      (Token::Fun, "fun"),
      (Token::Group, "group"),
      (Token::Imu, "imu"),
      (Token::Load, "load"),
      (Token::Mut, "mut"),
      (Token::Pack, "pack"),
      (Token::State, "state"),
      (Token::Struct, "struct"),
      (Token::Type, "type"),
      (Token::Val, "val"),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn test_control_flow_keywords() {
  assert_tokens_stream(
    r#"
      loop while for if else when
      break continue return match
    "#,
    &[
      (Token::Loop, "loop"),
      (Token::While, "while"),
      (Token::For, "for"),
      (Token::If, "if"),
      (Token::Else, "else"),
      (Token::When, "when"),
      (Token::Break, "break"),
      (Token::Continue, "continue"),
      (Token::Return, "return"),
      (Token::Match, "match"),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn test_types_keywords() {
  assert_tokens_stream(
    r#"
      int s8 s16 s32 s64
      uint u8 u16 u32 u64
      float f32 f64
      bool bytes char str
    "#,
    &[
      (Token::IntType, "int"),
      (Token::S8Type, "s8"),
      (Token::S16Type, "s16"),
      (Token::S32Type, "s32"),
      (Token::S64Type, "s64"),
      (Token::UintType, "uint"),
      (Token::U8Type, "u8"),
      (Token::U16Type, "u16"),
      (Token::U32Type, "u32"),
      (Token::U64Type, "u64"),
      (Token::FloatType, "float"),
      (Token::F32Type, "f32"),
      (Token::F64Type, "f64"),
      (Token::BoolType, "bool"),
      (Token::BytesType, "bytes"),
      (Token::CharType, "char"),
      (Token::StrType, "str"),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn test_others_keywords() {
  assert_tokens_stream(
    r#"
      pub raw wasm nursery spawn await
      as is self Self true false
    "#,
    &[
      (Token::Pub, "pub"),
      (Token::Raw, "raw"),
      (Token::Wasm, "wasm"),
      (Token::Nursery, "nursery"),
      (Token::Spawn, "spawn"),
      (Token::Await, "await"),
      (Token::As, "as"),
      (Token::Is, "is"),
      (Token::SelfLower, "self"),
      (Token::SelfUpper, "Self"),
      (Token::True, "true"),
      (Token::False, "false"),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn test_integers() {
  assert_tokens_stream(
    r#"
      123 0xFF 0b101 0o77 1_000
      0x1A3F 0b1101_0101 0o123_456 42 9_876_543
    "#,
    &[
      (Token::Int, "123"),
      (Token::Int, "0xFF"),
      (Token::Int, "0b101"),
      (Token::Int, "0o77"),
      (Token::Int, "1_000"),
      (Token::Int, "0x1A3F"),
      (Token::Int, "0b1101_0101"),
      (Token::Int, "0o123_456"),
      (Token::Int, "42"),
      (Token::Int, "9_876_543"),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn test_base_integers() {
  assert_tokens_stream(
    r#"
      b#0 b#101 b#110011
      o#7 o#123 o#7654
      x#F x#1A3 x#deadbeef
    "#,
    &[
      (Token::Int, "b#0"),
      (Token::Int, "b#101"),
      (Token::Int, "b#110011"),
      (Token::Int, "o#7"),
      (Token::Int, "o#123"),
      (Token::Int, "o#7654"),
      (Token::Int, "x#F"),
      (Token::Int, "x#1A3"),
      (Token::Int, "x#deadbeef"),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn test_floats() {
  assert_tokens_stream(
    r#"
      1.0 0.123 1e10 1.2e-5 3.14
      2.71828 0.0 10_000.0 6.022e23 1_000.000_1
    "#,
    &[
      (Token::Float, "1.0"),
      (Token::Float, "0.123"),
      (Token::Float, "1e10"),
      (Token::Float, "1.2e-5"),
      (Token::Float, "3.14"),
      (Token::Float, "2.71828"),
      (Token::Float, "0.0"),
      (Token::Float, "10_000.0"),
      (Token::Float, "6.022e23"),
      (Token::Float, "1_000.000_1"),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn test_consecutive_underscores_in_numbers() {
  assert_tokens_stream(
    "1__000 0x__FF 0b__11",
    &[
      (Token::Int, "1__000"),
      (Token::Int, "0x__FF"),
      (Token::Int, "0b__11"),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn test_hex_mixed_case() {
  assert_tokens_stream(
    "0xAbCdEf 0xABC 0xabc",
    &[
      (Token::Int, "0xAbCdEf"),
      (Token::Int, "0xABC"),
      (Token::Int, "0xabc"),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn test_number_edge_cases() {
  assert_tokens_stream(
    r#"
        0 00 0_0 1_2_3_4_5
        0x0 0xFFFF_FFFF 0xdead_beef
        0b0 0b1111_0000_1111_0000
        0o0 0o777_777
        0.0 00.00 1_234.567_890
        1e0 1E0 1e+10 1E-10 0.5e-5
      "#,
    &[
      // Zero variations
      (Token::Int, "0"),
      (Token::Int, "00"),
      (Token::Int, "0_0"),
      (Token::Int, "1_2_3_4_5"),
      // Hex edge cases
      (Token::Int, "0x0"),
      (Token::Int, "0xFFFF_FFFF"),
      (Token::Int, "0xdead_beef"),
      (Token::Int, "0b0"),
      (Token::Int, "0b1111_0000_1111_0000"),
      // Octal edge cases
      (Token::Int, "0o0"),
      (Token::Int, "0o777_777"),
      // Float edge cases
      (Token::Float, "0.0"),
      (Token::Float, "00.00"),
      (Token::Float, "1_234.567_890"),
      // Scientific notation edge cases
      (Token::Float, "1e0"),
      (Token::Float, "1E0"),
      (Token::Float, "1e+10"),
      (Token::Float, "1E-10"),
      (Token::Float, "0.5e-5"),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn test_chars() {
  assert_tokens_stream(
    "'a' 'Z' '\\0' '\\n' '\\t' '\\\\'",
    &[
      (Token::Char, "'a'"),
      (Token::Char, "'Z'"),
      (Token::Char, "'\\0'"),
      (Token::Char, "'\\n'"),
      (Token::Char, "'\\t'"),
      (Token::Char, "'\\\\'"),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn test_bytes() {
  assert_tokens_stream(
    "`` `hello` `abc123` `test` `123` `hello world` `\n\t`",
    &[
      (Token::Bytes, "``"),
      (Token::Bytes, "`hello`"),
      (Token::Bytes, "`abc123`"),
      (Token::Bytes, "`test`"),
      (Token::Bytes, "`123`"),
      (Token::Bytes, "`hello world`"),
      (Token::Bytes, "`\n\t`"),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn test_strings() {
  assert_tokens_stream(
    r#"
      "" "hello" "esc\n"
      $"" $"hello\n" $"raw\n"
    "#,
    &[
      (Token::String, "\"\""),
      (Token::String, "\"hello\""),
      (Token::String, "\"esc\\n\""),
      (Token::RawString, "$\"\""),
      (Token::RawString, "$\"hello\\n\""),
      (Token::RawString, "$\"raw\\n\""),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn test_raw_strings_special_chars() {
  assert_tokens_stream(
    r#"
      $"hello world" $"with'single" $"with`backticks`"
      $"path\to\file" $"tab space"
    "#,
    &[
      (Token::RawString, "$\"hello world\""),
      (Token::RawString, "$\"with'single\""),
      (Token::RawString, "$\"with`backticks`\""),
      (Token::RawString, "$\"path\\to\\file\""),
      (Token::RawString, "$\"tab space\""),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn test_escape_sequences_strings() {
  assert_tokens_stream(
    r#""\n" "\r" "\t" "\0" "\\""#,
    &[
      (Token::String, "\"\\n\""),
      (Token::String, "\"\\r\""),
      (Token::String, "\"\\t\""),
      (Token::String, "\"\\0\""),
      (Token::String, "\"\\\\\""),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn test_fragment_templates() {
  assert_tokens_stream(
    r#"
      ::= <></>;
    "#,
    &[
      (Token::TemplateAssign, "::="),
      (Token::TemplateFragmentStart, "<>"),
      (Token::TemplateFragmentEnd, "</>"),
      (Token::Semicolon, ";"),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn test_text_templates() {
  assert_tokens_stream(
    r#"
      ::= <>hello world</>;
    "#,
    &[
      (Token::TemplateAssign, "::="),
      (Token::TemplateFragmentStart, "<>"),
      (Token::TemplateText, "hello world"),
      (Token::TemplateFragmentEnd, "</>"),
      (Token::Semicolon, ";"),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn test_interpolation_templates() {
  assert_tokens_stream(
    r#"
      ::= <p>{user.name}</p>;
    "#,
    &[
      (Token::TemplateAssign, "::="),
      (Token::LAngle, "<"),
      (Token::Ident, "p"),
      (Token::RAngle, ">"),
      (Token::LBrace, "{"),
      (Token::Ident, "user"),
      (Token::Dot, "."),
      (Token::Ident, "name"),
      (Token::RBrace, "}"),
      (Token::LAngle, "<"),
      (Token::Slash2, "/"),
      (Token::Ident, "p"),
      (Token::RAngle, ">"),
      (Token::Semicolon, ";"),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn test_nested_code_templates() {
  assert_tokens_stream(
    "::= <div class={if x { \"a\" } else { \"b\" }} />;",
    &[
      (Token::TemplateAssign, "::="),
      (Token::LAngle, "<"),
      (Token::Ident, "div"),
      (Token::Ident, "class"),
      (Token::Eq, "="),
      (Token::LBrace, "{"),
      (Token::If, "if"),
      (Token::Ident, "x"),
      (Token::LBrace, "{"),
      (Token::String, "\"a\""),
      (Token::RBrace, "}"),
      (Token::Else, "else"),
      (Token::LBrace, "{"),
      (Token::String, "\"b\""),
      (Token::RBrace, "}"),
      (Token::RBrace, "}"),
      (Token::Slash2, "/"),
      (Token::RAngle, ">"),
      (Token::Semicolon, ";"),
      (Token::Eof, ""),
    ],
  );
}
