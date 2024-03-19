use super::token::group::Group;
use super::token::kw::Kw;
use super::token::op::Op;
use super::token::punctuation::Punctuation;
use super::token::{Token, TokenKind};
use super::tokenizer;

use zhoo_reader::reader;
use zhoo_session::session::Session;

use zo_core::interner::symbol::Symbol;
use zo_core::span::Span;

#[test]
fn tokenize_empty() {
  let mut session = Session::default();
  let source = "".as_bytes();

  tokenizer::tokenize(&mut session, source)
    .map(|tokens| assert!(tokens.len() == 0))
    .unwrap();
}

#[allow(dead_code)]
// #[test]
fn tokenize_atlas() {
  let mut session = Session::default();

  session.settings.input = "../zhoo-notes/samples/bench/atlas.tks".into();

  let source = reader::read_file(&mut session).unwrap();

  let expected = vec![
    Token {
      kind: TokenKind::Int(Symbol(0)),
      span: Span::of(59, 60),
    },
    Token {
      kind: TokenKind::Int(Symbol(1)),
      span: Span::of(61, 62),
    },
    Token {
      kind: TokenKind::Int(Symbol(2)),
      span: Span::of(63, 64),
    },
    Token {
      kind: TokenKind::Int(Symbol(3)),
      span: Span::of(65, 66),
    },
    Token {
      kind: TokenKind::Int(Symbol(4)),
      span: Span::of(67, 68),
    },
    Token {
      kind: TokenKind::Int(Symbol(5)),
      span: Span::of(69, 70),
    },
    Token {
      kind: TokenKind::Int(Symbol(6)),
      span: Span::of(71, 72),
    },
    Token {
      kind: TokenKind::Int(Symbol(7)),
      span: Span::of(73, 74),
    },
    Token {
      kind: TokenKind::Int(Symbol(8)),
      span: Span::of(75, 76),
    },
    Token {
      kind: TokenKind::Int(Symbol(9)),
      span: Span::of(77, 78),
    },
    Token {
      kind: TokenKind::Int(Symbol(10)),
      span: Span::of(79, 81),
    },
    Token {
      kind: TokenKind::Int(Symbol(11)),
      span: Span::of(82, 85),
    },
    Token {
      kind: TokenKind::Int(Symbol(12)),
      span: Span::of(86, 90),
    },
    Token {
      kind: TokenKind::Int(Symbol(13)),
      span: Span::of(91, 96),
    },
    Token {
      kind: TokenKind::Int(Symbol(14)),
      span: Span::of(97, 104),
    },
    Token {
      kind: TokenKind::Int(Symbol(15)),
      span: Span::of(105, 114),
    },
    Token {
      kind: TokenKind::Float(Symbol(16)),
      span: Span::of(115, 118),
    },
    Token {
      kind: TokenKind::Float(Symbol(17)),
      span: Span::of(119, 123),
    },
    Token {
      kind: TokenKind::Float(Symbol(18)),
      span: Span::of(124, 129),
    },
    Token {
      kind: TokenKind::Float(Symbol(19)),
      span: Span::of(130, 136),
    },
    Token {
      kind: TokenKind::Float(Symbol(20)),
      span: Span::of(137, 144),
    },
    Token {
      kind: TokenKind::Float(Symbol(21)),
      span: Span::of(145, 154),
    },
    Token {
      kind: TokenKind::Float(Symbol(22)),
      span: Span::of(155, 166),
    },
    Token {
      kind: TokenKind::Int(Symbol(23)),
      span: Span::of(168, 176),
    },
    Token {
      kind: TokenKind::Ident(Symbol(24)),
      span: Span::of(176, 179),
    },
    Token {
      kind: TokenKind::Op(Op::Equal),
      span: Span::of(181, 182),
    },
    Token {
      kind: TokenKind::Op(Op::Plus),
      span: Span::of(183, 184),
    },
    Token {
      kind: TokenKind::Op(Op::Minus),
      span: Span::of(185, 186),
    },
    Token {
      kind: TokenKind::Op(Op::Asterisk),
      span: Span::of(187, 188),
    },
    Token {
      kind: TokenKind::Op(Op::Slash),
      span: Span::of(189, 190),
    },
    Token {
      kind: TokenKind::Op(Op::Percent),
      span: Span::of(191, 192),
    },
    Token {
      kind: TokenKind::Op(Op::Circumflex),
      span: Span::of(193, 194),
    },
    Token {
      kind: TokenKind::Op(Op::Exclamation),
      span: Span::of(195, 196),
    },
    Token {
      kind: TokenKind::Op(Op::EqualEqual),
      span: Span::of(197, 199),
    },
    Token {
      kind: TokenKind::Op(Op::PlusEqual),
      span: Span::of(200, 202),
    },
    Token {
      kind: TokenKind::Op(Op::MinusEqual),
      span: Span::of(203, 205),
    },
    Token {
      kind: TokenKind::Op(Op::AsteriskEqual),
      span: Span::of(206, 208),
    },
    Token {
      kind: TokenKind::Op(Op::SlashEqual),
      span: Span::of(209, 211),
    },
    Token {
      kind: TokenKind::Op(Op::PercentEqual),
      span: Span::of(212, 214),
    },
    Token {
      kind: TokenKind::Op(Op::CircumflexEqual),
      span: Span::of(215, 217),
    },
    Token {
      kind: TokenKind::Op(Op::ExclamationEqual),
      span: Span::of(218, 220),
    },
    Token {
      kind: TokenKind::Op(Op::AmspersandEqual),
      span: Span::of(221, 223),
    },
    Token {
      kind: TokenKind::Op(Op::PipeEqual),
      span: Span::of(224, 226),
    },
    Token {
      kind: TokenKind::Op(Op::LessThanEqual),
      span: Span::of(227, 229),
    },
    Token {
      kind: TokenKind::Op(Op::GreaterThanEqual),
      span: Span::of(230, 232),
    },
    Token {
      kind: TokenKind::Op(Op::LessThanLessThanEqual),
      span: Span::of(233, 236),
    },
    Token {
      kind: TokenKind::Op(Op::GreaterThan),
      span: Span::of(239, 240),
    },
    Token {
      kind: TokenKind::Op(Op::LessThanLessThan),
      span: Span::of(241, 243),
    },
    Token {
      kind: TokenKind::Op(Op::GreaterThanGreaterThan),
      span: Span::of(244, 246),
    },
    Token {
      kind: TokenKind::Op(Op::AmpersandAmpersand),
      span: Span::of(247, 249),
    },
    Token {
      kind: TokenKind::Op(Op::PipePipe),
      span: Span::of(250, 252),
    },
    Token {
      kind: TokenKind::Punctuation(Punctuation::MinusGreaterThan),
      span: Span::of(253, 255),
    },
    Token {
      kind: TokenKind::Punctuation(Punctuation::EqualGreaterThan),
      span: Span::of(256, 258),
    },
    Token {
      kind: TokenKind::Group(Group::ParenOpen),
      span: Span::of(259, 260),
    },
    Token {
      kind: TokenKind::Group(Group::ParenClose),
      span: Span::of(260, 261),
    },
    Token {
      kind: TokenKind::Group(Group::BraceOpen),
      span: Span::of(262, 263),
    },
    Token {
      kind: TokenKind::Group(Group::BraceClose),
      span: Span::of(263, 264),
    },
    Token {
      kind: TokenKind::Group(Group::BracketOpen),
      span: Span::of(265, 266),
    },
    Token {
      kind: TokenKind::Group(Group::BracketClose),
      span: Span::of(266, 267),
    },
    Token {
      kind: TokenKind::Punctuation(Punctuation::Comma),
      span: Span::of(268, 269),
    },
    Token {
      kind: TokenKind::Punctuation(Punctuation::Period),
      span: Span::of(270, 271),
    },
    Token {
      kind: TokenKind::Punctuation(Punctuation::Colon),
      span: Span::of(272, 273),
    },
    Token {
      kind: TokenKind::Punctuation(Punctuation::Semicolon),
      span: Span::of(274, 275),
    },
    Token {
      kind: TokenKind::Punctuation(Punctuation::ColonColon),
      span: Span::of(276, 278),
    },
    Token {
      kind: TokenKind::Punctuation(Punctuation::EqualGreaterThan),
      span: Span::of(237, 239),
    },
    Token {
      kind: TokenKind::Kw(Kw::Abstract),
      span: Span::of(280, 288),
    },
    Token {
      kind: TokenKind::Kw(Kw::Apply),
      span: Span::of(289, 294),
    },
    Token {
      kind: TokenKind::Kw(Kw::Async),
      span: Span::of(296, 301),
    },
    Token {
      kind: TokenKind::Kw(Kw::Await),
      span: Span::of(303, 308),
    },
    Token {
      kind: TokenKind::Kw(Kw::Break),
      span: Span::of(309, 314),
    },
    Token {
      kind: TokenKind::Kw(Kw::Continue),
      span: Span::of(315, 323),
    },
    Token {
      kind: TokenKind::Kw(Kw::Else),
      span: Span::of(324, 328),
    },
    Token {
      kind: TokenKind::Kw(Kw::Enum),
      span: Span::of(330, 334),
    },
    Token {
      kind: TokenKind::Kw(Kw::Ext),
      span: Span::of(335, 338),
    },
    Token {
      kind: TokenKind::Kw(Kw::Fn),
      span: Span::of(339, 341),
    },
    Token {
      kind: TokenKind::Kw(Kw::For),
      span: Span::of(342, 345),
    },
    Token {
      kind: TokenKind::Kw(Kw::Fun),
      span: Span::of(351, 354),
    },
    Token {
      kind: TokenKind::Kw(Kw::If),
      span: Span::of(358, 360),
    },
    Token {
      kind: TokenKind::Kw(Kw::Imu),
      span: Span::of(365, 368),
    },
    Token {
      kind: TokenKind::Kw(Kw::Load),
      span: Span::of(371, 375),
    },
    Token {
      kind: TokenKind::Kw(Kw::Loop),
      span: Span::of(377, 381),
    },
    Token {
      kind: TokenKind::Kw(Kw::Match),
      span: Span::of(386, 391),
    },
    Token {
      kind: TokenKind::Kw(Kw::Me),
      span: Span::of(392, 394),
    },
    Token {
      kind: TokenKind::Kw(Kw::Mut),
      span: Span::of(397, 400),
    },
    Token {
      kind: TokenKind::Kw(Kw::Pack),
      span: Span::of(401, 405),
    },
    Token {
      kind: TokenKind::Kw(Kw::Pub),
      span: Span::of(406, 409),
    },
    Token {
      kind: TokenKind::Kw(Kw::Return),
      span: Span::of(415, 421),
    },
    Token {
      kind: TokenKind::Kw(Kw::Struct),
      span: Span::of(422, 428),
    },
    Token {
      kind: TokenKind::Kw(Kw::Type),
      span: Span::of(429, 433),
    },
    Token {
      kind: TokenKind::Kw(Kw::Val),
      span: Span::of(435, 438),
    },
    Token {
      kind: TokenKind::Kw(Kw::Wasm),
      span: Span::of(441, 445),
    },
    Token {
      kind: TokenKind::Kw(Kw::While),
      span: Span::of(450, 455),
    },
    Token {
      kind: TokenKind::Char(Symbol(25)),
      span: Span::of(457, 460),
    },
    Token {
      kind: TokenKind::Char(Symbol(26)),
      span: Span::of(461, 465),
    },
    Token {
      kind: TokenKind::Char(Symbol(27)),
      span: Span::of(466, 470),
    },
    Token {
      kind: TokenKind::Char(Symbol(28)),
      span: Span::of(471, 475),
    },
    Token {
      kind: TokenKind::Char(Symbol(29)),
      span: Span::of(476, 481),
    },
    Token {
      kind: TokenKind::Str(Symbol(30)),
      span: Span::of(483, 502),
    },
    Token {
      kind: TokenKind::Str(Symbol(31)),
      span: Span::of(503, 521),
    },
    Token {
      kind: TokenKind::Str(Symbol(32)),
      span: Span::of(522, 532),
    },
  ];

  tokenizer::tokenize(&mut session, &source)
    // todo (ivs) — compare by tokens.
    .map(|tokens| assert!(tokens == expected))
    .unwrap();
}
