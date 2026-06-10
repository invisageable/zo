//! Simulator device discovery and resolution.
//!
//! `zo run --target ios|watchos` accepts any device the machine
//! actually has: [`detect`] asks `simctl` for the available devices
//! and [`resolve`] picks one from the `--device` flag — or
//! auto-selects when the flag is omitted. Resolution also guards the
//! runtime contract through [`Artifact`]: an iPhone-family app runs
//! on iOS and visionOS simulators, a watch app only on watchOS
//! simulators.

use std::fmt;
use std::io;
use std::process::Command;

/// The operating system a simulator runtime boots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
  /// iPhone and iPad simulators.
  Ios,
  /// Apple Vision Pro simulators.
  VisionOs,
  /// Apple TV simulators.
  TvOs,
  /// Apple Watch simulators.
  WatchOs,
}

impl Os {
  /// The runtime named by a `simctl list` section header, e.g. `iOS`.
  fn from_runtime_name(name: &str) -> Option<Self> {
    match name {
      "iOS" => Some(Self::Ios),
      "visionOS" => Some(Self::VisionOs),
      "tvOS" => Some(Self::TvOs),
      "watchOS" => Some(Self::WatchOs),
      _ => None,
    }
  }
}

/// The app artifact a device must be able to run — decides which
/// simulator runtimes qualify during resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Artifact {
  /// An iPhone-family app (`--target ios`). iOS simulators run it,
  /// and so do visionOS simulators through their iOS
  /// app-compatibility layer; watchOS and tvOS only load binaries
  /// built against their own platform SDK.
  Ios,
  /// A watch app (`--target watchos`) — only watchOS simulators.
  Watchos,
}

impl Artifact {
  /// Whether a device running `os` installs and launches this app.
  fn supports(self, os: Os) -> bool {
    match self {
      Self::Ios => matches!(os, Os::Ios | Os::VisionOs),
      Self::Watchos => matches!(os, Os::WatchOs),
    }
  }

  /// The device families named in resolution errors, e.g.
  /// `an iOS or visionOS device`.
  fn device_label(self) -> &'static str {
    match self {
      Self::Ios => "an iOS or visionOS device",
      Self::Watchos => "a watchOS device",
    }
  }
}

impl fmt::Display for Os {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(match self {
      Self::Ios => "iOS",
      Self::VisionOs => "visionOS",
      Self::TvOs => "tvOS",
      Self::WatchOs => "watchOS",
    })
  }
}

/// One available simulator device, as reported by `simctl`.
#[derive(Debug)]
pub struct Device {
  /// The user-facing device name, e.g. `Apple Vision Pro`.
  pub name: String,
  /// The device UDID — the stable `simctl` address for boot/install.
  pub udid: String,
  /// The runtime the device boots.
  pub os: Os,
  /// The runtime version, e.g. `26.5`.
  pub os_version: String,
  /// Whether the device is currently booted.
  pub booted: bool,
}

/// Query `simctl` for the available simulator devices.
pub fn detect() -> io::Result<Vec<Device>> {
  let output = Command::new("xcrun")
    .args(["simctl", "list", "devices", "available"])
    .output()?;

  if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr);

    return Err(io::Error::other(format!(
      "could not list simulator devices: {}",
      stderr.trim(),
    )));
  }

  Ok(parse(&String::from_utf8_lossy(&output.stdout)))
}

/// Parse the plain-text `simctl list devices available` listing.
///
/// @note — sections open with `-- <runtime> <version> --` and each
/// device line reads `<name> (<udid>) (<state>)`. Names themselves
/// contain parentheses (`iPhone SE (3rd generation)`), so the UDID and
/// state are taken from the right.
pub(crate) fn parse(listing: &str) -> Vec<Device> {
  let mut devices = Vec::new();
  let mut runtime: Option<(Os, String)> = None;

  for line in listing.lines() {
    let trimmed = line.trim();

    if let Some(header) = trimmed
      .strip_prefix("--")
      .and_then(|rest| rest.strip_suffix("--"))
    {
      runtime = header.trim().split_once(' ').and_then(|(name, version)| {
        Os::from_runtime_name(name).map(|os| (os, version.trim().to_string()))
      });

      continue;
    }

    let Some((os, os_version)) = &runtime else {
      continue;
    };

    if let Some(device) = parse_device_line(trimmed, *os, os_version) {
      devices.push(device);
    }
  }

  devices
}

/// Parse one `<name> (<udid>) (<state>)` device line.
fn parse_device_line(line: &str, os: Os, os_version: &str) -> Option<Device> {
  let (rest, state) = split_trailing_group(line)?;
  let (name, udid) = split_trailing_group(rest)?;

  if name.is_empty() || udid.is_empty() {
    return None;
  }

  Some(Device {
    name: name.to_string(),
    udid: udid.to_string(),
    os,
    os_version: os_version.to_string(),
    booted: state.starts_with("Booted"),
  })
}

/// Split `prefix (group)` into `(prefix, group)` at the last
/// parenthesized group.
fn split_trailing_group(text: &str) -> Option<(&str, &str)> {
  let text = text.trim_end();
  let inner = text.strip_suffix(')')?;
  let open = inner.rfind('(')?;

  Some((inner[..open].trim_end(), &inner[open + 1..]))
}

/// Pick the device `requested` names — or auto-select when `None`.
pub fn resolve<'a>(
  devices: &'a [Device],
  requested: Option<&str>,
  artifact: Artifact,
) -> io::Result<&'a Device> {
  match requested {
    Some(requested) => resolve_named(devices, requested, artifact),
    None => auto_select(devices, artifact),
  }
}

/// Resolve a `--device` value — a device name or UDID, matched
/// case-insensitively. A name that exists under several runtime
/// versions resolves to the booted one, else the newest.
fn resolve_named<'a>(
  devices: &'a [Device],
  requested: &str,
  artifact: Artifact,
) -> io::Result<&'a Device> {
  let device = devices
    .iter()
    .filter(|device| {
      device.udid.eq_ignore_ascii_case(requested)
        || device.name.eq_ignore_ascii_case(requested)
    })
    .max_by_key(|device| (device.booted, version_key(&device.os_version)));

  let Some(device) = device else {
    return Err(io::Error::other(format!(
      "no simulator device named '{requested}' — devices able to run \
       this app:\n{}",
      compatible_listing(devices, artifact),
    )));
  };

  if !artifact.supports(device.os) {
    return Err(io::Error::other(format!(
      "'{}' runs {}, which cannot run this app — choose {}:\n{}",
      device.name,
      device.os,
      artifact.device_label(),
      compatible_listing(devices, artifact),
    )));
  }

  Ok(device)
}

/// Auto-select a device: the booted one wins, else the newest iPhone,
/// then iPad, then Apple Vision Pro (for a watch app: the booted
/// watch, else the newest).
fn auto_select(devices: &[Device], artifact: Artifact) -> io::Result<&Device> {
  devices
    .iter()
    .filter(|device| artifact.supports(device.os))
    .max_by_key(|device| {
      (
        device.booted,
        family_rank(device),
        version_key(&device.os_version),
      )
    })
    .ok_or_else(|| {
      io::Error::other(format!(
        "no simulator device able to run this app is available — \
         install a runtime providing {} first",
        artifact.device_label(),
      ))
    })
}

/// Auto-select preference: a plain `zo run --target ios` should land
/// on the device family the app is designed for.
fn family_rank(device: &Device) -> u8 {
  if device.name.starts_with("iPhone") {
    2
  } else if device.name.starts_with("iPad") {
    1
  } else {
    0
  }
}

/// Orders runtime versions numerically — `26.5` above `17.5`, which a
/// string compare gets wrong.
fn version_key(version: &str) -> (u32, u32) {
  let mut parts = version.splitn(2, '.');
  let mut next = || parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);

  (next(), next())
}

/// One indented line per device able to run the app, for error
/// messages.
fn compatible_listing(devices: &[Device], artifact: Artifact) -> String {
  let mut listing = String::new();

  for device in devices {
    if !artifact.supports(device.os) {
      continue;
    }

    listing.push_str(&format!(
      "  {} ({} {}){}\n",
      device.name,
      device.os,
      device.os_version,
      if device.booted { ", booted" } else { "" },
    ));
  }

  if listing.is_empty() {
    listing.push_str("  (none)\n");
  }

  listing.pop();
  listing
}
