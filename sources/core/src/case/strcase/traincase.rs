//! ...

use inflector::cases::traincase;

#[inline]
pub fn is_train_case(text: impl AsRef<str>) -> bool {
  traincase::is_train_case(text.as_ref())
}

#[inline]
pub fn to_train_case(text: impl AsRef<str>) -> String {
  traincase::to_train_case(text.as_ref())
}

#[cfg(test)]
mod test {
  use super::is_train_case;
  use super::to_train_case;

  #[test]
  fn is_correct_from_train_case() {
    let actual = "foo-bar";

    assert_eq!(is_train_case(actual), false)
  }

  #[test]
  fn from_train_case() {
    let actual = "foo-bar";
    let expected = "Foo-Bar";

    assert_eq!(to_train_case(actual), expected)
  }
}
