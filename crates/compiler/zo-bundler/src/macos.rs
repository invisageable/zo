//! macOS `.app` bundle construction.
//!
//! A macOS app is `Foo.app/Contents/{Info.plist, MacOS/<exe>, …}`. The
//! desktop binary loads its runtime through `@loader_path/deps/…`, so
//! the dylib sits in `Contents/MacOS/deps/` next to the executable —
//! the same layout the loose desktop build uses, just inside the
//! bundle. The binary and dylib are already ad-hoc signed (linker +
//! rustc), which macOS accepts for a locally-built app, so the bundler
//! does no re-signing.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Everything needed to lay down one macOS `.app`.
pub struct BundleSpec<'a> {
  /// The linked, ad-hoc-signed Mach-O executable.
  pub binary: &'a Path,
  /// The desktop runtime dylib to embed under `MacOS/deps/`.
  pub runtime_dylib: &'a Path,
  /// The output `App.app` directory (created fresh each call).
  pub app_dir: &'a Path,
  /// App display name and inner executable name.
  pub name: &'a str,
  /// Bundle identifier, e.g. `house.compilords.counter`.
  pub bundle_id: &'a str,
  /// Local files the program references (`<img>` srcs, CSS
  /// `background-image`s). Copied into `Contents/Resources/` by
  /// basename, where the webview runtime resolves them for a relocated
  /// bundle.
  pub assets: &'a [PathBuf],
}

/// Build a runnable `.app` from `spec`. Overwrites any existing bundle
/// at `app_dir` so each build starts clean.
pub fn bundle(spec: &BundleSpec) -> io::Result<()> {
  if spec.app_dir.exists() {
    fs::remove_dir_all(spec.app_dir)?;
  }

  let contents = spec.app_dir.join("Contents");
  let macos = contents.join("MacOS");
  let deps = macos.join("deps");

  fs::create_dir_all(&deps)?;

  // Executable at `Contents/MacOS/<name>`; the copy preserves the
  // embedded ad-hoc signature.
  let exe = macos.join(spec.name);

  fs::copy(spec.binary, &exe)?;
  crate::set_executable(&exe)?;

  // The desktop binary's `LC_LOAD_DYLIB` is
  // `@loader_path/deps/libzo_runtime.dylib`; `@loader_path` is the
  // executable's own directory, so the dylib lands beside it in
  // `deps/`.
  fs::copy(spec.runtime_dylib, deps.join("libzo_runtime.dylib"))?;

  // Referenced assets land in `Contents/Resources/<basename>`, the
  // location the webview runtime falls back to when a baked `src` no
  // longer resolves on the running machine.
  if !spec.assets.is_empty() {
    let resources = contents.join("Resources");

    fs::create_dir_all(&resources)?;

    for asset in spec.assets {
      if let Some(name) = asset.file_name() {
        fs::copy(asset, resources.join(name))?;
      }
    }
  }

  fs::write(contents.join("Info.plist"), info_plist(spec))?;
  fs::write(contents.join("PkgInfo"), b"APPL????")?;

  Ok(())
}

/// The `Info.plist` macOS needs to launch the app: identity, package
/// type, and a minimum system version. No UIKit scene manifest — that
/// is iOS-only.
fn info_plist(spec: &BundleSpec) -> String {
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
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
"#,
    name = spec.name,
    id = spec.bundle_id,
  )
}
