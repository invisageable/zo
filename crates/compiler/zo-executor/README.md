# zo — executor.

### tests.

tests are separate by concepts:

- `fuzzing` — contains fuzz tests using `cargo fuzz` and `proptest`.
- `snapshots` — contains snapshot tests using `insta`.
  - all snapshots files are located in [`tests/snapshots`](./tests/snapshots/).
- `tokenization` — contains integration tests using `proptest`.
  - all regressions files are located in [`tests/tokenization`](./tests/tokenization/).

## benchmarks.

RUN: `cargo bench --package zo-executor --bench execute`

### benchmarks — results.

**executor benchmarks**

| Test Name              | Time (µs)     | Throughput            |
| :--------------------- | :------------ | :-------------------- |
| `executor_bytes/hello` | 2.160 – 2.206 | 20.75 – 21.19 MiB/s   |
| `executor_lines/hello` | 2.197 – 2.221 | 1.801 – 1.820 Melem/s |

### benchmarks — average throughput by category.

| Category   | MiB/s | Melem/s |
| :--------- | :---- | :------ |
| `executor` | 20.97 | 1.811   |

**summary**

The executor benchmarks demonstrate solid performance for small programs, with throughput around **21 MiB/s** and nearly **1.82 M lines/s** for the `hello` program. While not targeting high throughput in this microbenchmark, these numbers suggest minimal overhead for simple executions.
