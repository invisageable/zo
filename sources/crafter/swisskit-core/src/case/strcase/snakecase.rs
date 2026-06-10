use inflector::cases::screamingsnakecase;
use inflector::cases::snakecase;

/// Checks if a text follows the snake case naming convention.
///
/// A direct byte scan rather than inflector's
/// `text == to_snake_case(text)` round-trip: inflector treats every
/// digit as a word boundary, so the round-trip turns `r0` into `r_0`
/// and rejects it — but digits never need a separator in snake_case
/// (`r0`, `grid2` are idiomatic).
///
/// #### examples.
///
/// ```
/// use swisskit::case::strcase::snakecase;
///
/// assert!(snakecase::is_snake_case("foo_bar"));
/// assert!(snakecase::is_snake_case("r0"));
/// assert!(!snakecase::is_snake_case("foo-bar"));
/// assert!(!snakecase::is_snake_case("fooBar"));
/// ```
#[inline]
pub fn is_snake_case(text: impl AsRef<str>) -> bool {
  let text = text.as_ref();

  !text.is_empty()
    && text
      .bytes()
      .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
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
/// A direct byte scan for the same reason as [`is_snake_case`] —
/// inflector's digit word boundary rejects `MAX2`.
///
/// #### examples.
///
/// ```
/// use swisskit::case::strcase::snakecase;
///
/// assert!(snakecase::is_snake_screaming_case("BAR_FOO"));
/// assert!(snakecase::is_snake_screaming_case("MAX2"));
/// assert!(!snakecase::is_snake_screaming_case("bar-foo"));
/// assert!(!snakecase::is_snake_screaming_case("Bar_Foo"));
/// ```
#[inline]
pub fn is_snake_screaming_case(text: impl AsRef<str>) -> bool {
  let text = text.as_ref();

  !text.is_empty()
    && text
      .bytes()
      .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
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
