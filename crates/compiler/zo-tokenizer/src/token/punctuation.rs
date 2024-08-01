/// The representation of punctuation tokens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Punctuation {
  /// The `=` punctuation.
  Equal,
  /// The `+` punctuation.
  Plus,
  /// The `-` punctuation.
  Minus,
  /// The `*` punctuation.
  Asterisk,
  /// The `/` punctuation.
  Slash,
  /// The `%` punctuation.
  Percent,
  /// The `^` punctuation.
  Circumflex,
  /// The `?` punctuation.
  Question,
  /// The `!` punctuation.
  Exclamation,
  /// The `&` punctuation.
  Ampersand,
  /// The `|` punctuation.
  Pipe,
  /// The `#` punctuation.
  Pound,
  /// The `<` punctuation.
  LessThan,
  /// The `>` punctuation.
  GreaterThan,
  /// The `.` punctuation.
  Period,
  /// `,` punctuation.
  Comma,
  /// `:` punctuation.
  Colon,
  /// `;` punctuation.
  Semicolon,
  /// The `==` punctuation.
  EqualEqual,
  /// The `!=` punctuation.
  ExclamationEqual,
  /// The `+=` punctuation.
  PlusEqual,
  /// The `-=` punctuation.
  MinusEqual,
  /// The `*=` punctuation.
  AsteriskEqual,
  /// The `/=` punctuation.
  SlashEqual,
  /// The `%=` punctuation.
  PercentEqual,
  /// The `&=` punctuation.
  AmspersandEqual,
  /// The `|=` punctuation.
  PipeEqual,
  /// The `^=` punctuation.
  CircumflexEqual,
  /// The `<<=` punctuation.
  LessThanLessThanEqual,
  /// The `>>=` punctuation.
  GreaterThanGreaterThanEqual,
  /// The `:=` punctuation.
  ColonEqual,
  /// The `<=` punctuation.
  LessThanEqual,
  /// The `>=` punctuation.
  GreaterThanEqual,
  /// The `<<` punctuation.
  LessThanLessThan,
  /// The `>>` punctuation.
  GreaterThanGreaterThan,
  /// The `&&` punctuation.
  AmpersandAmpersand,
  /// The `||` punctuation.
  PipePipe,
  /// The `..` punctuation.
  PeriodPeriod,
  /// `::` punctuation.
  ColonColon,
  /// `->` punctuation.
  MinusGreaterThan,
  /// `=>` punctuation.
  EqualGreaterThan,
}

impl From<char> for Punctuation {
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
      b',' => Self::Comma,
      b':' => Self::Colon,
      b';' => Self::Semicolon,
      _ => unreachable!("{op}"),
    }
  }
}

impl From<&str> for Punctuation {
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
      "::" => Self::ColonColon,
      "->" => Self::MinusGreaterThan,
      "=>" => Self::EqualGreaterThan,
      _ => unreachable!("{op}"),
    }
  }
}

impl std::fmt::Display for Punctuation {
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
      Self::Period => write!(f, "."),
      Self::Comma => write!(f, ","),
      Self::Colon => write!(f, ":"),
      Self::Semicolon => write!(f, ";"),
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
      Self::PeriodPeriod => write!(f, ".."),
      Self::ColonColon => write!(f, "::"),
      Self::MinusGreaterThan => write!(f, "->"),
      Self::EqualGreaterThan => write!(f, "=>"),
    }
  }
}
