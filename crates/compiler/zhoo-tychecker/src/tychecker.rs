use zo_core::Result;

#[derive(Debug)]
struct Tychecker {}

impl Tychecker {
  #[inline]
  fn new() -> Self {
    Self {}
  }

  fn check(&mut self) -> Result<()> {
    Ok(())
  }
}

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn check() -> Result<()> {
  println!("tycheck.");
  Tychecker::new().check()
}
