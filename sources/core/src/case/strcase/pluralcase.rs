use inflector::string::pluralize;

#[inline]
pub fn to_plural(text: impl AsRef<str>) -> String {
  pluralize::to_plural(text.as_ref())
}
