use inflector::cases::camelcase;

#[inline]
pub fn is_camel_case(text: impl AsRef<str>) -> bool {
  camelcase::is_camel_case(text.as_ref())
}

#[inline]
pub fn to_camel_case(text: impl AsRef<str>) -> String {
  camelcase::to_camel_case(text.as_ref())
}

#[cfg(test)]
mod test {
  use super::is_camel_case;
  use super::to_camel_case;

  #[test]
  fn is_correct_from_camel_case() {
    let actual = "foo-Bar";

    assert_eq!(is_camel_case(&actual), false)
  }

  #[test]
  fn from_camel_case() {
    let actual = "fooBar";
    let expected = "fooBar";

    assert_eq!(to_camel_case(&actual), expected)
  }
}
