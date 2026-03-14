---
name: zo-perf-bench
description: >
  Benchmarks zo compiler phases against performance targets (10M LoC/s
  parse, 5M LoC/s analyze, 5M LoC/s codegen). Use when user says
  "benchmark", "perf", "how fast", "performance test", "speed check",
  "are we hitting targets", or "LoC/s". Do NOT use for general profiling
  of non-compiler code.
---

# zo Performance Benchmark

Measure compiler phase throughput against targets.

## Targets

| Phase | Target | Unit |
|-------|--------|------|
| Tokenize + Parse (-> Tree) | 10,000,000 | LoC/s |
| Semantic Analysis (Tree -> SIR) | 5,000,000 | LoC/s |
| Codegen (SIR -> machine code) | 5,000,000 | LoC/s |

Reference: Carbon achieves ~8M/1M. zo's simpler design should beat that.

## Workflow

### Step 1: Discover benchmarks

Look for existing benchmarks:

```bash
find . -name "*.rs" -path "*/bench*" | head -20
```

Check `Cargo.toml` files for `[[bench]]` sections and benchmark dependencies (criterion, divan, etc.).

### Step 2: Run benchmarks

If benchmarks exist:
```bash
just bench
```

If no benchmarks exist, report that and suggest creating them (see Step 5).

IMPORTANT: Always use `just` recipes. The justfile is the single source of truth for build commands.

### Step 3: Analyze results

For each benchmark result:
- Extract throughput (operations/sec or LoC/s).
- Compare against the target for that phase.
- Calculate percentage of target achieved.
- Flag regressions if previous results are available.

### Step 4: Report

```
## Performance Report

### Phase Results
| Phase | Measured | Target | % of Target | Status |
|-------|----------|--------|-------------|--------|
| Tokenize | X LoC/s | 10M LoC/s | X% | [ON TRACK | BEHIND | AHEAD] |
| Parse | X LoC/s | 10M LoC/s | X% | ... |
| Analysis | X LoC/s | 5M LoC/s | X% | ... |
| Codegen | X LoC/s | 5M LoC/s | X% | ... |

### Bottlenecks
[If any phase is significantly behind target:]
- Phase: [which]
- Current: X LoC/s (Y% of target)
- Likely cause: [analysis based on code review]
- Suggestion: [concrete optimization]

### Trend
[If historical data available:]
- Previous: X LoC/s
- Current: Y LoC/s
- Delta: +/-Z%
```

### Step 5: Missing benchmarks

If benchmarks don't exist for a phase, suggest a minimal benchmark structure:

```rust
// benches/<phase>_bench.rs
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_<phase>(c: &mut Criterion) {
  let input = include_str!("../testdata/large_input.zo");
  c.bench_function("<phase>", |b| {
    b.iter(|| {
      // phase-specific code
    });
  });
}

criterion_group!(benches, bench_<phase>);
criterion_main!(benches);
```

## Reference

See `references/perf-targets.md` for detailed target breakdowns.
