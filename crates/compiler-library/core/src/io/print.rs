use swisskit::fmt;

/// Prints to the io.
pub fn print<V: std::fmt::Display + std::convert::AsRef<str>>(
  fmt: V,
  value: V,
) {
  match fmt::format(fmt, &[value]) {
    Ok(fmt) => println!("{fmt}"),
    Err(err) => panic!("{err}"), // todo(ivs) — shoudl raise an internal error.
  }
}

/// Prints to the io with a newline at the end.
pub fn println<V: std::fmt::Display + std::convert::AsRef<str>>(
  fmt: V,
  value: V,
) {
  match fmt::format(fmt, &[value]) {
    Ok(fmt) => println!("{fmt}"),
    Err(err) => panic!("{err}"), // todo(ivs) — shoudl raise an internal error.
  }
}
