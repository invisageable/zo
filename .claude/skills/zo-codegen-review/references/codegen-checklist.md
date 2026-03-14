# Codegen Review Checklist

## AArch64 Specific

### Registers
- x0-x7: argument/result registers
- x8: indirect result location
- x9-x15: temporary registers (caller-saved)
- x16-x17: intra-procedure-call scratch (avoid)
- x18: platform register (don't use on macOS)
- x19-x28: callee-saved registers
- x29 (FP): frame pointer
- x30 (LR): link register
- SP: stack pointer (16-byte aligned)

### Common Mistakes
- Forgetting to save/restore callee-saved registers
- Stack misalignment (must be 16-byte on AArch64)
- Using x18 on macOS (reserved by OS)
- Not handling large immediates (>12-bit for add/sub, >16-bit for mov)
- Wrong condition codes after comparisons
- Missing sign/zero extension for narrower-than-register values

### Instruction Selection Quality
- `add x0, x0, #1` not `mov x1, #1; add x0, x0, x1`
- `lsl x0, x0, #2` not `mov x1, #4; mul x0, x0, x1`
- `cbz x0, label` not `cmp x0, #0; b.eq label`
- `madd x0, x1, x2, x3` for `x1*x2+x3` when possible
- Fused compare-and-branch where applicable

### Calling Convention (macOS AArch64)
- Arguments: x0-x7 (integer), d0-d7 (float)
- Return: x0 (integer), d0 (float)
- Caller-saved: x0-x15, d0-d7, d16-d31
- Callee-saved: x19-x28, d8-d15
- Frame pointer (x29) must be maintained
- LR (x30) saved on entry if function makes calls

## SIR Quality Checks

- All types resolved (no `Unknown` types in final SIR)
- Constants folded where possible
- Dead code eliminated
- No redundant type conversions
- Control flow graph is reducible
