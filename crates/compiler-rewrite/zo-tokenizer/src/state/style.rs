/// The representation of a style state.
#[derive(Debug)]
pub enum Style {
  /// A data state.
  Data,
  /// A delimiter state.
  Delim,
  /// A group state.
  Group,
  /// An identifier state.
  Ident,
  /// A quote state.
  Quote,
}
