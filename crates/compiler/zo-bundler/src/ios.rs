//! iOS `.app` bundle construction.
//!
//! An iOS app is a flat directory: the executable at the root, an
//! `Info.plist`, a `PkgInfo`, and embedded dylibs under `Frameworks/`.
//! The binary and the runtime dylib are already ad-hoc signed (by the
//! linker and rustc respectively), which the Simulator accepts — so M1
//! does no re-signing and writes no `_CodeSignature` seal.

pub mod device;
pub mod simulator;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The Apple platform a bundle runs on — selects the `Info.plist`
/// platform values, the device family, and the watch-specific keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
  /// iPhone/iPad Simulator.
  IphoneSimulator,
  /// A physical iPhone/iPad.
  Iphone,
  /// Apple Watch Simulator.
  WatchSimulator,
}

/// Everything needed to lay down one iOS/watchOS `.app`.
pub struct BundleSpec<'a> {
  /// The linked, ad-hoc-signed Mach-O executable.
  pub binary: &'a Path,
  /// The platform runtime dylib to embed under `Frameworks/`.
  pub runtime_dylib: &'a Path,
  /// The output `App.app` directory (created fresh each call).
  pub app_dir: &'a Path,
  /// App display name and inner executable name.
  pub name: &'a str,
  /// Bundle identifier, e.g. `house.compilords.counter`.
  pub bundle_id: &'a str,
  /// The platform the bundle runs on.
  pub platform: Platform,
  /// The program's stylesheets. The bundler scans them for local
  /// `background-image` assets to copy into the bundle, so a shipped
  /// app reaches them (its sandbox sees only its own `.app`).
  pub stylesheets: &'a [&'a str],
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
  crate::set_executable(&exe)?;

  fs::write(spec.app_dir.join("Info.plist"), info_plist(spec))?;
  fs::write(spec.app_dir.join("PkgInfo"), b"APPL????")?;

  let frameworks = spec.app_dir.join("Frameworks");

  fs::create_dir_all(&frameworks)?;
  fs::copy(spec.runtime_dylib, frameworks.join("libzo_runtime.dylib"))?;

  // Copy referenced local images into the bundle root so the runtime
  // resolves them by basename on a device, not just in the Simulator.
  for asset in collect_local_assets(spec.stylesheets) {
    if let Some(name) = asset.file_name() {
      fs::copy(&asset, spec.app_dir.join(name))?;
    }
  }

  Ok(())
}

/// Local image source paths the program's stylesheets reference,
/// resolved to absolute against the build working directory. URLs and
/// missing files are skipped — only existing local files are bundled.
fn collect_local_assets(stylesheets: &[&str]) -> Vec<PathBuf> {
  let cwd = std::env::current_dir().unwrap_or_default();
  let mut assets: Vec<PathBuf> = Vec::new();

  for css in stylesheets {
    for url in zo_ui_protocol::style::css::parse(css).images {
      if url.starts_with("http://") || url.starts_with("https://") {
        continue;
      }

      let path = Path::new(&url);
      let absolute = if path.is_absolute() {
        path.to_path_buf()
      } else {
        cwd.join(path)
      };

      if absolute.exists() && !assets.contains(&absolute) {
        assets.push(absolute);
      }
    }
  }

  assets
}

/// The `Info.plist` the Simulator needs to install + launch the app:
/// identity, package type, the platform it was built for, an empty
/// `UILaunchScreen` so the app draws full-screen, and the
/// `UIApplicationSceneManifest` that opts into the `UIScene` lifecycle
/// and names `ZoSceneDelegate` — without it UIKit stays on the legacy
/// app-delegate window path and never connects the scene.
///
/// A watch bundle differs on every platform-derived value: device
/// family `4`, the `WKApplication`/`WKWatchOnly` keys that mark a
/// standalone watch app (installd rejects the bundle without them),
/// and the PepperUI Carousel scene role.
fn info_plist(spec: &BundleSpec) -> String {
  let (platform, dt_platform) = match spec.platform {
    Platform::IphoneSimulator => ("iPhoneSimulator", "iphonesimulator"),
    Platform::Iphone => ("iPhoneOS", "iphoneos"),
    Platform::WatchSimulator => ("WatchSimulator", "watchsimulator"),
  };
  let watch = spec.platform == Platform::WatchSimulator;
  let minimum_os = if watch { "9.0" } else { "15.0" };
  let device_family = if watch {
    "<integer>4</integer>"
  } else {
    "<integer>1</integer><integer>2</integer>"
  };
  let scene_role = if watch {
    "PUICApplicationSceneSessionRoleApplication"
  } else {
    "UIWindowSceneSessionRoleApplication"
  };
  let watch_keys = if watch {
    "\n  <key>WKApplication</key><true/>\n  \
     <key>WKWatchOnly</key><true/>"
  } else {
    ""
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
  <key>MinimumOSVersion</key><string>{minimum_os}</string>
  <key>UIDeviceFamily</key><array>{device_family}</array>{watch_keys}
  <key>UILaunchScreen</key><dict/>
  <key>UIApplicationSceneManifest</key>
  <dict>
    <key>UIApplicationSupportsMultipleScenes</key><false/>
    <key>UISceneConfigurations</key>
    <dict>
      <key>{scene_role}</key>
      <array>
        <dict>
          <key>UISceneConfigurationName</key><string>Default Configuration</string>
          <key>UISceneDelegateClassName</key><string>ZoSceneDelegate</string>
        </dict>
      </array>
    </dict>
  </dict>
</dict>
</plist>
"#,
    name = spec.name,
    id = spec.bundle_id,
    platform = platform,
    dt_platform = dt_platform,
    minimum_os = minimum_os,
    device_family = device_family,
    watch_keys = watch_keys,
    scene_role = scene_role,
  )
}
