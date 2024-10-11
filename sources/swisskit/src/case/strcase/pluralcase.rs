use inflector::string::pluralize;

/// Transforms a text to pluralized String.
///
/// #### examples.
///
/// ```
/// use swisskit::case::strcase::pluralcase;
///
/// assert_eq!(pluralcase::to_plural("crate"), String::from("crates"));
/// ```
#[inline]
pub fn to_plural(text: impl AsRef<str>) -> String {
  pluralize::to_plural(text.as_ref())
}
