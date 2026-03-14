# Skills Testing Protocol

How to validate that each skill triggers correctly, works correctly, and improves outcomes.

---

## zo-architecture-guard

### Triggering — SHOULD activate
1. "check architecture of this change"
2. "does this follow the rules"
3. "validate design"
4. "guard check"
5. "architecture review on the parser refactor"

### Triggering — should NOT activate
1. "fix this typo in the README"
2. "format the code"
3. "what does this function do"
4. "run the tests"
5. "help me write a benchmark"

### Functional tests
1. **Detects AOT path violation** — Add a `tokio::fs::read` call inside the tokenizer. Guard should flag it as blocking I/O in the AOT path (Law of Velocity).
2. **Detects Copilord feedback loop** — Add code where Copilord writes to a SIR node. Guard should flag reverse data flow (Law of Insight).
3. **Passes clean change** — Add a new arithmetic operator to `zo-constant-folding`. Guard should report PASS.

---

## zo-build-check

### Triggering — SHOULD activate
1. "build"
2. "does it compile"
3. "check"
4. "run clippy"
5. "ci check"

### Triggering — should NOT activate
1. "review the architecture"
2. "how fast is the tokenizer"
3. "explain this code"
4. "write a new parser rule"
5. "what changed in the last commit"

### Functional tests
1. **Catches typo** — Introduce a typo in a doc comment. Build check should report it at the typos stage.
2. **Catches clippy warning** — Add an unnecessary `.clone()`. Build check should report it with the lint name and fix.
3. **Full pipeline pass** — Run on a clean working tree. Should report PASS across all stages.

---

## zo-perf-bench

### Triggering — SHOULD activate
1. "benchmark"
2. "how fast is the tokenizer"
3. "perf"
4. "are we hitting targets"
5. "LoC/s"

### Triggering — should NOT activate
1. "run the tests"
2. "build the project"
3. "review this code"
4. "check architecture"
5. "fix this bug"

### Functional tests
1. **Reports existing benchmarks** — Run on `zo-codegen-arm` (has `benches/`). Should find and report benchmark results.
2. **Detects missing benchmarks** — Run on a crate without `benches/`. Should suggest creating one with the template.
3. **Compares against targets** — Should display the target table and calculate % of target achieved.

---

## zo-test-runner

### Triggering — SHOULD activate
1. "run tests"
2. "what's failing"
3. "test analysis"
4. "fix failing tests"
5. "why is this test broken"

### Triggering — should NOT activate
1. "benchmark the parser"
2. "review the codegen output"
3. "check architecture"
4. "format the code"
5. "build the project"

### Functional tests
1. **Classifies regression** — Break a passing test by modifying compiler code. Runner should identify it as a regression and point to the recent change.
2. **Reads test source** — On a failure, should read the `.zo` test file and the relevant compiler code, not just dump the error.
3. **Scoped run** — `just test_crate zo-parser` should run only parser tests.

---

## zo-codegen-review

### Triggering — SHOULD activate
1. "review codegen"
2. "check generated code"
3. "codegen quality"
4. "is the arm output correct"
5. "review emitter output"

### Triggering — should NOT activate
1. "review this Rust code" (source-level review)
2. "run the tests"
3. "benchmark the codegen"
4. "check architecture"
5. "fix this parser bug"

### Functional tests
1. **Spots redundant instruction** — Feed a function with `mov x0, x0`. Should flag as redundant.
2. **Checks calling convention** — Review a function that clobbers a callee-saved register without saving it. Should flag.
3. **Passes correct output** — Review correctly generated code for a simple function. Should report CORRECT.

---

## How to Run Tests

### Manual testing (recommended for iteration)

For each skill, pick 2-3 queries from the "SHOULD activate" list and 2-3 from "should NOT activate." Run them in Claude Code and verify:

- [ ] Skill loads when expected
- [ ] Skill does NOT load when not expected
- [ ] Output follows the documented format
- [ ] Findings are accurate (not hallucinated)

### Validation query

Ask Claude: "When would you use the [skill-name] skill?"

Claude will quote the description back. If the answer doesn't match your intent, revise the description field in the skill's frontmatter.

### Performance baseline

For each skill, note on first use:
- Number of tool calls to complete the workflow
- Whether user correction was needed
- Whether the output format matched the spec

Compare after any skill revision.
