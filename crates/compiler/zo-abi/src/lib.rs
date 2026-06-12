//! The toolchain ↔ runtime ABI contract.
//!
//! A dependency-free leaf both sides share: the runtime cdylib
//! embeds [`RUNTIME_ABI_TAG`] as bytes, and `zo build` scans the
//! staged dylib for it before shipping — a mismatch means the
//! staged runtime would decode an older context layout and
//! silently drop behavior, so the build refuses instead. Neither
//! the compiler nor the runtime needs the other's types for this;
//! the contract is the only shared surface.

/// Runtime ABI tag, embedded verbatim in the runtime dylib and
/// scanned by `zo build` from the staged copy.
///
/// @note — BUMP THE SUFFIX whenever `ZoRuntimeContext` or any
/// `#[repr(C)]` binding ABI changes shape.
pub const RUNTIME_ABI_TAG: &[u8; 14] = b"ZO_RT_ABI:0001";

/// The tag's scan prefix — version-independent, used to locate the
/// tag bytes inside a staged dylib.
pub const RUNTIME_ABI_TAG_PREFIX: &[u8; 10] = b"ZO_RT_ABI:";

/// Locates the `ZO_RT_ABI:` tag inside a dylib's bytes, returning
/// the full tag slice (prefix + version digits) when present.
pub fn find_abi_tag(bytes: &[u8]) -> Option<&[u8]> {
  let prefix = RUNTIME_ABI_TAG_PREFIX.as_slice();
  let tag_len = RUNTIME_ABI_TAG.len();

  bytes
    .windows(prefix.len())
    .position(|window| window == prefix)
    .and_then(|at| bytes.get(at..at + tag_len))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn finds_the_tag_anywhere_in_the_bytes() {
    let mut bytes = vec![0u8; 64];

    bytes.extend_from_slice(RUNTIME_ABI_TAG);
    bytes.extend_from_slice(&[0u8; 32]);

    assert_eq!(find_abi_tag(&bytes), Some(RUNTIME_ABI_TAG.as_slice()));
  }

  #[test]
  fn missing_tag_reads_none() {
    assert_eq!(find_abi_tag(&[0u8; 128]), None);
  }

  #[test]
  fn a_stale_tag_is_returned_for_the_diagnostic() {
    let mut bytes = b"ZO_RT_ABI:0000".to_vec();

    bytes.extend_from_slice(&[0u8; 16]);

    assert_eq!(find_abi_tag(&bytes), Some(b"ZO_RT_ABI:0000".as_slice()));
  }
}
