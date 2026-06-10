use crate::ios::device::{self, Artifact, Os};

/// A captured `simctl list devices available` listing: every runtime
/// family, parenthesized device names, and one booted device.
const LISTING: &str = "\
== Devices ==
-- iOS 17.5 --
    iPhone 15 Pro (CABE096E-69EF-44E2-8006-5D0E5F4BD4BF) (Shutdown)
    iPhone SE (3rd generation) (A03CF08D-0C01-400A-92DD-D355C9E6BA29) (Shutdown)
-- iOS 26.5 --
    iPhone 17 Pro (A69E4AEF-5111-4064-8798-4224637CBAD7) (Shutdown)
    iPad Pro 13-inch (M5) (564BE819-1E5C-4162-9F5B-811C1705F6C1) (Shutdown)
-- tvOS 26.5 --
    Apple TV 4K (3rd generation) (at 1080p) (78E67410-15E8-4E17-AC0F-5EBA641D9EEB) (Shutdown)
-- watchOS 26.5 --
    Apple Watch Series 11 (46mm) (3EC22E96-32CB-4DEE-9DE7-42CB5015C78E) (Shutdown)
-- visionOS 1.2 --
    Apple Vision Pro (CB338DD3-E0EE-4A0B-B1BC-3D7CBB126D23) (Shutdown)
-- visionOS 26.5 --
    Apple Vision Pro (F99652F9-4274-40CD-BEC5-B6F6F6F2DF18) (Booted)
";

#[test]
fn parse_extracts_every_device() {
  let devices = device::parse(LISTING);

  assert_eq!(devices.len(), 8);
}

#[test]
fn parse_keeps_parenthesized_names_intact() {
  let devices = device::parse(LISTING);
  let tv = devices
    .iter()
    .find(|d| d.udid == "78E67410-15E8-4E17-AC0F-5EBA641D9EEB")
    .unwrap();

  assert_eq!(tv.name, "Apple TV 4K (3rd generation) (at 1080p)");
  assert_eq!(tv.os, Os::TvOs);
  assert_eq!(tv.os_version, "26.5");
  assert!(!tv.booted);

  let se = devices
    .iter()
    .find(|d| d.udid == "A03CF08D-0C01-400A-92DD-D355C9E6BA29")
    .unwrap();

  assert_eq!(se.name, "iPhone SE (3rd generation)");
  assert_eq!(se.os, Os::Ios);
}

#[test]
fn parse_reads_the_booted_state() {
  let devices = device::parse(LISTING);
  let vision = devices
    .iter()
    .find(|d| d.udid == "F99652F9-4274-40CD-BEC5-B6F6F6F2DF18")
    .unwrap();

  assert!(vision.booted);
}

#[test]
fn resolve_matches_names_case_insensitively() {
  let devices = device::parse(LISTING);
  let device =
    device::resolve(&devices, Some("iphone 17 pro"), Artifact::Ios).unwrap();

  assert_eq!(device.udid, "A69E4AEF-5111-4064-8798-4224637CBAD7");
}

#[test]
fn resolve_matches_a_udid() {
  let devices = device::parse(LISTING);
  let device = device::resolve(
    &devices,
    Some("cabe096e-69ef-44e2-8006-5d0e5f4bd4bf"),
    Artifact::Ios,
  )
  .unwrap();

  assert_eq!(device.name, "iPhone 15 Pro");
}

#[test]
fn resolve_prefers_the_booted_duplicate() {
  // `Apple Vision Pro` exists under visionOS 1.2 (shut down) and
  // visionOS 26.5 (booted) — the booted one must win.
  let devices = device::parse(LISTING);
  let device =
    device::resolve(&devices, Some("Apple Vision Pro"), Artifact::Ios).unwrap();

  assert_eq!(device.udid, "F99652F9-4274-40CD-BEC5-B6F6F6F2DF18");
}

#[test]
fn resolve_rejects_watch_devices_with_the_runtime_contract() {
  let devices = device::parse(LISTING);
  let error = device::resolve(
    &devices,
    Some("Apple Watch Series 11 (46mm)"),
    Artifact::Ios,
  )
  .unwrap_err();
  let message = error.to_string();

  assert!(message.contains("watchOS"), "{message}");
  assert!(message.contains("iPhone 17 Pro"), "{message}");
}

#[test]
fn resolve_lists_candidates_for_an_unknown_name() {
  let devices = device::parse(LISTING);
  let error =
    device::resolve(&devices, Some("iPhone 99"), Artifact::Ios).unwrap_err();
  let message = error.to_string();

  assert!(message.contains("iPhone 99"), "{message}");
  assert!(message.contains("Apple Vision Pro"), "{message}");
  assert!(!message.contains("Apple Watch"), "{message}");
}

#[test]
fn auto_select_prefers_the_booted_device() {
  let devices = device::parse(LISTING);
  let device = device::resolve(&devices, None, Artifact::Ios).unwrap();

  assert_eq!(device.udid, "F99652F9-4274-40CD-BEC5-B6F6F6F2DF18");
}

#[test]
fn auto_select_falls_back_to_the_newest_iphone() {
  let listing = LISTING.replace("(Booted)", "(Shutdown)");
  let devices = device::parse(&listing);
  let device = device::resolve(&devices, None, Artifact::Ios).unwrap();

  assert_eq!(device.name, "iPhone 17 Pro");
  assert_eq!(device.os_version, "26.5");
}

#[test]
fn auto_select_fails_without_a_compatible_device() {
  let devices = device::parse(
    "-- watchOS 26.5 --\n    Apple Watch Series 11 (46mm) \
     (3EC22E96-32CB-4DEE-9DE7-42CB5015C78E) (Shutdown)",
  );
  let error = device::resolve(&devices, None, Artifact::Ios).unwrap_err();

  assert!(error.to_string().contains("visionOS"));
}

#[test]
fn watch_artifact_resolves_a_watch_device() {
  let devices = device::parse(LISTING);
  let device = device::resolve(
    &devices,
    Some("Apple Watch Series 11 (46mm)"),
    Artifact::Watchos,
  )
  .unwrap();

  assert_eq!(device.udid, "3EC22E96-32CB-4DEE-9DE7-42CB5015C78E");
}

#[test]
fn watch_artifact_auto_selects_a_watch() {
  let devices = device::parse(LISTING);
  let device = device::resolve(&devices, None, Artifact::Watchos).unwrap();

  assert_eq!(device.os, Os::WatchOs);
}

#[test]
fn watch_artifact_rejects_an_iphone() {
  let devices = device::parse(LISTING);
  let error =
    device::resolve(&devices, Some("iPhone 17 Pro"), Artifact::Watchos)
      .unwrap_err();
  let message = error.to_string();

  assert!(message.contains("watchOS device"), "{message}");
  assert!(message.contains("Apple Watch"), "{message}");
}
