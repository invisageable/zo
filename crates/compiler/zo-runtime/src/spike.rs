//! NOT FOR USER CODE — codegen-regression fixtures.
//!
//! @note — `_zo_spike_pair` backs `programming/codegen/
//! abi_composite_return.zo`, the CHECK test that pins the
//! `AbiRet::Composite` lift. The symbol ships in every zo
//! binary because the runtime is a single cdylib; size
//! cost is one i64+i64 returner. Delete only when a
//! permanent FFI surface replaces the spike.

/// 2-field 16B composite — AAPCS returns in (X0, X1).
#[repr(C)]
pub struct SpikePair {
  pub a: i64,
  pub b: i64,
}

/// Return a fixed `(a: 42, b: 100)` pair for codegen tests.
#[unsafe(export_name = "zo_spike_pair")]
pub extern "C" fn _zo_spike_pair() -> SpikePair {
  SpikePair { a: 42, b: 100 }
}
