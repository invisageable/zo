#[macro_export]
macro_rules! typewrite (
  ($data:expr) => {
    $crate::io::typewriter::typewrite(
      $data,
      $crate::io::typewriter::DEFAULT_PRINT_DELAY,
    );
  };
  ($data:expr, $delay:expr) => {
    $crate::io::typewriter::typewrite($data, $delay);
  };
);

#[macro_export]
macro_rules! typewriteln (
  ($data:expr) => {
    $crate::io::typewriter::typewriteln(
      $data,
      $crate::io::typewriter::DEFAULT_PRINT_DELAY,
    );
  };
  ($data:expr, $delay:expr $(,)?) => {
    $crate::io::typewriter::typewriteln($data, $delay);
  };
);
