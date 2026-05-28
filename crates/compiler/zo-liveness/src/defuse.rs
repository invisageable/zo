//! Def-site index — `ValueId → defining instruction`.
//!
//! A flat `Vec` keyed by `ValueId.0` — a single array load per
//! query, no hashing.
//!
//! @note — correct only over a **single value space**, where
//! every `ValueId` has one definition (one module's SIR before
//! merging, or a single function body). Merged-module SIR
//! reuses `ValueId`s across functions — each module numbers its
//! values independently — so a whole-program `DefSites` would
//! misroute lookups. Callers spanning the merged stream must
//! scope the index per function.

use crate::insn::insn_def;

use zo_sir::Insn;
use zo_value::ValueId;

/// Maps each `ValueId` to the index of the instruction that
/// defines it.
pub struct DefSites {
  /// `site[vid]` = defining instruction index, or `u32::MAX`
  /// when no instruction defines `vid`.
  site: Vec<u32>,
}

impl DefSites {
  /// Build the index in one linear pass. `num_values` is
  /// `Sir::next_value_id` — the upper bound on `ValueId.0`.
  pub fn build(insns: &[Insn], num_values: u32) -> Self {
    let mut site = vec![u32::MAX; num_values as usize];

    for (i, insn) in insns.iter().enumerate() {
      if let Some(dst) = insn_def(insn) {
        site[dst.0 as usize] = i as u32;
      }
    }

    Self { site }
  }

  /// Index of the instruction defining `vid`, or `None`.
  #[inline]
  pub fn of(&self, vid: ValueId) -> Option<usize> {
    let s = self.site[vid.0 as usize];

    (s != u32::MAX).then_some(s as usize)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  use zo_ty::TyId;

  #[test]
  fn maps_dst_to_defining_index() {
    let insns = vec![
      Insn::ConstInt {
        dst: ValueId(0),
        value: 42,
        ty_id: TyId(9),
      },
      Insn::ConstInt {
        dst: ValueId(1),
        value: 7,
        ty_id: TyId(9),
      },
    ];

    let defs = DefSites::build(&insns, 2);

    assert_eq!(defs.of(ValueId(0)), Some(0));
    assert_eq!(defs.of(ValueId(1)), Some(1));
  }

  #[test]
  fn absent_value_returns_none() {
    let insns = vec![Insn::ConstInt {
      dst: ValueId(0),
      value: 1,
      ty_id: TyId(9),
    }];

    let defs = DefSites::build(&insns, 3);

    assert_eq!(defs.of(ValueId(1)), None);
    assert_eq!(defs.of(ValueId(2)), None);
  }
}
