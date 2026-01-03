use zo_value::ValueStorage;

/// Represents a [`ConstProp`] instance.
pub struct ConstProp<'a> {
  /// The value storage.
  values: &'a ValueStorage,
}
impl<'a> ConstProp<'a> {
  /// Creates a new [`ConstProp`] instance.
  pub const fn new(values: &'a ValueStorage) -> Self {
    Self { values }
  }
}
