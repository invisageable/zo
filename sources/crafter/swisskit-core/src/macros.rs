#[macro_export]
macro_rules! tprint (
  ($data:expr) => {
    $crate::io::tprint(
      $data,
      $crate::io::DEFAULT_PRINT_DELAY,
    );
  };
  ($data:expr, $delay:expr) => {
    $crate::io::tprint($data, $delay);
  };
);

#[macro_export]
macro_rules! tprintln (
  ($data:expr) => {
    $crate::io::tprintln(
      $data,
      $crate::io::DEFAULT_PRINT_DELAY,
    );
  };
  ($data:expr, $delay:expr $(,)?) => {
    $crate::io::tprintln($data, $delay);
  };
);
