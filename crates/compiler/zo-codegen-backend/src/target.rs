/// Represents the target backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Target {
  /// ARM64 macOS target (arm64-apple-darwin).
  Arm64AppleDarwin,
  /// (arm64-pc-windows-msvc).
  Arm64PcWindowsMsvc,
  /// (arm64-unknown-linux-gnu).
  Arm64UnknownLinuxGnu,
  /// 64-bit macOS target (x86_64-apple-darwin).
  X8664AppleDarwin,
  /// 64-bit Windows target (x86_64-pc-windows-msvc).
  X8664PcWindowsMsvc,
  /// 64-bit Windows target (x86_64-unknown-linux-gnu).
  X8664UnknownLinuxGnu,
  /// Wasm target (wasm32-unknown-unknown).
  Wasm32UnknownUnknown,
  /// arm64 iOS device (Mach-O, PLATFORM_IOS).
  Arm64AppleIos,
  /// arm64 iOS Simulator on Apple Silicon
  /// (Mach-O, PLATFORM_IOSSIMULATOR).
  Arm64AppleIosSim,
  /// arm64 Android (ELF, bionic libc).
  Aarch64LinuxAndroid,
}

impl Target {
  /// Gets the host target based on the current platform.
  pub const fn host() -> Self {
    #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
    return Self::Arm64AppleDarwin;

    #[cfg(all(target_arch = "aarch64", target_os = "windows"))]
    return Self::Arm64PcWindowsMsvc;

    #[cfg(all(target_arch = "aarch64", target_os = "linux"))]
    return Self::Arm64UnknownLinuxGnu;

    #[cfg(all(target_arch = "x86_64", target_os = "macos"))]
    return Self::X8664AppleDarwin;

    #[cfg(all(target_arch = "x86_64", target_os = "windows"))]
    return Self::X8664PcWindowsMsvc;

    #[cfg(all(target_arch = "x86_64", target_os = "linux"))]
    return Self::X8664UnknownLinuxGnu;

    #[cfg(target_arch = "wasm32")]
    return Self::Wasm32UnknownUnknown;
  }

  /// Gets the [`Target`] name.
  pub fn name(self) -> &'static str {
    match self {
      Self::Arm64AppleDarwin => "arm64-apple-darwin",
      Self::Arm64PcWindowsMsvc => "arm64-pc-windows-msvc",
      Self::Arm64UnknownLinuxGnu => "arm64-unknown-linux-gnu",
      Self::X8664AppleDarwin => "x86_64-apple-darwin",
      Self::X8664PcWindowsMsvc => "x86_64-pc-windows-msvc",
      Self::X8664UnknownLinuxGnu => "x86_64-unknown-linux-gnu",
      Self::Wasm32UnknownUnknown => "wasm32-unknown-unknown",
      Self::Arm64AppleIos => "arm64-apple-ios",
      Self::Arm64AppleIosSim => "arm64-apple-ios-sim",
      Self::Aarch64LinuxAndroid => "aarch64-linux-android",
    }
  }

  /// Gets the [`Target`] extension stem.
  pub fn extension(self) -> &'static str {
    match self {
      Self::Arm64AppleDarwin
      | Self::Arm64PcWindowsMsvc
      | Self::X8664AppleDarwin
      | Self::X8664PcWindowsMsvc => "exe",
      Self::Arm64UnknownLinuxGnu | Self::X8664UnknownLinuxGnu => "",
      Self::Wasm32UnknownUnknown => "wasm",
      Self::Arm64AppleIos | Self::Arm64AppleIosSim => "app",
      Self::Aarch64LinuxAndroid => "apk",
    }
  }

  /// Whether the target is an Apple platform — Mach-O object
  /// format and ad-hoc code signing apply.
  pub fn is_apple(self) -> bool {
    matches!(
      self,
      Self::Arm64AppleDarwin
        | Self::X8664AppleDarwin
        | Self::Arm64AppleIos
        | Self::Arm64AppleIosSim
    )
  }

  /// Whether the target's object format is Mach-O.
  pub fn is_macho(self) -> bool {
    self.is_apple()
  }

  /// Whether the target is a mobile platform (iOS or Android).
  pub fn is_mobile(self) -> bool {
    matches!(
      self,
      Self::Arm64AppleIos
        | Self::Arm64AppleIosSim
        | Self::Aarch64LinuxAndroid
    )
  }
}
