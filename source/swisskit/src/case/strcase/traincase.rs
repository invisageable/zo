use inflector::cases::traincase;

/// Checks if a text follows the snake case naming convention.
///
/// #### examples.
///
/// ```
/// use swisskit::case::strcase::traincase;
///
/// assert!(traincase::is_train_case("Foo-Bar"));
/// assert!(!traincase::is_train_case("foo-bar"));
/// assert!(!traincase::is_train_case("foobar"));
/// ```
#[inline]
pub fn is_train_case(text: impl AsRef<str>) -> bool {
  traincase::is_train_case(text.as_ref())
}

/// Checks if a text follows the snake case naming convention.
///
/// #### examples.
///
/// ```
/// use swisskit::case::strcase::traincase;
///
/// assert_eq!(traincase::to_train_case( "foo-bar"), String::from("Foo-Bar"))
/// ```
#[inline]
pub fn to_train_case(text: impl AsRef<str>) -> String {
  traincase::to_train_case(text.as_ref())
}
