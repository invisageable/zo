pub mod bitvec;
pub mod cfg;
pub mod defuse;
pub mod insn;
pub mod liveness;

#[cfg(test)]
mod tests;

pub use bitvec::BitVec;
pub use cfg::{Block, BlockId, Cfg};
pub use defuse::DefSites;
pub use insn::{
  compute_value_ids, insn_def, insn_var_def, insn_var_use, visit_uses,
};
pub use liveness::{LivenessInfo, analyze};
