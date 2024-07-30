use inflector::cases::kebabcase;

/// Checks if a text follows the kebab case naming convention.
///
/// #### examples.
///
/// ```
/// use swisskit::case::strcase::kebabcase;
///
/// assert!(kebabcase::is_kebab_case("foo-bar"));
/// assert!(!kebabcase::is_kebab_case("foo-Bar"));
/// ```
#[inline]
pub fn is_kebab_case(text: impl AsRef<str>) -> bool {
  kebabcase::is_kebab_case(text.as_ref())
}

/// Checks if a text follows the kebab case naming convention.
///
/// #### examples.
///
/// ```
/// use swisskit::case::strcase::kebabcase;
///
/// assert_eq!(kebabcase::to_kebab_case("barFoo"), String::from("bar-foo"));
/// ```
#[inline]
pub fn to_kebab_case(text: impl AsRef<str>) -> String {
  kebabcase::to_kebab_case(text.as_ref())
}
