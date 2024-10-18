/// The representation of punctuation tokens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Punctuation {
  /// A equal punctuation — `=`.
  Equal,
  /// A equal punctuation — `+`.
  Plus,
  /// A equal punctuation — `-`.
  Minus,
  /// A equal punctuation — `*`.
  Asterisk,
  /// A equal punctuation — `/`.
  Slash,
  /// A equal punctuation — `%`.
  Percent,
  /// A equal punctuation — `^`.
  Circumflex,
  /// A equal punctuation — `?`.
  Question,
  /// A equal punctuation — `!`.
  Exclamation,
  /// A equal punctuation — `&`.
  Ampersand,
  /// A equal punctuation — `|`.
  Pipe,
  /// A equal punctuation — `#`.
  Pound,
  /// A equal punctuation — `<`.
  LessThan,
  /// A equal punctuation — `>`.
  GreaterThan,
  /// A equal punctuation — `.`.
  Dot,
  /// A comma punctustion — `,`.
  Comma,
  /// A comma punctustion — `:`.
  Colon,
  /// A comma punctustion — `;`.
  Semi,
  /// A equal punctuation — `==`.
  EqualEqual,
  /// A equal punctuation — `!=`.
  ExclamationEqual,
  /// A equal punctuation — `+=`.
  PlusEqual,
  /// A equal punctuation — `-=`.
  MinusEqual,
  /// A equal punctuation — `*=`.
  AsteriskEqual,
  /// A equal punctuation — `/=`.
  SlashEqual,
  /// A equal punctuation — `%=`.
  PercentEqual,
  /// A equal punctuation — `&=`.
  AmspersandEqual,
  /// A equal punctuation — `|=`.
  PipeEqual,
  /// A equal punctuation — `^=`.
  CircumflexEqual,
  /// A equal punctuation — `<<=`.
  LessThanLessThanEqual,
  /// A equal punctuation — `>>=`.
  GreaterThanGreaterThanEqual,
  /// A equal punctuation — `:=`.
  ColonEqual,
  /// A equal punctuation — `::=`.
  ColonColonEqual,
  /// A equal punctuation — `<=`.
  LessThanEqual,
  /// A equal punctuation — `>=`.
  GreaterThanEqual,
  /// A equal punctuation — `<<`.
  LessThanLessThan,
  /// A equal punctuation — `>>`.
  GreaterThanGreaterThan,
  /// A equal punctuation — `&&`.
  AmpersandAmpersand,
  /// A equal punctuation — `||`.
  PipePipe,
  /// A equal punctuation — `..`.
  DotDot,
  /// A colon colon punctuation — `::`.
  ColonColon,
  /// A colon colon punctuation — `->`.
  MinusGreaterThan,
  /// A colon colon punctuation — `=>`.
  EqualGreaterThan,
}

impl From<&str> for Punctuation {
  fn from(op: &str) -> Self {
    match op {
      "=" => Self::Equal,
      "+" => Self::Plus,
      "-" => Self::Minus,
      "*" => Self::Asterisk,
      "/" => Self::Slash,
      "%" => Self::Percent,
      "^" => Self::Circumflex,
      "?" => Self::Question,
      "!" => Self::Exclamation,
      "&" => Self::Ampersand,
      "|" => Self::Pipe,
      "#" => Self::Pound,
      "<" => Self::LessThan,
      ">" => Self::GreaterThan,
      "." => Self::Dot,
      "," => Self::Comma,
      ":" => Self::Colon,
      ";" => Self::Semi,
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
      "::=" => Self::ColonColonEqual,
      "<=" => Self::LessThanEqual,
      ">=" => Self::GreaterThanEqual,
      "<<" => Self::LessThanLessThan,
      ">>" => Self::GreaterThanGreaterThan,
      "&&" => Self::AmpersandAmpersand,
      "||" => Self::PipePipe,
      ".." => Self::DotDot,
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
      Self::Dot => write!(f, "."),
      Self::Comma => write!(f, ","),
      Self::Colon => write!(f, ":"),
      Self::Semi => write!(f, ";"),
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
      Self::ColonColonEqual => write!(f, "::="),
      Self::LessThanEqual => write!(f, "<="),
      Self::GreaterThanEqual => write!(f, ">="),
      Self::LessThanLessThan => write!(f, "<<"),
      Self::GreaterThanGreaterThan => write!(f, ">>"),
      Self::AmpersandAmpersand => write!(f, "&&"),
      Self::PipePipe => write!(f, "||"),
      Self::DotDot => write!(f, ".."),
      Self::ColonColon => write!(f, "::"),
      Self::MinusGreaterThan => write!(f, "->"),
      Self::EqualGreaterThan => write!(f, "=>"),
    }
  }
}
