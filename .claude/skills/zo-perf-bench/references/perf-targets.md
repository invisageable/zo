# Performance Targets — Detailed Breakdown

## Why These Numbers

- **10M LoC/s** for tokenize+parse: A 100K LoC project compiles in 10ms. A 1M LoC project in 100ms. Instant feedback.
- **5M LoC/s** for analysis: Half the parse speed is acceptable — type resolution is inherently more work. Still sub-200ms for 1M LoC.
- **5M LoC/s** for codegen: Matching analysis speed. Machine code emission should not be the bottleneck.

## Comparable Systems

| Compiler | Parse | Analysis | Notes |
|----------|-------|----------|-------|
| Carbon | ~8M LoC/s | ~1M LoC/s | Chandler Carruth's execution model |
| Zig | ~2M LoC/s | unknown | Self-hosted, single-threaded |
| Go | ~1M LoC/s | ~1M LoC/s | Simple type system helps |
| Rust (rustc) | ~200K LoC/s | ~50K LoC/s | Complex type system, incremental |

## Key Performance Principles

1. **No allocations in hot paths.** Tokenizer and parser must be zero-alloc.
2. **Linear memory access.** Process arrays sequentially. No pointer chasing.
3. **Arena allocation for IR nodes.** Tree and SIR nodes live in arenas.
4. **Brute-force parallelism.** Files parsed independently. Functions analyzed independently.
5. **No incremental compilation.** From-scratch is faster when the base speed is high enough.
6. **Streaming SIR emission.** Don't wait for complete type info. Emit as resolved.

## How to Measure

- Use `criterion` or `divan` for microbenchmarks.
- Use `hyperfine` for end-to-end wall clock.
- Measure with `--release` profile only.
- Use representative input sizes (10K, 100K, 1M LoC).
- Report both throughput (LoC/s) and latency (ms for N LoC).
