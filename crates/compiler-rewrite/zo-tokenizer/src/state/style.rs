/// The representation of a style state.
#[derive(Debug)]
pub enum Style {
  /// A data state.
  Data,
  /// An identifier state.
  Ident,
  /// A delimiter state.
  Delim,
  /// A quote state.
  Quote,
}
