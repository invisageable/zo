//! ...

use inflector::string::singularize;

#[inline]
pub fn to_singular(text: impl AsRef<str>) -> String {
  singularize::to_singular(text.as_ref())
}
