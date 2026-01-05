use inflector::cases::camelcase;

/// Checks if a text follows the camel case naming convention.
///
/// #### examples.
///
/// ```
/// use swisskit::case::strcase::camelcase;
///
/// assert!(camelcase::is_camel_case("fooBar"));
/// assert!(!camelcase::is_camel_case("foo-Bar"));
/// ```
#[inline]
pub fn is_camel_case(text: impl AsRef<str>) -> bool {
  camelcase::is_camel_case(text.as_ref())
}

/// Transforms to camel case naming convention from text.
///
/// #### examples.
///
/// ```
/// use swisskit::case::strcase::camelcase;
///
/// assert_eq!(camelcase::to_camel_case("bar_foo"), String::from("barFoo"));
/// ```
#[inline]
pub fn to_camel_case(text: impl AsRef<str>) -> String {
  camelcase::to_camel_case(text.as_ref())
}
