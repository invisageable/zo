//! Base64 encode / decode — standard alphabet, padded.

use zo_c_abi::{CBytes, cstr_to_str, stage_cbytes};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use std::cell::RefCell;
use std::os::raw::c_char;

thread_local! {
  static SCRATCH: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Base64-encode `input` with the standard alphabet (padded).
///
/// # Safety
///
/// `input` must be a NUL-terminated UTF-8 string or null.
#[unsafe(export_name = "zo_base64_encode")]
pub unsafe extern "C" fn _zo_base64_encode(input: *const c_char) -> CBytes {
  let input = unsafe { cstr_to_str(input) };
  let encoded = STANDARD.encode(input.as_bytes());

  stage_cbytes(&SCRATCH, encoded.as_bytes())
}

/// Base64-decode `input`; empty bytes on invalid input.
///
/// @note — decoded payload is returned as raw bytes via
/// `CBytes`, so it may contain interior NULs. `to_str()`
/// heap-copies into a length-prefixed zo `str` that preserves
/// them.
///
/// # Safety
///
/// `input` must be a NUL-terminated ASCII string or null.
#[unsafe(export_name = "zo_base64_decode")]
pub unsafe extern "C" fn _zo_base64_decode(input: *const c_char) -> CBytes {
  let input = unsafe { cstr_to_str(input) };

  match STANDARD.decode(input.as_bytes()) {
    Ok(bytes) => stage_cbytes(&SCRATCH, &bytes),
    Err(_) => CBytes::empty(),
  }
}
