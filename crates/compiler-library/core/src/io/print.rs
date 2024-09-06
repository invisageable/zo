use swisskit::fmt;

/// Prints to the io.
pub fn show<V: std::fmt::Display + std::convert::AsRef<str>>(
  source: V,
  value: V,
) {
  match fmt::format(source, &[value]) {
    Ok(fmt) => print!("{fmt}"),
    Err(err) => panic!("{err}"), // todo(ivs) — should raise an internal error.
  }
}

/// Prints to the io with a newline at the end.
pub fn showln<V: std::fmt::Display + std::convert::AsRef<str>>(
  source: V,
  value: V,
) {
  match fmt::format(source, &[value]) {
    Ok(fmt) => println!("{fmt}"),
    Err(err) => panic!("{err}"), // todo(ivs) — should raise an internal error.
  }
}
