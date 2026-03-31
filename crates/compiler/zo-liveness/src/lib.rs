pub mod bitvec;
pub mod insn;
pub mod liveness;

#[cfg(test)]
mod tests;

pub use bitvec::BitVec;
pub use insn::{compute_value_ids, insn_uses, insn_var_def, insn_var_use};
pub use liveness::{LivenessInfo, analyze};
