use std::io::Write;

/// ...
pub fn tprint(data: impl AsRef<str>, delay: std::time::Duration) {
  let stdout = std::io::stdout();
  let mut handle = stdout.lock();

  for ch in data.as_ref().chars() {
    write!(handle, "{ch}").unwrap();
    handle.flush().unwrap();
    std::thread::sleep(delay);
  }
}

/// ...
pub fn tprintln(data: impl AsRef<str>, delay: std::time::Duration) {
  tprint(data, delay);
  println!("\n");
}

#[cfg(feature = "future")]
pub mod future {
  use tokio::io::{AsyncWriteExt, time};
  use tokio::sync::watch;

  use std::io::Write;
  use std::time::Duration;

  /// Async typewriter printing with cancellation support.
  ///
  /// If the receiver sees a `true`, it will cancel printing.
  pub async fn tprint_async(
    text: impl AsRef<str>,
    delay: Duration,
    mut cancel_rx: watch::Receiver<bool>,
  ) -> io::Result<()> {
    let text = text.as_ref();
    let mut stdout = io::stdout();

    for ch in text.chars() {
      if *cancel_rx.borrow() {
        break;
      }

      stdout.write_all(ch.to_string().as_bytes()).await?;
      stdout.flush().await?;
      tokio::time::sleep(delay).await;
    }

    Ok(())
  }

  pub(crate) async fn etprint_async() {}
  pub(crate) async fn etprintln_async() {}
}
