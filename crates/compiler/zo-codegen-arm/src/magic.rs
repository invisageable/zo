//! Magic-number computation for division by a constant.
//!
//! Division by a runtime-unknown value uses the hardware
//! `sdiv` / `udiv` — multi-cycle, unpipelined. When the divisor
//! is a compile-time constant, the quotient can instead be
//! computed with one high-multiply (`smulh` / `umulh`), a few
//! shifts, and a couple of adds — the technique optimizing C
//! compilers use to retire hardware divides from hot loops.
//!
//! The constants here follow Hacker's Delight, chapter 10
//! ("Integer Division by Constants"). Power-of-two divisors are
//! handled separately by the codegen (a shift), so these
//! routines assume a non-power-of-two magnitude — but they stay
//! correct for any non-zero divisor.

/// Signed magic numbers for `n / d`: the quotient is
/// `(SMULH(n, multiplier) [± n]) >>(arith) shift`, then the
/// dividend's sign bit is added back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SignedMagic {
  /// The 64-bit signed reciprocal fed to `smulh`.
  pub multiplier: i64,
  /// Arithmetic right-shift applied to the high product.
  pub shift: u32,
}

/// Unsigned magic numbers for `n / d`: the quotient is
/// `UMULH(n, multiplier)` then a logical shift, with an extra
/// add-and-rotate correction when `add` is set (the multiplier
/// did not fit in 64 bits on its own).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnsignedMagic {
  /// The 64-bit reciprocal fed to `umulh`.
  pub multiplier: u64,
  /// Whether the incremented-multiplier correction is needed.
  pub add: bool,
  /// Logical right-shift applied after the correction.
  pub shift: u32,
}

/// Compute the signed magic numbers for divisor `d`.
///
/// `d` must be non-zero and not ±1 (those are handled directly
/// by the codegen). Mirrors Hacker's Delight figure 10-1 at
/// 64-bit width.
pub fn signed_magic(d: i64) -> SignedMagic {
  debug_assert!(d != 0 && d != 1 && d != -1);

  const BITS: u32 = 64;
  // Two's-complement minimum at this width.
  const MIN: u64 = 1u64 << (BITS - 1);

  let ad = d.unsigned_abs();
  let t = MIN.wrapping_add((d as u64) >> (BITS - 1));
  // Initial quotient/remainder of the threshold by |d|.
  let anc = t.wrapping_sub(1).wrapping_sub(t.wrapping_rem(ad));

  let mut p: u32 = BITS - 1;
  let mut q1 = MIN / anc;
  let mut r1 = MIN - q1 * anc;
  let mut q2 = MIN / ad;
  let mut r2 = MIN - q2 * ad;

  loop {
    p += 1;

    q1 = q1.wrapping_mul(2);
    r1 = r1.wrapping_mul(2);
    if r1 >= anc {
      q1 = q1.wrapping_add(1);
      r1 = r1.wrapping_sub(anc);
    }

    q2 = q2.wrapping_mul(2);
    r2 = r2.wrapping_mul(2);
    if r2 >= ad {
      q2 = q2.wrapping_add(1);
      r2 = r2.wrapping_sub(ad);
    }

    let delta = ad - r2;

    if !(q1 < delta || (q1 == delta && r1 == 0)) {
      break;
    }
  }

  let mut multiplier = q2.wrapping_add(1) as i64;

  if d < 0 {
    multiplier = multiplier.wrapping_neg();
  }

  SignedMagic {
    multiplier,
    shift: p - BITS,
  }
}

/// Compute the unsigned magic numbers for divisor `d`.
///
/// `d` must be non-zero and not a power of two (those are a
/// shift). Mirrors Hacker's Delight figure 10-2 at 64-bit
/// width.
pub fn unsigned_magic(d: u64) -> UnsignedMagic {
  debug_assert!(d != 0);

  const BITS: u32 = 64;
  const MAX: u64 = u64::MAX;

  let mut add = false;
  // nc = largest multiple of d below 2^BITS, minus 1.
  let nc = MAX - (MAX.wrapping_sub(d)).wrapping_rem(d);

  let mut p: u32 = BITS - 1;
  let mut q1 = MIN_POW / nc;
  let mut r1 = MIN_POW - q1 * nc;
  let mut q2 = (MIN_POW - 1) / d;
  let mut r2 = (MIN_POW - 1) - q2 * d;

  loop {
    p += 1;

    if r1 >= nc - r1 {
      q1 = q1.wrapping_mul(2).wrapping_add(1);
      r1 = r1.wrapping_mul(2).wrapping_sub(nc);
    } else {
      q1 = q1.wrapping_mul(2);
      r1 = r1.wrapping_mul(2);
    }

    if r2.wrapping_add(1) >= d - r2 {
      if q2 >= MIN_POW - 1 {
        add = true;
      }
      q2 = q2.wrapping_mul(2).wrapping_add(1);
      r2 = r2.wrapping_mul(2).wrapping_add(1).wrapping_sub(d);
    } else {
      if q2 >= MIN_POW {
        add = true;
      }
      q2 = q2.wrapping_mul(2);
      r2 = r2.wrapping_mul(2).wrapping_add(1);
    }

    let delta = d - 1 - r2;

    if !(p < 2 * BITS && (q1 < delta || (q1 == delta && r1 == 0))) {
      break;
    }
  }

  UnsignedMagic {
    multiplier: q2.wrapping_add(1),
    add,
    shift: p - BITS,
  }
}

/// `2^63` — the most-significant-bit weight used as the
/// starting numerator in both magic-number searches.
const MIN_POW: u64 = 1u64 << 63;

#[cfg(test)]
mod tests {
  use super::*;

  /// Reference signed quotient via the magic sequence, exactly
  /// as the emitted instructions compute it: `q = SMULH(n, M)`,
  /// optional `± n`, arithmetic shift, then add the sign bit.
  fn signed_via_magic(n: i64, d: i64) -> i64 {
    let m = signed_magic(d);
    let hi = ((n as i128 * m.multiplier as i128) >> 64) as i64;
    let mut q = hi;

    if d > 0 && m.multiplier < 0 {
      q = q.wrapping_add(n);
    } else if d < 0 && m.multiplier > 0 {
      q = q.wrapping_sub(n);
    }

    q >>= m.shift;
    q.wrapping_add((q as u64 >> 63) as i64)
  }

  /// Reference unsigned quotient via the magic sequence.
  fn unsigned_via_magic(n: u64, d: u64) -> u64 {
    let m = unsigned_magic(d);
    let hi = ((n as u128 * m.multiplier as u128) >> 64) as u64;

    if m.add {
      // q = (((n - hi) >> 1) + hi) >> (shift - 1).
      let t = (n.wrapping_sub(hi) >> 1).wrapping_add(hi);
      t >> (m.shift - 1)
    } else {
      hi >> m.shift
    }
  }

  #[test]
  fn signed_matches_hardware_divide() {
    let divisors = [3i64, 7, 10, 100, 1000, 6, -3, -7, -10, 11, 13, 25, 12345];
    let samples = [
      0i64,
      1,
      -1,
      2,
      -2,
      42,
      -42,
      1000,
      -1000,
      i64::MAX,
      i64::MIN + 1,
      123456789,
      -987654321,
    ];

    for &d in &divisors {
      for &n in &samples {
        assert_eq!(signed_via_magic(n, d), n / d, "signed {n} / {d}",);
      }
    }
  }

  #[test]
  fn unsigned_matches_hardware_divide() {
    let divisors = [3u64, 7, 10, 100, 1000, 6, 11, 13, 25, 12345];
    let samples = [
      0u64,
      1,
      2,
      42,
      1000,
      u64::MAX,
      u64::MAX - 1,
      123456789,
      9999999999,
    ];

    for &d in &divisors {
      for &n in &samples {
        assert_eq!(unsigned_via_magic(n, d), n / d, "unsigned {n} / {d}",);
      }
    }
  }

  #[test]
  fn signed_exhaustive_small() {
    for d in [3i64, -3, 7, -7, 10, 6] {
      for n in -2000i64..=2000 {
        assert_eq!(signed_via_magic(n, d), n / d, "{n} / {d}");
      }
    }
  }

  #[test]
  fn unsigned_exhaustive_small() {
    for d in [3u64, 7, 10, 6, 100] {
      for n in 0u64..=4000 {
        assert_eq!(unsigned_via_magic(n, d), n / d, "{n} / {d}");
      }
    }
  }
}
