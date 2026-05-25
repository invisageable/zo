//! Cryptographic digests — SHA-1 and SHA-256.

use std::cell::RefCell;
use std::os::raw::c_char;

use sha1::{Digest, Sha1};
use sha2::Sha256;

use zo_c_abi::{CBytes, cstr_to_str, stage_cbytes};

thread_local! {
  static SCRATCH: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// SHA-1 digest of `input` as a 40-char lowercase hex string.
///
/// @note — SHA-1 is broken for collision resistance; use it
/// only for non-security checksums (git object IDs, dedup
/// keys). Reach for `sha256` when integrity matters.
///
/// # Safety
///
/// `input` must be a NUL-terminated UTF-8 string or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zo_hash_sha1(input: *const c_char) -> CBytes {
  let input = unsafe { cstr_to_str(input) };
  let digest = Sha1::digest(input.as_bytes());

  stage_cbytes(&SCRATCH, hex_lower(&digest).as_bytes())
}

/// SHA-256 digest of `input` as a 64-char lowercase hex string.
///
/// # Safety
///
/// `input` must be a NUL-terminated UTF-8 string or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zo_hash_sha256(input: *const c_char) -> CBytes {
  let input = unsafe { cstr_to_str(input) };
  let digest = Sha256::digest(input.as_bytes());

  stage_cbytes(&SCRATCH, hex_lower(&digest).as_bytes())
}

/// Lowercase hex of an arbitrary byte slice.
///
/// @note — hand-rolled to dodge a `hex` / `base16ct` dep for
/// a 5-line function. Output length is always `2 * bytes.len()`.
fn hex_lower(bytes: &[u8]) -> String {
  const HEX: &[u8; 16] = b"0123456789abcdef";

  let mut out = String::with_capacity(bytes.len() * 2);

  for &b in bytes {
    out.push(HEX[(b >> 4) as usize] as char);
    out.push(HEX[(b & 0x0f) as usize] as char);
  }

  out
}
