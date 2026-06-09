//! Simulator device discovery and resolution.
//!
//! `zo run --target ios` accepts any device the machine actually has:
//! [`detect`] asks `simctl` for the available devices and [`resolve`]
//! picks one from the `--device` flag — or auto-selects when the flag
//! is omitted. zo apps are iPhone-family binaries, so resolution also
//! guards the runtime contract: iOS and visionOS simulators can run
//! them, watchOS and tvOS simulators cannot.

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

  /// Whether this runtime installs and launches iPhone-family apps.
  ///
  /// @note — visionOS runs them through its iOS app-compatibility
  /// layer; watchOS and tvOS only load binaries built against their
  /// own platform SDK, so an iPhone-family binary fails at install or
  /// launch.
  pub fn runs_ios_apps(self) -> bool {
    matches!(self, Self::Ios | Self::VisionOs)
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
) -> io::Result<&'a Device> {
  match requested {
    Some(requested) => resolve_named(devices, requested),
    None => auto_select(devices),
  }
}

/// Resolve a `--device` value — a device name or UDID, matched
/// case-insensitively. A name that exists under several runtime
/// versions resolves to the booted one, else the newest.
fn resolve_named<'a>(
  devices: &'a [Device],
  requested: &str,
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
      compatible_listing(devices),
    )));
  };

  if !device.os.runs_ios_apps() {
    return Err(io::Error::other(format!(
      "'{}' runs {}, which cannot run iPhone-family apps — choose an \
       iOS or visionOS device:\n{}",
      device.name,
      device.os,
      compatible_listing(devices),
    )));
  }

  Ok(device)
}

/// Auto-select a device: the booted one wins, else the newest iPhone,
/// then iPad, then Apple Vision Pro.
fn auto_select(devices: &[Device]) -> io::Result<&Device> {
  devices
    .iter()
    .filter(|device| device.os.runs_ios_apps())
    .max_by_key(|device| {
      (
        device.booted,
        family_rank(device),
        version_key(&device.os_version),
      )
    })
    .ok_or_else(|| {
      io::Error::other(
        "no iOS or visionOS simulator devices are available — install \
         an iOS simulator runtime first",
      )
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
fn compatible_listing(devices: &[Device]) -> String {
  let mut listing = String::new();

  for device in devices {
    if !device.os.runs_ios_apps() {
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
