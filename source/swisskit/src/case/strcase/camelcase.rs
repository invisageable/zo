use inflector::cases::camelcase;

/// Checks if a text follows the camel case naming convention.
///
/// ```
/// use swisskit::case::strcase::camelcase;
///
/// let actual = "foo-Bar";
///
/// assert!(!camelcase::is_camel_case(&actual));
///
/// let actual = "fooBar";
///
/// assert!(camelcase::is_camel_case(&actual));
/// ```
#[inline]
pub fn is_camel_case(text: impl AsRef<str>) -> bool {
  camelcase::is_camel_case(text.as_ref())
}

/// Transforms to camel case naming convention from text.
#[inline]
pub fn to_camel_case(text: impl AsRef<str>) -> String {
  camelcase::to_camel_case(text.as_ref())
}
