# zo — codegen arm.

### tests.

tests are separate by concepts:

- `fuzzing` — contains fuzz tests using `cargo fuzz` and `proptest`.
- `snapshots` — contains snapshot tests using `insta`.
  - all snapshots files are located in [`tests/snapshots`](./tests/snapshots/).
- `tokenization` — contains integration tests using `proptest`.
  - all regressions files are located in [`tests/tokenization`](./tests/tokenization/).

## benchmarks.

RUN: `cargo bench --package zo-parser --bench parse`

### benchmarks — results.
