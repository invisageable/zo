#[macro_export]
macro_rules! tprint (
  ($data:expr) => {
    $crate::io::typewriter::tprint(
      $data,
      $crate::io::typewriter::DEFAULT_PRINT_DELAY,
    );
  };
  ($data:expr, $delay:expr) => {
    $crate::io::typewriter::tprint($data, $delay);
  };
);

#[macro_export]
macro_rules! tprintln (
  ($data:expr) => {
    $crate::io::typewriter::tprintln(
      $data,
      $crate::io::typewriter::DEFAULT_PRINT_DELAY,
    );
  };
  ($data:expr, $delay:expr $(,)?) => {
    $crate::io::typewriter::tprintln($data, $delay);
  };
);
