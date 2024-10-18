/// The representation of a template state.
#[derive(Debug)]
pub enum Template {
  /// A data state.
  Data,
  /// A character state.
  Character,
  /// A raw text state.
  RawText,
  /// A tag open state.
  TagOpen,
  /// A tag open end state.
  TagOpenEnd,
  /// A self closing tag state.
  TagSelfClosingStart,
  /// A tag name state.
  TagName,
  /// A before attribute name state.
  BeforeAttributeName,
  /// An attribute name state.
  AttributeName,
  /// An after attribute name state.
  AfterAttributeName,
  /// A before attribute value state.
  BeforeAttributeValue,
  /// An attribute name state.
  AttributeValue(Quoted),
  /// An after attribute name state.
  AfterAttributeValue,
}

/// The representation of a tokenizer quoted state.
#[derive(Debug)]
pub enum Quoted {
  /// An unquoted state.
  No,
  /// A double quoted state.
  Double,
  /// A single quoted state.
  Single,
  /// A curly quoted state.
  Brace,
}
