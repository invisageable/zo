---
name: zo-build-check
description: >
  Runs the zo build pipeline (cargo build, clippy, tests) and analyzes
  failures with targeted fix suggestions. Use when user says "build",
  "check", "does it compile", "run clippy", "run tests", "ci check",
  or after making code changes. Do NOT use for non-Rust files or
  documentation-only changes.
---

# zo Build Check

Run the full build pipeline and analyze results.

## Workflow

### Step 1: Run the pipeline

Execute in order (stop on first failure). Always use `just` recipes:

```bash
just typos
just fmt_check
just clippy
just test
```

Or run all at once:
```bash
just pre-commit
```

IMPORTANT: Always use `just` recipes. Do not run raw cargo commands — the justfile is the single source of truth for build commands.

> Get the full list of `just` commands; @justfile

### Step 2: Analyze failures

For each failure:

**Typos**
- Show the typo and file location.
- Suggest the correction.
- Run `typos --write-changes` if user approves.

**Format**
- Run `cargo fmt --all` to fix (no `--check`).
- Report which files were reformatted.

**Clippy**
- Parse the warning/error output.
- Group by category (unused, borrowing, performance, correctness).
- For each: file:line, the lint name, and a concrete fix.
- Prioritize correctness and performance lints over style lints.

**Tests**
- Identify which tests failed.
- Show the failure message and expected vs actual.
- Read the failing test source to understand intent.
- Suggest a fix for the code (not the test) unless the test itself is wrong.

### Step 3: Report

```
## Build Check Report

### Status: PASS | FAIL (stage: [which stage failed])

### Issues Found
[For each issue:]
- **Stage**: typos | fmt | clippy | test
- **Location**: file:line
- **Issue**: [description]
- **Fix**: [concrete action]

### Summary
- Typos: X found
- Format: X files need formatting
- Clippy: X warnings, Y errors
- Tests: X passed, Y failed
```

### Step 4: Auto-fix (if user requests)

If user says "fix it" or "auto-fix":
1. Run `just typos_fix`
2. Run `just fmt`
3. Apply clippy suggestions where `--fix` is safe
4. Re-run `just pre-commit` to verify fixes
