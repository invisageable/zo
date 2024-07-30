use inflector::cases::pascalcase;

/// Checks if a text follows the pascal case naming convention.
///
/// #### examples.
///
/// ```
/// use swisskit::case::strcase::pascalcase;
///
/// assert!(pascalcase::is_pascal_case("FooBar"));
/// assert!(!pascalcase::is_pascal_case("foo-bar"));
/// ```
#[inline]
pub fn is_pascal_case(text: impl AsRef<str>) -> bool {
  pascalcase::is_pascal_case(text.as_ref())
}

/// Transforms a text into the pascal case naming convention.
///
/// #### examples.
///
/// ```
/// use swisskit::case::strcase::pascalcase;
///
/// assert_eq!(pascalcase::to_pascal_case("foo-bar"), String::from("FooBar"));
/// ```
#[inline]
pub fn to_pascal_case(text: impl AsRef<str>) -> String {
  pascalcase::to_pascal_case(text.as_ref())
}
