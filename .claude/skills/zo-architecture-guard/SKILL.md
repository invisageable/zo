---
name: zo-architecture-guard
description: >
  Validates code changes against zo's Three Prime Directives and architectural
  mandates. Use when reviewing PRs, planning new features, or when user says
  "check architecture", "validate design", "does this follow the rules",
  "architecture review", or "guard check". Do NOT use for simple formatting,
  typo fixes, or non-compiler code changes.
---

# zo Architecture Guard

Validate the current code changes or proposed design against zo's core laws.

## Process

### Step 1: Identify what changed

Read the modified files. If reviewing a proposal (not code), analyze the described approach.

### Step 2: Check against the Three Prime Directives

For each change, answer these questions:

**Law of Velocity**
- Does this add work to the synchronous AOT path (`Parse -> Tree -> SIR -> Codegen`)?
- Does it introduce allocations, locks, or blocking I/O in hot paths?
- Could it degrade throughput below 10M LoC/s for parsing or 5M LoC/s for analysis/codegen?
- VIOLATION if: the change adds latency to the compile-to-run path without clear justification.

**Law of Pragmatism**
- Does this introduce "magic" — hidden complexity, implicit behavior, or framework abstractions?
- Does it add incremental compilation, complex caching, or esoteric IR transformations?
- Does it use theoretical approaches (bidirectional types) over proven ones (Hindley-Milner)?
- Does it hand control to a third-party system instead of owning the stack?
- VIOLATION if: the change adds complexity that isn't justified by measurable performance gain.

**Law of Insight**
- Does Copilord analysis code feed data back into the AOT pipeline?
- Does analysis work block the synchronous compilation path?
- VIOLATION if: the one-way data flow (AOT -> Copilord, never reverse) is broken.

### Step 3: Check architectural mandates

- **Two Pipelines**: Is new code clearly in the AOT path OR the Copilord path? Not both?
- **Parallelism**: Are new data structures `Send + Sync` if shared across threads? Does the wave model hold?
- **Data Sovereignty**: Is the change a clean data transformation? Does SIR remain the single source of typed semantic truth?
- **Execution-Based Compilation**: Does the change preserve single-pass Tree execution into SIR? No tree-walking regressions?

### Step 4: Check data-oriented design

- Stack vs heap: is heap allocation justified?
- Linear access patterns: does the change introduce pointer chasing or random access in hot paths?
- Arena allocation: should this data live in an arena?
- Zero-copy: are there unnecessary clones or copies?

## Output Format

```
## Architecture Guard Report

### Verdict: PASS | WARN | FAIL

### Findings
[For each finding:]
- **Directive**: [which law/mandate violated]
- **Location**: file:line
- **Issue**: [what's wrong]
- **Severity**: BLOCKING | WARNING | NOTE
- **Recommendation**: [how to fix]
```

If no issues found, state PASS with a one-line confirmation.

## Reference

See `references/architecture-quick-ref.md` for the full architectural reference.
