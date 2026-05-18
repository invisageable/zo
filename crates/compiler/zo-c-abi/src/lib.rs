//! Shared C-ABI types between `zo-runtime` and provider
//! cdylibs (`zo-provider-json`, future HTTP / etc.).
//!
//! Single source of truth for `CBytes` so the AAPCS
//! `Composite` return layout stays identical across every
//! string-returning FFI the zo side consumes via
//! `CBytes::to_str()` on `core/c.zo`.

use std::cell::RefCell;
use std::os::raw::c_char;
use std::thread::LocalKey;

/// 16B `(ptr, len)` byte-slice returned to zo as `CBytes`.
///
/// @note — `repr(C)` two-i64-field shape so AAPCS lifts it
/// through `AbiRet::Composite` (X0/X1). The zo-side mirror
/// is `pub struct CBytes { ptr: s64, len: s64 }` in
/// `core/c.zo`; keep field order/types in sync.
#[repr(C)]
pub struct CBytes {
  pub ptr: *const c_char,
  pub len: i64,
}

impl CBytes {
  /// Empty payload pointing at a static NUL byte.
  pub fn empty() -> Self {
    static NUL: u8 = 0;

    Self {
      ptr: &raw const NUL as *const c_char,
      len: 0,
    }
  }
}

/// Stage `bytes` + trailing NUL in `scratch` and return the
/// `(ptr, len)` pair. Pointer is valid until the next call
/// touching the same scratch on this thread.
pub fn stage_cbytes(
  scratch: &'static LocalKey<RefCell<Vec<u8>>>,
  bytes: &[u8],
) -> CBytes {
  scratch.with(|cell| {
    let mut out = cell.borrow_mut();

    out.clear();
    out.reserve(bytes.len() + 1);
    out.extend_from_slice(bytes);
    out.push(0);

    CBytes {
      ptr: out.as_ptr() as *const c_char,
      len: bytes.len() as i64,
    }
  })
}

/// Lift a NUL-terminated `*const c_char` to a borrowed `&str`.
///
/// @note — empty on null / non-UTF-8. The borrow's lifetime
/// is unbounded; callers must consume within the same FFI
/// body before any other write touches the source memory.
///
/// # Safety
///
/// `ptr` must be NUL-terminated UTF-8 or null.
pub unsafe fn cstr_to_str<'a>(ptr: *const c_char) -> &'a str {
  if ptr.is_null() {
    return "";
  }

  unsafe { std::ffi::CStr::from_ptr(ptr) }
    .to_str()
    .unwrap_or("")
}
