---
name: zo-test-runner
description: >
  Runs zo's test suite and intelligently analyzes failures. Reads failing
  test sources, traces the failure to compiler code, and suggests fixes.
  Use when user says "run tests", "what's failing", "test analysis",
  "fix failing tests", or "why is this test broken". Do NOT use for
  benchmarks or performance testing (use zo-perf-bench instead).
---

# zo Test Runner

Run tests and analyze failures with context.

## Test Suite Structure

```
crates/compiler/zo-tests/
├── run-pass/      # Must compile AND run successfully
├── build-pass/    # Must compile successfully
├── build-fail/    # Must FAIL to compile (tests error reporting)
└── run-fail/      # Must compile but FAIL at runtime
```

## Workflow

### Step 1: Run tests

```bash
just test_all
```

If user specifies a scope, narrow it:
- Single crate: `just test_crate zo-parser`
- Single test: `just test_filter test_name`
- Test category: run only `.zo` files from the relevant `zo-tests/` subdirectory

IMPORTANT: Always use `just` recipes. The justfile is the single source of truth for build commands.

### Step 2: Parse results

For each failure, extract:
- Test name and crate
- Failure type (panic, assertion, compile error, timeout)
- Error message and backtrace (if available)
- Expected vs actual output

### Step 3: Trace to source

For each failing test:

1. **Read the test source.** Understand what it's testing.
2. **Identify the compiler phase.** Is this a tokenizer, parser, analyzer, codegen, or runtime issue?
3. **Read the relevant compiler code.** Follow the error to the source — don't guess.
4. **Check recent changes.** Use `git diff` and `git log` to see if a recent change caused the regression.

### Step 4: Classify failures

Group failures by root cause:

- **Regression** — Previously passing test broke. Check git history.
- **Incomplete implementation** — Test expects a feature not yet built.
- **Wrong expectation** — Test itself has incorrect expected output.
- **Environment** — Flaky test, platform-specific, or dependency issue.

### Step 5: Report

```
## Test Report

### Summary
- Total: X tests
- Passed: X
- Failed: X
- Skipped: X

### Failures

[For each failure:]
#### test_name (crate)
- **Category**: regression | incomplete | wrong-expectation | environment
- **Phase**: tokenizer | parser | analyzer | codegen | runtime
- **Error**: [concise error message]
- **Root cause**: [what's actually wrong, with file:line]
- **Fix**: [concrete suggestion — fix the code, not the test, unless the test is wrong]
```

### Step 6: Fix (if requested)

If user says "fix it":
1. Apply fixes in priority order (regressions first).
2. Re-run only the previously failing tests to verify.
3. Run the full suite to check for new regressions.
