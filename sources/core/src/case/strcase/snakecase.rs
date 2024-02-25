use inflector::cases::screamingsnakecase;
use inflector::cases::snakecase;

#[inline]
pub fn is_snake_case(text: impl AsRef<str>) -> bool {
  snakecase::is_snake_case(text.as_ref())
}

#[inline]
pub fn to_snake_case(text: impl AsRef<str>) -> String {
  snakecase::to_snake_case(text.as_ref())
}

#[inline]
pub fn is_snake_screaming_case(text: impl AsRef<str>) -> bool {
  screamingsnakecase::is_screaming_snake_case(text.as_ref())
}

#[inline]
pub fn to_snake_screaming_case(text: impl AsRef<str>) -> String {
  screamingsnakecase::to_screaming_snake_case(text.as_ref())
}
