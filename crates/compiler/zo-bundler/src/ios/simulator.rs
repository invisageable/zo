//! Driving the iOS Simulator from the compiler: boot a device,
//! install the freshly built `.app`, and launch it.
//!
//! `zo run --target ios` resolves a device through
//! [`crate::ios::device`], builds a [`Simulator`] for it, and calls
//! [`Simulator::launch`] — so the developer never reaches for
//! `xcrun simctl` by hand.

use std::io;
use std::path::Path;
use std::process::Command;

/// One Simulator device, addressed by a `simctl` specifier — a UDID
/// (preferred, unambiguous) or a device name.
pub struct Simulator {
  /// The `simctl` device specifier.
  device: String,
}

impl Simulator {
  /// A simulator handle for `device` (a `simctl` device specifier).
  pub fn new(device: &str) -> Self {
    Self {
      device: device.to_string(),
    }
  }

  /// Boot the device, bring the Simulator window forward, install
  /// `app`, and launch it by `bundle_id`. Idempotent on an
  /// already-booted device.
  pub fn launch(&self, app: &Path, bundle_id: &str) -> io::Result<()> {
    self.boot()?;
    self.reveal()?;
    self.install(app)?;
    self.start(bundle_id)?;

    Ok(())
  }

  /// Boot the device directly. We avoid `simctl bootstatus -b`, which
  /// wedges on a cold CoreSimulator. An already-booted device reports
  /// a non-zero "current state: Booted" — success here, not a failure.
  fn boot(&self) -> io::Result<()> {
    let output = Command::new("xcrun")
      .args(["simctl", "boot", &self.device])
      .output()?;

    if output.status.success() {
      return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);

    if stderr.contains("Booted") {
      return Ok(());
    }

    Err(io::Error::other(format!(
      "could not boot simulator '{}': {}",
      self.device,
      stderr.trim(),
    )))
  }

  /// Bring the Simulator app to the foreground so the launched app is
  /// visible. A no-op when it is already open.
  fn reveal(&self) -> io::Result<()> {
    run("open", &["-a", "Simulator"])
  }

  /// Install `app` onto the device, replacing any previous copy.
  fn install(&self, app: &Path) -> io::Result<()> {
    let app = app.to_str().ok_or_else(|| {
      io::Error::other(format!("non-UTF-8 app path: {}", app.display()))
    })?;

    run("xcrun", &["simctl", "install", &self.device, app])
  }

  /// Launch the installed app by its bundle identifier.
  fn start(&self, bundle_id: &str) -> io::Result<()> {
    run("xcrun", &["simctl", "launch", &self.device, bundle_id])
  }
}

/// Run `program args...`, mapping a non-zero exit into an error that
/// carries the captured stderr.
fn run(program: &str, args: &[&str]) -> io::Result<()> {
  let output = Command::new(program).args(args).output()?;

  if output.status.success() {
    return Ok(());
  }

  let stderr = String::from_utf8_lossy(&output.stderr);

  Err(io::Error::other(format!(
    "{program} {}: {}",
    args.join(" "),
    stderr.trim(),
  )))
}
