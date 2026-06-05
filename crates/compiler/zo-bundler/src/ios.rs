//! iOS `.app` bundle construction.
//!
//! An iOS app is a flat directory: the executable at the root, an
//! `Info.plist`, a `PkgInfo`, and embedded dylibs under `Frameworks/`.
//! The binary and the runtime dylib are already ad-hoc signed (by the
//! linker and rustc respectively), which the Simulator accepts — so M1
//! does no re-signing and writes no `_CodeSignature` seal.

use std::fs;
use std::io;
use std::path::Path;

/// Everything needed to lay down one iOS `.app`.
pub struct BundleSpec<'a> {
  /// The linked, ad-hoc-signed Mach-O executable.
  pub binary: &'a Path,
  /// The iOS runtime dylib to embed under `Frameworks/`.
  pub runtime_dylib: &'a Path,
  /// The output `App.app` directory (created fresh each call).
  pub app_dir: &'a Path,
  /// App display name and inner executable name.
  pub name: &'a str,
  /// Bundle identifier, e.g. `house.compilords.counter`.
  pub bundle_id: &'a str,
  /// Whether the bundle targets the Simulator (vs a device) — selects
  /// the `CFBundleSupportedPlatforms` / `DTPlatformName` values.
  pub simulator: bool,
}

/// Build a runnable `.app` from `spec`. Overwrites any existing bundle
/// at `app_dir` so each build starts clean.
pub fn bundle(spec: &BundleSpec) -> io::Result<()> {
  if spec.app_dir.exists() {
    fs::remove_dir_all(spec.app_dir)?;
  }

  fs::create_dir_all(spec.app_dir)?;

  // Flat iOS layout: the executable sits at `App.app/<name>` and the
  // copy preserves the embedded ad-hoc signature.
  let exe = spec.app_dir.join(spec.name);

  fs::copy(spec.binary, &exe)?;
  set_executable(&exe)?;

  fs::write(spec.app_dir.join("Info.plist"), info_plist(spec))?;
  fs::write(spec.app_dir.join("PkgInfo"), b"APPL????")?;

  let frameworks = spec.app_dir.join("Frameworks");

  fs::create_dir_all(&frameworks)?;
  fs::copy(spec.runtime_dylib, frameworks.join("libzo_runtime.dylib"))?;

  Ok(())
}

/// The minimal `Info.plist` the Simulator needs to install + launch a
/// legacy (non-scene) UIKit app: identity, package type, the platform
/// it was built for, and an empty `UILaunchScreen` so the app draws
/// full-screen instead of letterboxed.
fn info_plist(spec: &BundleSpec) -> String {
  let (platform, dt_platform) = if spec.simulator {
    ("iPhoneSimulator", "iphonesimulator")
  } else {
    ("iPhoneOS", "iphoneos")
  };

  format!(
    r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>{name}</string>
  <key>CFBundleIdentifier</key><string>{id}</string>
  <key>CFBundleName</key><string>{name}</string>
  <key>CFBundleDisplayName</key><string>{name}</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>CFBundleShortVersionString</key><string>1.0</string>
  <key>CFBundleSupportedPlatforms</key><array><string>{platform}</string></array>
  <key>DTPlatformName</key><string>{dt_platform}</string>
  <key>MinimumOSVersion</key><string>15.0</string>
  <key>UIDeviceFamily</key><array><integer>1</integer><integer>2</integer></array>
  <key>UILaunchScreen</key><dict/>
</dict>
</plist>
"#,
    name = spec.name,
    id = spec.bundle_id,
    platform = platform,
    dt_platform = dt_platform,
  )
}

/// Mark the copied executable `rwxr-xr-x` — `fs::copy` drops the bit
/// on some filesystems and the Simulator refuses a non-executable.
fn set_executable(path: &Path) -> io::Result<()> {
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;

    let mut perms = fs::metadata(path)?.permissions();

    perms.set_mode(0o755);
    fs::set_permissions(path, perms)?;
  }

  Ok(())
}
