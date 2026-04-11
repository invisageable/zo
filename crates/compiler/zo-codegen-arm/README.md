# zo — codegen (arm).

> *...*

## about.

...

### tests.

tests are separate by concepts:

- `fuzzing` — contains fuzz tests using `cargo fuzz` and `proptest`.
- `snapshots` — contains snapshot tests using `insta`.
  - all snapshots files are located in [`tests/snapshots`](./tests/snapshots/).
- `generation` — contains integration tests using `proptest`.
  - all regressions files are located in [`tests/generation`](./tests/generation/).

## benchmarks.

  - @RUN: `cargo bench --package zo-codegen-arm --bench parse`

### benchmarks — results.
