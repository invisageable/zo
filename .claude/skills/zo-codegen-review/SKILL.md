---
name: zo-codegen-review
description: >
  Reviews SIR-to-machine-code generation output for correctness and
  efficiency. Use when user says "review codegen", "check generated code",
  "codegen quality", "is the output correct", "review emitter output",
  or "arm output review". Do NOT use for source-level code review
  (use /blame instead) or performance benchmarking (use zo-perf-bench).
---

# zo Codegen Review

Review generated machine code quality from the SIR codegen pipeline.

## Codegen Crates

```
zo-codegen/           # Main codegen orchestration
zo-codegen-backend/   # Backend trait and shared infrastructure
zo-codegen-arm/       # AArch64 code generation
zo-emitter/           # Instruction emission
zo-emitter-arm/       # AArch64 instruction encoding
zo-writer-macho/      # Mach-O binary format writer
```

## Workflow

### Step 1: Identify scope

Ask or infer what to review:
- A specific function's generated output
- A specific SIR pattern (e.g., how loops are lowered)
- Overall codegen quality for a test file
- A specific backend (ARM, future WASM, etc.)

### Step 2: Trace the pipeline

For the target code, follow:
```
Source (.zo) -> Tokens -> Tree -> SIR -> Machine Code
```

1. **Read the source** — Understand the intent.
2. **Read the SIR** — Check that semantic analysis produced correct typed IR.
3. **Read the codegen output** — Check the generated instructions.

### Step 3: Check correctness

For each generated function:

- **Instruction selection** — Are the right instructions chosen for each SIR operation?
- **Register allocation** — Are registers used efficiently? Unnecessary spills?
- **Calling convention** — Do function calls follow the platform ABI?
- **Stack frame** — Is the frame sized correctly? Are locals at correct offsets?
- **Control flow** — Are branches correct? Fallthrough optimized?
- **Constants** — Are immediates encoded correctly? Large constants handled?

### Step 4: Check efficiency

- **Redundant instructions** — mov to same register, unnecessary loads after stores, dead stores.
- **Missed optimizations** — Strength reduction opportunities (mul by power of 2 -> shift), constant folding that should have happened in SIR.
- **Register pressure** — Excessive spilling that could be avoided with better allocation.
- **Code size** — Unnecessarily long instruction sequences for simple operations.
- **Alignment** — Functions and loops properly aligned for the target.

### Step 5: Check binary format

If reviewing the full output:
- **Mach-O structure** — Correct headers, segments, sections.
- **Relocations** — All references resolved correctly.
- **Symbol table** — Exports and imports correct.

### Step 6: Report

```
## Codegen Review

### Target: [function/file being reviewed]
### Backend: [arm/wasm/etc.]

### Correctness
[For each issue:]
- **Location**: SIR node / instruction offset
- **Issue**: [what's wrong]
- **Expected**: [correct output]
- **Actual**: [current output]

### Efficiency
[For each opportunity:]
- **Location**: instruction offset
- **Current**: [instruction sequence]
- **Optimized**: [better sequence]
- **Impact**: [estimated improvement]

### Verdict: CORRECT | INCORRECT | CORRECT-BUT-INEFFICIENT
```

## Reference

See `references/codegen-checklist.md` for the full review checklist.
