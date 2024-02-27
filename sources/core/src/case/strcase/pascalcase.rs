use inflector::cases::pascalcase;

#[inline]
pub fn is_pascal_case(text: impl AsRef<str>) -> bool {
  pascalcase::is_pascal_case(text.as_ref())
}

#[inline]
pub fn to_pascal_case(text: impl AsRef<str>) -> String {
  pascalcase::to_pascal_case(text.as_ref())
}

#[cfg(test)]
mod test {
  use super::is_pascal_case;
  use super::to_pascal_case;

  #[test]
  fn is_correct_from_pascal_case() {
    let actual = "foo-bar";

    assert_eq!(is_pascal_case(actual), false)
  }

  #[test]
  fn from_pascal_case() {
    let actual = "foo-bar";
    let expected = "FooBar";

    assert_eq!(to_pascal_case(actual), expected)
  }
}
