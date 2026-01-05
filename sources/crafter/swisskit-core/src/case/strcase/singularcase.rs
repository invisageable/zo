use inflector::string::singularize;

/// Transforms a text to singularized String.
///
/// #### examples.
///
/// ```
/// use swisskit::case::strcase::singularcase;
///
/// assert_eq!(singularcase::to_singular("crates"), String::from("crate"));
/// ```
#[inline]
pub fn to_singular(text: impl AsRef<str>) -> String {
  singularize::to_singular(text.as_ref())
}
