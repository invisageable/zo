//! ...

use inflector::cases::kebabcase;

#[inline]
pub fn is_kebab_case(text: impl AsRef<str>) -> bool {
  kebabcase::is_kebab_case(text.as_ref())
}

#[inline]
pub fn to_kebab_case(text: impl AsRef<str>) -> String {
  kebabcase::to_kebab_case(text.as_ref())
}

#[cfg(test)]
mod test {
  use super::is_kebab_case;
  use super::to_kebab_case;

  #[test]
  fn is_correct_from_kebab_case() {
    let actual = "fooBar";

    assert_eq!(is_kebab_case(actual), false)
  }

  #[test]
  fn from_kebab_case() {
    let actual = "fooBar";
    let expected = "foo-bar";

    assert_eq!(to_kebab_case(actual), expected)
  }
}
