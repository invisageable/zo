use super::token::group::Group;
use super::token::int::BaseInt;
use super::token::kw::Kw;
use super::token::op::Op;
use super::token::punctuation::Punctuation;
use super::token::{Token, TokenKind};
use super::tokenizer;

use zo_reader::reader;
use zo_session::session::Session;

use zo_core::interner::symbol::Symbol;
use zo_core::span::Span;

#[test]
fn tokenize_empty() {
  let mut session = Session::default();

  session.settings.input = "../zo-notes/samples/test/empty.zo".into();

  let source = reader::read_file(&mut session).unwrap();

  tokenizer::tokenize(&mut session, &source)
    .map(|tokens| assert!(tokens.len() == 0))
    .unwrap();
}

#[test]
fn tokenize_comments() {
  let mut session = Session::default();

  session.settings.input = "../zo-notes/samples/test/tokens/comments.zo".into();

  let source = reader::read_file(&mut session).unwrap();

  tokenizer::tokenize(&mut session, &source)
    .map(|tokens| assert!(tokens.len() == 0))
    .unwrap();
}

#[test]
fn tokenize_groups() {
  let mut session = Session::default();

  session.settings.input = "../zo-notes/samples/test/tokens/groups.zo".into();

  let source = reader::read_file(&mut session).unwrap();

  let expected = vec![
    Token::new(TokenKind::Group(Group::ParenOpen), Span::of(22, 23)),
    Token::new(TokenKind::Group(Group::ParenClose), Span::of(23, 24)),
    Token::new(TokenKind::Group(Group::BraceOpen), Span::of(25, 26)),
    Token::new(TokenKind::Group(Group::BraceClose), Span::of(26, 27)),
    Token::new(TokenKind::Group(Group::BracketOpen), Span::of(28, 29)),
    Token::new(TokenKind::Group(Group::BracketClose), Span::of(29, 30)),
  ];

  tokenizer::tokenize(&mut session, &source)
    .map(|tokens| assert!(tokens == expected))
    .unwrap();
}

#[test]
fn tokenize_operators() {
  let mut session = Session::default();

  session.settings.input =
    "../zo-notes/samples/test/tokens/operators.zo".into();

  let source = reader::read_file(&mut session).unwrap();

  let expected = vec![
    Token::new(TokenKind::Op(Op::Equal), Span::of(25, 26)),
    Token::new(TokenKind::Op(Op::Plus), Span::of(29, 30)),
    Token::new(TokenKind::Op(Op::Minus), Span::of(33, 34)),
    Token::new(TokenKind::Op(Op::Asterisk), Span::of(37, 38)),
    Token::new(TokenKind::Op(Op::Slash), Span::of(41, 42)),
    Token::new(TokenKind::Op(Op::Percent), Span::of(45, 46)),
    Token::new(TokenKind::Op(Op::Circumflex), Span::of(49, 50)),
    Token::new(TokenKind::Op(Op::Exclamation), Span::of(53, 54)),
    Token::new(TokenKind::Op(Op::Ampersand), Span::of(57, 58)),
    Token::new(TokenKind::Op(Op::Pipe), Span::of(61, 62)),
    Token::new(TokenKind::Op(Op::LessThan), Span::of(65, 66)),
    Token::new(TokenKind::Op(Op::GreaterThan), Span::of(69, 70)),
    Token::new(TokenKind::Op(Op::LessThanEqual), Span::of(73, 75)),
    Token::new(TokenKind::Op(Op::GreaterThanEqual), Span::of(77, 79)),
    Token::new(TokenKind::Op(Op::EqualEqual), Span::of(80, 82)),
    Token::new(TokenKind::Op(Op::PlusEqual), Span::of(84, 86)),
    Token::new(TokenKind::Op(Op::MinusEqual), Span::of(88, 90)),
    Token::new(TokenKind::Op(Op::AsteriskEqual), Span::of(92, 94)),
    Token::new(TokenKind::Op(Op::SlashEqual), Span::of(96, 98)),
    Token::new(TokenKind::Op(Op::PercentEqual), Span::of(100, 102)),
    Token::new(TokenKind::Op(Op::CircumflexEqual), Span::of(104, 106)),
    Token::new(TokenKind::Op(Op::ExclamationEqual), Span::of(108, 110)),
    Token::new(TokenKind::Op(Op::AmspersandEqual), Span::of(112, 114)),
    Token::new(TokenKind::Op(Op::PipeEqual), Span::of(116, 118)),
    Token::new(TokenKind::Op(Op::LessThanLessThanEqual), Span::of(120, 123)),
    Token::new(
      TokenKind::Op(Op::GreaterThanGreaterThanEqual),
      Span::of(124, 127),
    ),
    Token::new(TokenKind::Op(Op::LessThanLessThan), Span::of(128, 130)),
    Token::new(
      TokenKind::Op(Op::GreaterThanGreaterThan),
      Span::of(132, 134),
    ),
    Token::new(TokenKind::Op(Op::AmpersandAmpersand), Span::of(135, 137)),
    Token::new(TokenKind::Op(Op::PipePipe), Span::of(138, 140)),
  ];

  tokenizer::tokenize(&mut session, &source)
    .map(|tokens| assert!(tokens == expected))
    .unwrap();
}

#[test]
fn tokenize_punctuations() {
  let mut session = Session::default();

  session.settings.input =
    "../zo-notes/samples/test/tokens/punctuations.zo".into();

  let source = reader::read_file(&mut session).unwrap();

  let expected = vec![
    Token::new(TokenKind::Punctuation(Punctuation::Comma), Span::of(35, 36)),
    Token::new(
      TokenKind::Punctuation(Punctuation::Period),
      Span::of(37, 38),
    ),
    Token::new(TokenKind::Punctuation(Punctuation::Colon), Span::of(39, 40)),
    Token::new(
      TokenKind::Punctuation(Punctuation::ColonColon),
      Span::of(41, 43),
    ),
    Token::new(
      TokenKind::Punctuation(Punctuation::Semicolon),
      Span::of(44, 45),
    ),
    Token::new(
      TokenKind::Punctuation(Punctuation::MinusGreaterThan),
      Span::of(46, 48),
    ),
    Token::new(
      TokenKind::Punctuation(Punctuation::EqualGreaterThan),
      Span::of(49, 51),
    ),
  ];

  tokenizer::tokenize(&mut session, &source)
    .map(|tokens| assert!(tokens == expected))
    .unwrap();
}

#[test]
fn tokenize_integers() {
  let mut session = Session::default();

  session.settings.input =
    "../zo-notes/samples/test/tokens/numbers/integers.zo".into();

  let source = reader::read_file(&mut session).unwrap();

  let expected = vec![
    Token::new(TokenKind::Int(Symbol(0), BaseInt::Dec), Span::of(33, 34)),
    Token::new(TokenKind::Int(Symbol(1), BaseInt::Dec), Span::of(35, 36)),
    Token::new(TokenKind::Int(Symbol(2), BaseInt::Dec), Span::of(37, 38)),
    Token::new(TokenKind::Int(Symbol(3), BaseInt::Dec), Span::of(39, 40)),
    Token::new(TokenKind::Int(Symbol(4), BaseInt::Dec), Span::of(41, 42)),
    Token::new(TokenKind::Int(Symbol(5), BaseInt::Dec), Span::of(43, 44)),
    Token::new(TokenKind::Int(Symbol(6), BaseInt::Dec), Span::of(45, 46)),
    Token::new(TokenKind::Int(Symbol(7), BaseInt::Dec), Span::of(47, 48)),
    Token::new(TokenKind::Int(Symbol(8), BaseInt::Dec), Span::of(49, 50)),
    Token::new(TokenKind::Int(Symbol(9), BaseInt::Dec), Span::of(51, 52)),
    Token::new(TokenKind::Int(Symbol(10), BaseInt::Dec), Span::of(53, 55)),
    Token::new(TokenKind::Int(Symbol(11), BaseInt::Dec), Span::of(56, 59)),
    Token::new(TokenKind::Int(Symbol(12), BaseInt::Dec), Span::of(60, 64)),
    Token::new(TokenKind::Int(Symbol(13), BaseInt::Dec), Span::of(65, 70)),
    Token::new(TokenKind::Int(Symbol(14), BaseInt::Dec), Span::of(71, 78)),
    Token::new(TokenKind::Int(Symbol(15), BaseInt::Dec), Span::of(79, 88)),
  ];

  tokenizer::tokenize(&mut session, &source)
    .map(|tokens| assert!(tokens == expected))
    .unwrap();
}

#[test]
fn tokenize_hexadecimals() {
  let mut session = Session::default();

  session.settings.input =
    "../zo-notes/samples/test/tokens/numbers/hexadecimals.zo".into();

  let source = reader::read_file(&mut session).unwrap();

  let expected = vec![
    Token::new(TokenKind::Int(Symbol(0), BaseInt::Hex), Span::of(37, 43)),
    Token::new(TokenKind::Int(Symbol(1), BaseInt::Hex), Span::of(44, 48)),
    Token::new(TokenKind::Int(Symbol(2), BaseInt::Hex), Span::of(49, 57)),
    Token::new(TokenKind::Int(Symbol(3), BaseInt::Hex), Span::of(58, 68)),
    Token::new(TokenKind::Int(Symbol(4), BaseInt::Hex), Span::of(69, 73)),
    Token::new(TokenKind::Int(Symbol(5), BaseInt::Hex), Span::of(74, 78)),
    Token::new(TokenKind::Int(Symbol(6), BaseInt::Hex), Span::of(79, 85)),
    Token::new(TokenKind::Int(Symbol(7), BaseInt::Hex), Span::of(86, 92)),
    Token::new(TokenKind::Int(Symbol(8), BaseInt::Hex), Span::of(93, 97)),
    Token::new(TokenKind::Int(Symbol(9), BaseInt::Hex), Span::of(98, 106)),
    Token::new(TokenKind::Int(Symbol(10), BaseInt::Hex), Span::of(107, 115)),
    Token::new(TokenKind::Int(Symbol(11), BaseInt::Hex), Span::of(116, 120)),
  ];

  tokenizer::tokenize(&mut session, &source)
    .map(|tokens| assert!(tokens == expected))
    .unwrap();
}

#[test]
fn tokenize_octals() {
  let mut session = Session::default();

  session.settings.input =
    "../zo-notes/samples/test/tokens/numbers/octals.zo".into();

  let source = reader::read_file(&mut session).unwrap();

  let expected = vec![
    Token::new(TokenKind::Int(Symbol(0), BaseInt::Oct), Span::of(31, 35)),
    Token::new(TokenKind::Int(Symbol(1), BaseInt::Oct), Span::of(36, 41)),
    Token::new(TokenKind::Int(Symbol(2), BaseInt::Oct), Span::of(42, 47)),
    Token::new(TokenKind::Int(Symbol(3), BaseInt::Oct), Span::of(48, 54)),
    Token::new(TokenKind::Int(Symbol(4), BaseInt::Oct), Span::of(55, 59)),
    Token::new(TokenKind::Int(Symbol(5), BaseInt::Oct), Span::of(60, 72)),
    Token::new(TokenKind::Int(Symbol(6), BaseInt::Oct), Span::of(73, 78)),
    Token::new(TokenKind::Int(Symbol(7), BaseInt::Oct), Span::of(79, 86)),
    Token::new(TokenKind::Int(Symbol(8), BaseInt::Oct), Span::of(87, 91)),
    Token::new(TokenKind::Int(Symbol(9), BaseInt::Oct), Span::of(92, 97)),
    Token::new(TokenKind::Int(Symbol(10), BaseInt::Oct), Span::of(98, 102)),
    Token::new(TokenKind::Int(Symbol(11), BaseInt::Oct), Span::of(103, 107)),
  ];

  tokenizer::tokenize(&mut session, &source)
    .map(|tokens| assert!(tokens == expected))
    .unwrap();
}

#[test]
fn tokenize_binaries() {
  let mut session = Session::default();

  session.settings.input =
    "../zo-notes/samples/test/tokens/numbers/binaries.zo".into();

  let source = reader::read_file(&mut session).unwrap();

  let expected = vec![
    Token::new(TokenKind::Int(Symbol(0), BaseInt::Bin), Span::of(33, 44)),
    Token::new(TokenKind::Int(Symbol(1), BaseInt::Bin), Span::of(45, 50)),
    Token::new(TokenKind::Int(Symbol(2), BaseInt::Bin), Span::of(51, 59)),
    Token::new(TokenKind::Int(Symbol(3), BaseInt::Bin), Span::of(60, 72)),
    Token::new(TokenKind::Int(Symbol(4), BaseInt::Bin), Span::of(73, 84)),
    Token::new(TokenKind::Int(Symbol(5), BaseInt::Bin), Span::of(85, 98)),
    Token::new(TokenKind::Int(Symbol(6), BaseInt::Bin), Span::of(99, 104)),
    Token::new(TokenKind::Int(Symbol(7), BaseInt::Bin), Span::of(105, 113)),
    Token::new(TokenKind::Int(Symbol(8), BaseInt::Bin), Span::of(114, 148)),
    Token::new(TokenKind::Int(Symbol(9), BaseInt::Bin), Span::of(149, 155)),
    Token::new(TokenKind::Int(Symbol(10), BaseInt::Bin), Span::of(156, 162)),
    Token::new(TokenKind::Int(Symbol(11), BaseInt::Bin), Span::of(163, 169)),
  ];

  tokenizer::tokenize(&mut session, &source)
    .map(|tokens| assert!(tokens == expected))
    .unwrap();
}

#[test]
fn tokenize_floats() {
  let mut session = Session::default();

  session.settings.input =
    "../zo-notes/samples/test/tokens/numbers/floats.zo".into();

  let source = reader::read_file(&mut session).unwrap();

  let expected = vec![
    Token::new(TokenKind::Float(Symbol(0)), Span::of(31, 34)),
    Token::new(TokenKind::Float(Symbol(1)), Span::of(35, 39)),
    Token::new(TokenKind::Float(Symbol(2)), Span::of(40, 45)),
    Token::new(TokenKind::Float(Symbol(3)), Span::of(46, 52)),
    Token::new(TokenKind::Float(Symbol(4)), Span::of(53, 60)),
    Token::new(TokenKind::Float(Symbol(5)), Span::of(61, 70)),
    Token::new(TokenKind::Float(Symbol(6)), Span::of(71, 82)),
    Token::new(TokenKind::Float(Symbol(7)), Span::of(83, 91)),
    Token::new(TokenKind::Float(Symbol(8)), Span::of(92, 97)),
    Token::new(TokenKind::Float(Symbol(9)), Span::of(98, 104)),
    Token::new(TokenKind::Float(Symbol(10)), Span::of(105, 112)),
    Token::new(TokenKind::Float(Symbol(11)), Span::of(113, 122)),
    Token::new(TokenKind::Float(Symbol(12)), Span::of(123, 134)),
  ];

  tokenizer::tokenize(&mut session, &source)
    .map(|tokens| assert!(tokens == expected))
    .unwrap();
}

#[test]
fn tokenize_keywords() {
  let mut session = Session::default();

  session.settings.input = "../zo-notes/samples/test/tokens/keywords.zo".into();

  let source = reader::read_file(&mut session).unwrap();

  let expected = vec![
    Token::new(TokenKind::Kw(Kw::Abstract), Span::of(24, 32)),
    Token::new(TokenKind::Kw(Kw::Apply), Span::of(33, 38)),
    Token::new(TokenKind::Kw(Kw::Async), Span::of(40, 45)),
    Token::new(TokenKind::Kw(Kw::Await), Span::of(47, 52)),
    Token::new(TokenKind::Kw(Kw::Break), Span::of(53, 58)),
    Token::new(TokenKind::Kw(Kw::Continue), Span::of(59, 67)),
    Token::new(TokenKind::Kw(Kw::Else), Span::of(68, 72)),
    Token::new(TokenKind::Kw(Kw::Enum), Span::of(74, 78)),
    Token::new(TokenKind::Kw(Kw::Ext), Span::of(79, 82)),
    Token::new(TokenKind::Kw(Kw::Fn), Span::of(83, 85)),
    Token::new(TokenKind::Kw(Kw::For), Span::of(86, 89)),
    Token::new(TokenKind::Kw(Kw::Fun), Span::of(95, 98)),
    Token::new(TokenKind::Kw(Kw::If), Span::of(102, 104)),
    Token::new(TokenKind::Kw(Kw::Imu), Span::of(109, 112)),
    Token::new(TokenKind::Kw(Kw::Load), Span::of(115, 119)),
    Token::new(TokenKind::Kw(Kw::Loop), Span::of(121, 125)),
    Token::new(TokenKind::Kw(Kw::Match), Span::of(130, 135)),
    Token::new(TokenKind::Kw(Kw::Me), Span::of(136, 138)),
    Token::new(TokenKind::Kw(Kw::Mut), Span::of(141, 144)),
    Token::new(TokenKind::Kw(Kw::Pack), Span::of(145, 149)),
    Token::new(TokenKind::Kw(Kw::Pub), Span::of(150, 153)),
    Token::new(TokenKind::Kw(Kw::Return), Span::of(159, 165)),
    Token::new(TokenKind::Kw(Kw::Struct), Span::of(166, 172)),
    Token::new(TokenKind::Kw(Kw::Type), Span::of(173, 177)),
    Token::new(TokenKind::Kw(Kw::Val), Span::of(179, 182)),
    Token::new(TokenKind::Kw(Kw::Wasm), Span::of(185, 189)),
    Token::new(TokenKind::Kw(Kw::While), Span::of(194, 199)),
  ];

  tokenizer::tokenize(&mut session, &source)
    .map(|tokens| assert!(tokens == expected))
    .unwrap();
}

#[test]
fn tokenize_identifiers() {
  let mut session = Session::default();

  session.settings.input =
    "../zo-notes/samples/test/tokens/identifiers.zo".into();

  let source = reader::read_file(&mut session).unwrap();

  let expected = vec![
    Token::new(TokenKind::Ident(Symbol(0)), Span::of(27, 30)),
    Token::new(TokenKind::Ident(Symbol(1)), Span::of(31, 38)),
    Token::new(TokenKind::Ident(Symbol(2)), Span::of(39, 45)),
    Token::new(TokenKind::Ident(Symbol(3)), Span::of(46, 52)),
    Token::new(TokenKind::Ident(Symbol(4)), Span::of(53, 60)),
    Token::new(TokenKind::Ident(Symbol(5)), Span::of(61, 64)),
  ];

  tokenizer::tokenize(&mut session, &source)
    .map(|tokens| assert!(tokens == expected))
    .unwrap();
}

#[test]
fn tokenize_booleans() {
  let mut session = Session::default();

  session.settings.input = "../zo-notes/samples/test/tokens/booleans.zo".into();

  let source = reader::read_file(&mut session).unwrap();

  let expected = vec![
    Token::new(TokenKind::Kw(Kw::True), Span::of(33, 37)),
    Token::new(TokenKind::Kw(Kw::False), Span::of(38, 43)),
  ];

  tokenizer::tokenize(&mut session, &source)
    .map(|tokens| assert!(tokens == expected))
    .unwrap();
}

#[test]
fn tokenize_chars() {
  let mut session = Session::default();

  session.settings.input = "../zo-notes/samples/test/tokens/chars.zo".into();

  let source = reader::read_file(&mut session).unwrap();

  let expected = vec![
    Token::new(TokenKind::Char(Symbol(0)), Span::of(26, 29)),
    Token::new(TokenKind::Char(Symbol(1)), Span::of(30, 34)),
    Token::new(TokenKind::Char(Symbol(2)), Span::of(35, 39)),
    Token::new(TokenKind::Char(Symbol(3)), Span::of(40, 44)),
    Token::new(TokenKind::Char(Symbol(4)), Span::of(45, 50)),
    Token::new(TokenKind::Char(Symbol(5)), Span::of(51, 57)),
    Token::new(TokenKind::Char(Symbol(6)), Span::of(58, 64)),
    Token::new(TokenKind::Char(Symbol(7)), Span::of(65, 70)),
    Token::new(TokenKind::Char(Symbol(8)), Span::of(71, 75)),
    Token::new(TokenKind::Char(Symbol(9)), Span::of(76, 80)),
    Token::new(TokenKind::Char(Symbol(10)), Span::of(81, 84)),
    Token::new(TokenKind::Char(Symbol(11)), Span::of(85, 90)),
  ];

  tokenizer::tokenize(&mut session, &source)
    .map(|tokens| assert!(tokens == expected))
    .unwrap();
}

#[test]
fn tokenize_strings() {
  let mut session = Session::default();

  session.settings.input = "../zo-notes/samples/test/tokens/strings.zo".into();

  let source = reader::read_file(&mut session).unwrap();

  let expected = vec![
    Token::new(TokenKind::Str(Symbol(0)), Span::of(31, 45)),
    Token::new(TokenKind::Str(Symbol(1)), Span::of(46, 65)),
    Token::new(TokenKind::Str(Symbol(2)), Span::of(66, 84)),
    Token::new(TokenKind::Str(Symbol(3)), Span::of(85, 95)),
    Token::new(TokenKind::Str(Symbol(4)), Span::of(96, 100)),
    Token::new(TokenKind::Str(Symbol(5)), Span::of(101, 111)),
    Token::new(TokenKind::Str(Symbol(6)), Span::of(112, 120)),
    Token::new(TokenKind::Str(Symbol(7)), Span::of(121, 135)),
    Token::new(TokenKind::Str(Symbol(8)), Span::of(136, 153)),
    Token::new(TokenKind::Str(Symbol(9)), Span::of(154, 174)),
    Token::new(TokenKind::Str(Symbol(10)), Span::of(175, 200)),
    Token::new(TokenKind::Str(Symbol(11)), Span::of(201, 209)),
  ];

  tokenizer::tokenize(&mut session, &source)
    .map(|tokens| assert!(tokens == expected))
    .unwrap();
}
