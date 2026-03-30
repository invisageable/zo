# zo — liveness.

> *backward bitvector liveness analysis for SIR.*

## about.

  - BiTVECTOR LiVENESS — *fixed-point dataflow on instruction-level CFG.*
  - SHARED ACROSS PASSES — *used by register allocation and dead code elimination.*

## dev.

### overview.

`zo-liveness` provides backward liveness analysis and SIR instruction
introspection. extracted from `zo-register-allocation` so both
register allocation and DCE can share the same analysis.

### modules.

| module | purpose |
|--------|---------|
| `bitvec` | compact bitvector (u64-word, set/test/union/difference) |
| `insn` | `compute_value_ids()` and `insn_uses()` — def/use extraction |
| `liveness` | `analyze()` — backward fixed-point liveness |

### algorithm.

classical backward bitvector dataflow:

  ```
  for each instruction i (backward):
    live_out[i] = ∪ live_in[successors of i]
    live_in[i]  = uses[i] ∪ (live_out[i] \ defs[i])

  repeat until no change (fixed-point).
  ```

CFG successor computation handles:
  - `Jump { target }` → label target
  - `BranchIfNot { target }` → fall-through + label target
  - `Return` → no successors
  - everything else → fall-through to next instruction

### consumers.

  ```
  zo-liveness
    ├── zo-register-allocation — spill only live values
    └── zo-dce                 — eliminate dead variables/instructions
  ```
