/// The representation of a program state.
#[derive(Debug)]
pub enum Program {
  /// A data state.
  Data,
  /// A comment line state.
  CommentLine,
  /// A comment line doc state.
  CommentLineDoc,
  /// A number state.
  Num(Num),
  /// A group state.
  Group,
  /// A punctuation state.
  Punctuation,
  /// An identifier state.
  Ident,
  /// A quote state.
  Quote,
  /// A character state.
  Char,
  /// A string state.
  Str,
  /// An invalid number state.
  InvalidNumber,
  /// An unknown state.
  Unknown,
}

/// The representation of a number state.
#[derive(Debug)]
pub enum Num {
  /// A zero number state.
  Zero,
  /// An integer number state.
  Int,
  /// A hexadecimal state.
  Hex,
  /// An octal state.
  Oct,
  /// A binary state.
  Bin,
  /// A decimal point encountered state.
  DecPoint,
  /// A floating-point number state.
  Float,
  /// An exponent state.
  Expo(Expo),
  /// An error state.
  Error,
}

/// The representation of an expo number state.
#[derive(Debug)]
pub enum Expo {
  /// A `E` or `e` character state.
  E,
  /// A `+` or `-` character state.
  Sign,
  /// An exponent digits.
  Digits,
}
