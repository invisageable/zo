//! Control-flow graph over a function body.
//!
//! The SIR is a flat instruction stream; control flow is
//! explicit (`Label` / `Jump` / `BranchIfNot` / `Return`).
//! This builds the basic-block graph the SCCP solver needs:
//! maximal straight-line runs split at leaders, with
//! successor/predecessor edges.
//!
//! Edges (not nodes) carry the executable flag in SCCP, so
//! `succs` / `preds` are the unit the solver iterates.

use zo_sir::Insn;

use rustc_hash::FxHashMap as HashMap;

/// Index into [`Cfg::blocks`].
pub type BlockId = usize;

/// A maximal run of instructions with a single entry (its
/// first instruction) and a single exit (its terminator).
#[derive(Debug)]
pub struct Block {
  /// First instruction index, global into the SIR stream.
  pub start: usize,
  /// One past the last instruction index (exclusive).
  pub end: usize,
  /// Blocks control can flow to from this block's terminator.
  pub succs: Vec<BlockId>,
  /// Blocks that can flow into this block.
  pub preds: Vec<BlockId>,
}

/// The control-flow graph of one function body.
#[derive(Debug)]
pub struct Cfg {
  pub blocks: Vec<Block>,
  /// Block containing the function's first instruction.
  pub entry: BlockId,
}

impl Cfg {
  /// Build the CFG for the function body `[start, end)`.
  ///
  /// `insns[start]` is the `FunDef`; `end` is one past the
  /// body's last instruction.
  pub fn build(insns: &[Insn], start: usize, end: usize) -> Self {
    // --- leaders: the first instruction of each block ---
    //
    // A leader is: the entry, any `Label`, or any instruction
    // immediately after a terminator (`Jump` / `BranchIfNot` /
    // `Return`).
    let mut is_leader = vec![false; end - start];

    if !is_leader.is_empty() {
      is_leader[0] = true;
    }

    let len = end - start;

    for (local, insn) in insns[start..end].iter().enumerate() {
      match insn {
        Insn::Label { .. } => is_leader[local] = true,
        Insn::Jump { .. } | Insn::BranchIfNot { .. } | Insn::Return { .. }
          if local + 1 < len =>
        {
          is_leader[local + 1] = true;
        }
        _ => {}
      }
    }

    // --- blocks: span each leader to the next leader ---

    let mut blocks: Vec<Block> = Vec::new();
    // `Label id → BlockId` so jump targets resolve to a block.
    let mut label_block: HashMap<u32, BlockId> = HashMap::default();
    let mut block_start: Option<usize> = None;

    for (local, &leader) in is_leader.iter().enumerate() {
      if !leader {
        continue;
      }

      if let Some(bs) = block_start.take() {
        blocks.push(Block {
          start: start + bs,
          end: start + local,
          succs: Vec::new(),
          preds: Vec::new(),
        });
      }

      block_start = Some(local);
    }

    if let Some(bs) = block_start {
      blocks.push(Block {
        start: start + bs,
        end,
        succs: Vec::new(),
        preds: Vec::new(),
      });
    }

    for (bid, block) in blocks.iter().enumerate() {
      if let Insn::Label { id } = &insns[block.start] {
        label_block.insert(*id, bid);
      }
    }

    // --- edges: resolve each block's terminator ---

    for bid in 0..blocks.len() {
      let last = blocks[bid].end - 1;
      let fallthrough = bid + 1;
      let has_fallthrough = fallthrough < blocks.len();

      let succs = match &insns[last] {
        Insn::Jump { target } => {
          label_block.get(target).copied().into_iter().collect()
        }
        Insn::BranchIfNot { target, .. } => {
          let mut s = Vec::new();

          if has_fallthrough {
            s.push(fallthrough);
          }

          if let Some(&t) = label_block.get(target) {
            s.push(t);
          }

          s
        }
        Insn::Return { .. } => Vec::new(),
        _ => {
          if has_fallthrough {
            vec![fallthrough]
          } else {
            Vec::new()
          }
        }
      };

      for &succ in &succs {
        blocks[succ].preds.push(bid);
      }

      blocks[bid].succs = succs;
    }

    Cfg { blocks, entry: 0 }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  use zo_sir::Insn;
  use zo_ty::TyId;
  use zo_value::ValueId;

  fn ret() -> Insn {
    Insn::Return {
      value: None,
      ty_id: TyId(1),
    }
  }

  fn fundef() -> Insn {
    Insn::Nop // stand-in entry marker; CFG only reads control flow
  }

  #[test]
  fn straight_line_is_one_block() {
    let insns = vec![fundef(), Insn::Nop, ret()];
    let cfg = Cfg::build(&insns, 0, 3);

    assert_eq!(cfg.blocks.len(), 1);
    assert!(cfg.blocks[0].succs.is_empty());
  }

  #[test]
  fn if_else_diamond() {
    // 0 fundef
    // 1 branch_if_not -> L_else (id 1)
    // 2 nop            (then)
    // 3 jump -> L_end  (id 2)
    // 4 label 1        (else)
    // 5 nop
    // 6 label 2        (end)
    // 7 return
    let insns = vec![
      fundef(),
      Insn::BranchIfNot {
        cond: ValueId(0),
        target: 1,
      },
      Insn::Nop,
      Insn::Jump { target: 2 },
      Insn::Label { id: 1 },
      Insn::Nop,
      Insn::Label { id: 2 },
      ret(),
    ];

    let cfg = Cfg::build(&insns, 0, insns.len());

    // entry, then, else, end.
    assert_eq!(cfg.blocks.len(), 4);

    // entry branches to then (fallthrough) and else.
    assert_eq!(cfg.blocks[0].succs.len(), 2);
    // both then and else reach the end block.
    let end = cfg.blocks.len() - 1;
    assert_eq!(cfg.blocks[end].preds.len(), 2);
  }

  #[test]
  fn loop_back_edge() {
    // 0 fundef
    // 1 label 1     (head)
    // 2 branch_if_not -> L_exit (id 2)
    // 3 jump -> L_head (id 1)   back-edge
    // 4 label 2     (exit)
    // 5 return
    let insns = vec![
      fundef(),
      Insn::Label { id: 1 },
      Insn::BranchIfNot {
        cond: ValueId(0),
        target: 2,
      },
      Insn::Jump { target: 1 },
      Insn::Label { id: 2 },
      ret(),
    ];

    let cfg = Cfg::build(&insns, 0, insns.len());

    // The head block is a successor of the body (back-edge).
    let head = cfg
      .blocks
      .iter()
      .position(|b| matches!(insns[b.start], Insn::Label { id: 1 }))
      .unwrap();

    assert!(
      cfg.blocks.iter().any(|b| b.succs.contains(&head)),
      "loop head must have an incoming back-edge"
    );
  }
}
