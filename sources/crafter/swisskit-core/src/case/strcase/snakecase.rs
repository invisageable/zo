use inflector::cases::screamingsnakecase;
use inflector::cases::snakecase;

/// Checks if a text follows the snake case naming convention.
///
/// #### examples.
///
/// ```
/// use swisskit::case::strcase::snakecase;
///
/// assert!(snakecase::is_snake_case("foo_bar"));
/// assert!(!snakecase::is_snake_case("foo-bar"));
/// ```
#[inline]
pub fn is_snake_case(text: impl AsRef<str>) -> bool {
  snakecase::is_snake_case(text.as_ref())
}

/// Checks if a text follows the snake case naming convention.
///
/// #### examples.
///
/// ```
/// use swisskit::case::strcase::snakecase;
///
/// assert_eq!(snakecase::to_snake_case( "FooBar"), String::from("foo_bar"));
/// ```
#[inline]
pub fn to_snake_case(text: impl AsRef<str>) -> String {
  snakecase::to_snake_case(text.as_ref())
}

/// Checks if a text follows the snake screaming case naming convention.
///
/// #### examples.
///
/// ```
/// use swisskit::case::strcase::snakecase;
///
/// assert!(snakecase::is_snake_screaming_case("BAR_FOO"));
/// assert!(!snakecase::is_snake_screaming_case("bar-foo"));
/// ```
#[inline]
pub fn is_snake_screaming_case(text: impl AsRef<str>) -> bool {
  screamingsnakecase::is_screaming_snake_case(text.as_ref())
}

/// Checks if a text follows the snake screaming case naming convention.
///
/// #### examples.
///
/// ```
/// use swisskit::case::strcase::snakecase;
///
/// assert_eq!(snakecase::to_snake_screaming_case( "bar-foo"), String::from("BAR_FOO"));
/// ```
#[inline]
pub fn to_snake_screaming_case(text: impl AsRef<str>) -> String {
  screamingsnakecase::to_screaming_snake_case(text.as_ref())
}
