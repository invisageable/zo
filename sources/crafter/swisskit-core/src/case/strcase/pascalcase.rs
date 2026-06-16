use inflector::cases::pascalcase;

/// Checks if a text follows the pascal case naming convention.
///
/// A name is PascalCase when it is non-empty, starts with an
/// uppercase letter, and contains only alphanumeric characters
/// (no separators). Consecutive capitals are allowed, so
/// acronym-style names like `CStr`, `CBytes`, and `HTTPServer`
/// are valid — a round-trip check against `to_pascal_case`
/// mangles the acronym (`CStr` → `Cstr`) and wrongly rejects
/// them.
///
/// #### examples.
///
/// ```
/// use swisskit::case::strcase::pascalcase;
///
/// assert!(pascalcase::is_pascal_case("FooBar"));
/// assert!(pascalcase::is_pascal_case("CStr"));
/// assert!(pascalcase::is_pascal_case("CBytes"));
/// assert!(pascalcase::is_pascal_case("HTTPServer"));
/// assert!(pascalcase::is_pascal_case("Vec2"));
/// assert!(!pascalcase::is_pascal_case("fooBar"));
/// assert!(!pascalcase::is_pascal_case("foo-bar"));
/// assert!(!pascalcase::is_pascal_case("foo_bar"));
/// ```
#[inline]
pub fn is_pascal_case(text: impl AsRef<str>) -> bool {
  let mut chars = text.as_ref().chars();

  match chars.next() {
    Some(first) if first.is_uppercase() => chars.all(|c| c.is_alphanumeric()),
    _ => false,
  }
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
