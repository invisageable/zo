pub const DEFAULT_PRINT_DELAY: std::time::Duration =
  std::time::Duration::from_millis(100);

/// Print given text slowly with delay of given Duration.
///
/// #### examples.
///
/// ```ignore
/// use swisskit::io::tprint;
///
/// let delay = std::time::Duration::from_millis(200);
///
/// tprint("Hello, Rust.", delay);
/// tprint("Hello, Rust.".to_string(), delay);
/// tprint(String::from("Hello, Rust."), delay);
/// ```
pub fn tprint(
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
fn test_tprint() {
  let delay = std::time::Duration::from_millis(50);
  let now = std::time::Instant::now();

  tprint("Hello, Rust.", delay);
  assert!(now.elapsed() >= delay);
}

/// Print given text slowly with delay of given Duration and add a newline at
/// the end of text.
///
/// #### examples.
///
/// ```ignore
/// use swisskit::io::tprintln;
///
/// let delay = std::time::Duration::from_millis(200);
///
/// tprintln("Hello, Rust.", delay);
/// tprintln("Hello, Rust.".to_string(), delay);
/// tprintln(String::from("Hello, Rust."), delay);
/// ```
pub fn tprintln(data: impl Into<String>, delay: std::time::Duration) {
  use std::ops::Add;

  tprint(data.into().add("\n"), delay);
}

#[test]
fn test_tprintln() {
  let delay = std::time::Duration::from_millis(100);
  let now = std::time::Instant::now();

  tprintln("Hello, Rust.", delay);
  assert!(now.elapsed() >= delay);
}

/// Print given text slowly with delay of fixed Duration
/// `Duration::from_millis(1000)`.
///
/// #### examples.
///
/// ```ignore
/// use swisskit::io::print;
///
/// print("Hello, Rust.");
/// print("Hello, Rust.".to_string());
/// print(String::from("Hello, Rust."));
/// ```
pub fn print(data: impl Into<String>) {
  tprint(data.into(), DEFAULT_PRINT_DELAY);
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
/// use swisskit::io::println;
///
/// println("Hello, Rust.");
/// println("Hello, Rust.".to_string());
/// println(String::from("Hello, Rust."));
/// ```
pub fn println(data: impl Into<String>) {
  tprintln(data, DEFAULT_PRINT_DELAY);
}

#[test]
fn test_println() {
  let now = std::time::Instant::now();

  println("Hello, Rust.");
  assert!(now.elapsed() >= DEFAULT_PRINT_DELAY);
}
