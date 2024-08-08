/// Prints to the io with a newline at the end.
pub fn println<V: std::fmt::Display>(fmt: V, value: V) {
  let _fmt = format!("{fmt}"); // todo — needs work.

  println!("{value}");
}
