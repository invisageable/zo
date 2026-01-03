// adapted from https://github.com/gauravssnl/slowprint.
// s/o to @gauravssnl.

pub const DEFAULT_PRINT_DELAY: std::time::Duration =
  std::time::Duration::from_millis(100);

/// Print given text slowly with delay of given Duration.
///
/// #### examples.
///
/// ```ignore
/// use swisskit::io::typewriter::typewrite;
///
/// let delay = std::time::Duration::from_millis(200);
///
/// typewrite("Hello, Rust.", delay);
/// typewrite("Hello, Rust.".to_string(), delay);
/// typewrite(String::from("Hello, Rust."), delay);
/// ```
pub fn typewrite(
  data: impl AsRef<str> + std::marker::Send + 'static,
  delay: std::time::Duration,
) {
  let data = std::sync::Arc::new(std::sync::Mutex::new(data));
  let (tx, rx) = flume::unbounded();

  let producer = std::thread::spawn(move || {
    let data_guard = data.lock().unwrap();
    let data = data_guard.as_ref();

    for ch in data.chars() {
      tx.send(ch).ok();
    }
  });

  let consumer = std::thread::spawn(move || {
    use smol_str::ToSmolStr;
    use std::io::Write;

    let stdout = std::io::stdout();
    let mut handle = stdout.lock();

    for ch in rx {
      handle.write_all(ch.to_smolstr().as_bytes()).unwrap();
      handle.flush().unwrap();
      std::thread::sleep(delay);
    }
  });

  producer.join().unwrap();
  consumer.join().unwrap();
}

#[test]
fn test_typewrite() {
  let delay = std::time::Duration::from_millis(50);
  let now = std::time::Instant::now();

  typewrite("Hello, Rust.", delay);
  assert!(now.elapsed() >= delay);
}

/// Print given text slowly with delay of given Duration and add a newline at
/// the end of text.
///
/// #### examples.
///
/// ```ignore
/// use swisskit::io::typewriter::typewriteln;
///
/// let delay = std::time::Duration::from_millis(200);
///
/// typewriteln("Hello, Rust.", delay);
/// typewriteln("Hello, Rust.".to_string(), delay);
/// typewriteln(String::from("Hello, Rust."), delay);
/// ```
pub fn typewriteln(data: impl Into<String>, delay: std::time::Duration) {
  use std::ops::Add;

  typewrite(data.into().add("\n"), delay);
}

#[test]
fn test_typewriteln() {
  let delay = std::time::Duration::from_millis(100);
  let now = std::time::Instant::now();

  typewriteln("Hello, Rust.", delay);
  assert!(now.elapsed() >= delay);
}

/// Print given text slowly with delay of fixed Duration
/// `Duration::from_millis(1000)`.
///
/// #### examples.
///
/// ```ignore
/// use swisskit::io::typewriter::print;
///
/// print("Hello, Rust.");
/// print("Hello, Rust.".to_string());
/// print(String::from("Hello, Rust."));
/// ```
pub fn print(data: impl Into<String>) {
  typewrite(data.into(), DEFAULT_PRINT_DELAY);
}

#[test]
fn test_print() {
  let now = std::time::Instant::now();

  print("Hello, Rust.");
  assert!(now.elapsed() >= DEFAULT_PRINT_DELAY);
}

/// Print given text slowly with delay of fixed Duration
/// `Duration::from_millis(1000)` and add a newline at the end of the text.
///
/// #### examples.
///
/// ```ignore
/// use swisskit::io::typewriter::println;
///
/// println("Hello, Rust.");
/// println("Hello, Rust.".to_string());
/// println(String::from("Hello, Rust."));
/// ```
pub fn println(data: impl Into<String>) {
  typewriteln(data, DEFAULT_PRINT_DELAY);
}

#[test]
fn test_println() {
  let now = std::time::Instant::now();

  println("Hello, Rust.");
  assert!(now.elapsed() >= DEFAULT_PRINT_DELAY);
}
