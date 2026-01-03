# zo — writer mach-o.

### tests.

tests are separate by concepts:

- `fuzzing` — contains fuzz tests using `cargo fuzz` and `proptest`.
- `snapshots` — contains snapshot tests using `insta`.
  - all snapshots files are located in [`tests/snapshots`](./tests/snapshots/).
- `writing` — contains integration tests using `proptest`.
  - all regressions files are located in [`tests/writing`](./tests/writing/).

## benchmarks.

RUN: `cargo bench --package zo-writer-macho --bench write --quiet`

### benchmarks — results.

**binary generation & manipulation benchmarks**

| Test Name                      | Time (µs)       | Change                        |
| :----------------------------- | :-------------- | :---------------------------- |
| `small_binary_generation`      | 18.723 – 18.804 | +1.47% – +3.50% (regression)  |
| `medium_binary_with_symbols`   | 34.292 – 34.443 | −0.11% – +0.86% (no change)   |
| `large_code_section_1mb`       | 1376.5 – 1412.6 | −5.54% – −0.60% (minor drop)  |
| `relocations_1000`             | 121.82 – 123.32 | −1.65% – +1.65% (no change)   |
| `complex_binary_full_featured` | 35.487 – 35.974 | −0.83% – +0.32% (no change)   |
| `code_signature_generation`    | 159.18 – 160.99 | +4.28% – +11.38% (regression) |
| `realistic_compiler_output`    | 224.25 – 228.01 | +3.23% – +25.31% (regression) |
