//! ...

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Op {
  Equal,
  Plus,
  Minus,
  Asterisk,
  Slash,
  Percent,
  Circumflex,
  Question,
  Exclamation,
  Ampersand,
  Pipe,
  Pound,
  LessThan,
  GreaterThan,
  EqualEqual,
  ExclamationEqual,
  PlusEqual,
  MinusEqual,
  AsteriskEqual,
  SlashEqual,
  PercentEqual,
  AmspersandEqual,
  PipeEqual,
  CircumflexEqual,
  LessThanLessThanEqual,
  GreaterThanGreaterThanEqual,
  ColonEqual,
  LessThanEqual,
  GreaterThanEqual,
  LessThanLessThan,
  GreaterThanGreaterThan,
  AmpersandAmpersand,
  PipePipe,
  Period,
  PeriodPeriod,
}

impl From<char> for Op {
  fn from(op: char) -> Self {
    match op as u8 {
      b'=' => Self::Equal,
      b'+' => Self::Plus,
      b'-' => Self::Minus,
      b'*' => Self::Asterisk,
      b'/' => Self::Slash,
      b'%' => Self::Percent,
      b'^' => Self::Circumflex,
      b'?' => Self::Question,
      b'!' => Self::Exclamation,
      b'&' => Self::Ampersand,
      b'|' => Self::Pipe,
      b'#' => Self::Pound,
      b'<' => Self::LessThan,
      b'>' => Self::GreaterThan,
      b'.' => Self::Period,
      _ => unreachable!("{op}"),
    }
  }
}

impl From<&str> for Op {
  fn from(op: &str) -> Self {
    match op {
      "==" => Self::EqualEqual,
      "!=" => Self::ExclamationEqual,
      "+=" => Self::PlusEqual,
      "-=" => Self::MinusEqual,
      "*=" => Self::AsteriskEqual,
      "/=" => Self::SlashEqual,
      "%=" => Self::PercentEqual,
      "&=" => Self::AmspersandEqual,
      "|=" => Self::PipeEqual,
      "^=" => Self::CircumflexEqual,
      "<<=" => Self::LessThanLessThanEqual,
      ">>=" => Self::GreaterThanGreaterThanEqual,
      ":=" => Self::ColonEqual,
      "<=" => Self::LessThanEqual,
      ">=" => Self::GreaterThanEqual,
      "<<" => Self::LessThanLessThan,
      ">>" => Self::GreaterThanGreaterThan,
      "&&" => Self::AmpersandAmpersand,
      "||" => Self::PipePipe,
      ".." => Self::PeriodPeriod,
      _ => unreachable!("{op}"),
    }
  }
}

impl std::fmt::Display for Op {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Equal => write!(f, "="),
      Self::Plus => write!(f, "+"),
      Self::Minus => write!(f, "-"),
      Self::Asterisk => write!(f, "*"),
      Self::Slash => write!(f, "/"),
      Self::Percent => write!(f, "%"),
      Self::Circumflex => write!(f, "^"),
      Self::Question => write!(f, "?"),
      Self::Exclamation => write!(f, "!"),
      Self::Ampersand => write!(f, "&"),
      Self::Pipe => write!(f, "|"),
      Self::Pound => write!(f, "#"),
      Self::LessThan => write!(f, "<"),
      Self::GreaterThan => write!(f, ">"),
      Self::EqualEqual => write!(f, "=="),
      Self::ExclamationEqual => write!(f, "!="),
      Self::PlusEqual => write!(f, "+="),
      Self::MinusEqual => write!(f, "-="),
      Self::AsteriskEqual => write!(f, "*="),
      Self::SlashEqual => write!(f, "/="),
      Self::PercentEqual => write!(f, "%="),
      Self::AmspersandEqual => write!(f, "&="),
      Self::PipeEqual => write!(f, "|="),
      Self::CircumflexEqual => write!(f, "^="),
      Self::LessThanLessThanEqual => write!(f, "<<="),
      Self::GreaterThanGreaterThanEqual => write!(f, ">>="),
      Self::ColonEqual => write!(f, ":="),
      Self::LessThanEqual => write!(f, "<="),
      Self::GreaterThanEqual => write!(f, ">="),
      Self::LessThanLessThan => write!(f, "<<"),
      Self::GreaterThanGreaterThan => write!(f, ">>"),
      Self::AmpersandAmpersand => write!(f, "&&"),
      Self::PipePipe => write!(f, "||"),
      Self::Period => write!(f, "."),
      Self::PeriodPeriod => write!(f, ".."),
    }
  }
}
