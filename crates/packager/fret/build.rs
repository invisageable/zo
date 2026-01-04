fn main() {
  // Link advapi32 on Windows for SystemFunction036 (RtlGenRandom)
  #[cfg(target_os = "windows")]
  println!("cargo:rustc-link-lib=advapi32");
}
