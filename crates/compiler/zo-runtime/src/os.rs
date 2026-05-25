//! Platform detection — `core/os.zo` FFI backing.
//!
//! Returns CBytes (ptr, len) pointing at static string
//! data. Zero allocation — the strings live in .rodata.

use crate::str::alloc_str;

/// # Safety
///
/// No preconditions — returns a pointer to static data.
#[unsafe(export_name = "zo_os_name")]
pub unsafe extern "C-unwind" fn _zo_os_name() -> *const u8 {
  #[cfg(target_os = "macos")]
  {
    alloc_str(b"macos")
  }

  #[cfg(target_os = "linux")]
  {
    alloc_str(b"linux")
  }

  #[cfg(not(any(target_os = "macos", target_os = "linux")))]
  {
    alloc_str(b"unknown")
  }
}

/// # Safety
///
/// No preconditions — returns a pointer to static data.
#[unsafe(export_name = "zo_os_arch")]
pub unsafe extern "C-unwind" fn _zo_os_arch() -> *const u8 {
  #[cfg(target_arch = "aarch64")]
  {
    alloc_str(b"aarch64")
  }

  #[cfg(target_arch = "x86_64")]
  {
    alloc_str(b"x86_64")
  }

  #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
  {
    alloc_str(b"unknown")
  }
}

/// # Safety
///
/// No preconditions — returns a pointer to static data.
#[unsafe(export_name = "zo_os_family")]
pub unsafe extern "C-unwind" fn _zo_os_family() -> *const u8 {
  #[cfg(unix)]
  {
    alloc_str(b"unix")
  }

  #[cfg(windows)]
  {
    alloc_str(b"windows")
  }

  #[cfg(not(any(unix, windows)))]
  {
    alloc_str(b"unknown")
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn read_zo_str(ptr: *const u8) -> String {
    let bytes = unsafe { crate::str::str_bytes(ptr) };

    std::str::from_utf8(bytes).unwrap().to_owned()
  }

  #[test]
  fn os_name_is_known() {
    let name = read_zo_str(unsafe { _zo_os_name() });

    assert!(
      name == "macos" || name == "linux",
      "unexpected os name: {name}"
    );
  }

  #[test]
  fn os_arch_is_known() {
    let arch = read_zo_str(unsafe { _zo_os_arch() });

    assert!(
      arch == "aarch64" || arch == "x86_64",
      "unexpected arch: {arch}"
    );
  }

  #[test]
  fn os_family_is_unix() {
    let family = read_zo_str(unsafe { _zo_os_family() });

    assert_eq!(family, "unix");
  }
}
