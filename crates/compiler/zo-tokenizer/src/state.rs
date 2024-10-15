/// The representation of a tokenizer's state.
#[derive(Clone, Copy, Debug)]
pub enum TokenizerState {
  // --- MODE:PROGRAM:START. ---
  ///
  /// An initial state for zo programming language.
  Program,
  /// An unknown state.
  Unknown,
  /// A space state.
  Space,
  /// A comment line state.
  CommentLine,
  /// A comment line doc state.
  CommentLineDoc,
  /// A zero state.
  Num(Num),
  /// A punctuation state.
  Punctuation,
  /// A group state.
  Group,
  /// A quote state.
  Quote,
  /// A identifier state.
  Ident,
  /// A byte state.
  Byte,
  /// A character state.
  Char,
  /// A string state.
  Str,
  /// A escape state.
  Escape(In),
  /// A unicode state.
  Unicode(In),

  // --- STYLE. ---
  ZssStart,
  Zss,
  ZssEnd,
  // --- STYLE. ---

  // --- MODE:TEMPLATE:START. ---
  ///
  /// An initial state for zsx templating langage.
  ZsxData,
  /// A zsx's raws text.
  ZsxRawText,
  /// A zsx's text state.
  ZsxCharacter,
  /// A zsx's tag state.
  ZsxTag,
  /// A zsx's tag open state.
  ZsxTagOpen,
  /// A zsx's tag open end state.
  ZsxTagOpenEnd,
  /// A zsx's self closing tag state.
  ZsxTagSelfClosingStart,
  /// A zsx's tag name state.
  ZsxTagName,
  /// A zsx's before attribute name state.
  ZsxBeforeAttributeName,
  /// A zsx's attribute name state.
  ZsxAttributeName,
  /// A zsx's after attribute name state.
  ZsxAfterAttributeName,
  /// A zsx's before attribute value state.
  ZsxBeforeAttributeValue,
  /// A zsx's attribute name state.
  ZsxAttributeValue(Quoted),
  /// A zsx's after attribute name state.
  ZsxAfterAttributeValue,
  /// A zsx's comment start state.
  ZsxCommentStart,
  /// A zsx's comment start dash state.
  ZsxCommentStartDash,
  /// A zsx's comment state.
  ZsxComment,
  /// A zsx's comment end dash state.
  ZsxCommentEndDash,
  /// A zsx's comment end state.
  ZsxCommentEnd,
  /// A zsx's comment end bang state.
  ZsxCommentEndBang,
}

/// The representation of a number state.
#[derive(Clone, Copy, Debug)]
pub enum Comment {
  /// A line comment state.
  Line,
  /// An line doc comment state.
  LineDoc,
}

/// The representation of a number state.
#[derive(Clone, Copy, Debug)]
pub enum Num {
  /// A zero number state.
  Zero,
  /// An integer number state.
  Int,
  /// A hexadecimal state.
  Hex,
  /// A octal state.
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
#[derive(Clone, Copy, Debug)]
pub enum Expo {
  /// A 'E' or 'e' character state.
  E,
  /// A  `+` or `-` character state.
  Sign,
  /// A exponent digits.
  Digits,
}

/// The representation of a tokenizer's escaped state.
#[derive(Clone, Copy, Debug)]
pub enum In {
  /// A in byte escaped state.
  Byte,
  /// A in char escaped state.
  Char,
  /// A in string escaped state.
  Str,
}

/// The representation of a tokenizer's quoted state.
#[derive(Clone, Copy, Debug)]
pub enum Quoted {
  /// A unquoted state.
  No,
  /// A double quoted state.
  Double,
  /// A single quoted state.
  Single,
  /// A curly quoted state.
  Brace,
}
