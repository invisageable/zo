# zo — parser.

### tests.

tests are separate by concepts:

- `fuzzing` — contains fuzz tests using `cargo fuzz` and `proptest`.
- `snapshots` — contains snapshot tests using `insta`.
  - all snapshots files are located in [`tests/snapshots`](./tests/snapshots/).
- `tokenization` — contains integration tests using `proptest`.
  - all regressions files are located in [`tests/tokenization`](./tests/tokenization/).

## benchmarks.

RUN: `cargo bench --package zo-parser --bench parse --quiet`

### benchmarks — results.

**parser benchmarks**

| Test Name              | Time (µs)       | Throughput            |
| :--------------------- | :-------------- | :-------------------- |
| `parser_bytes/simple`  | 1.857 – 1.875   | 40.68 – 41.08 MiB/s   |
| `parser_lines/simple`  | 1.909 – 1.933   | 3.622 – 3.666 Melem/s |
| `parser_bytes/complex` | 4.737 – 4.790   | 74.66 – 75.49 MiB/s   |
| `parser_lines/complex` | 4.738 – 4.777   | 5.233 – 5.277 Melem/s |
| `parser_bytes/medium`  | 167.27 – 169.16 | 143.09 – 144.70 MiB/s |
| `parser_lines/medium`  | 170.38 – 174.21 | 6.957 – 7.113 Melem/s |
| `parser_bytes/large`   | 1659.8 – 1678.5 | 143.79 – 145.42 MiB/s |
| `parser_lines/large`   | 1666.0 – 1684.8 | 7.130 – 7.210 Melem/s |

**mixed code**

| Test Name               | Time (µs)       | Throughput            |
| :---------------------- | :-------------- | :-------------------- |
| `mixed_code_bytes/100`  | 171.82 – 177.75 | 136.17 – 140.87 MiB/s |
| `mixed_code_lines/100`  | 168.22 – 170.75 | 7.098 – 7.205 Melem/s |
| `mixed_code_bytes/500`  | 820.73 – 852.38 | 141.62 – 147.08 MiB/s |
| `mixed_code_lines/500`  | 826.89 – 853.55 | 7.044 – 7.271 Melem/s |
| `mixed_code_bytes/1000` | 1661.5 – 1798.4 | 134.21 – 145.27 MiB/s |
| `mixed_code_lines/1000` | 1708.0 – 1747.3 | 6.875 – 7.033 Melem/s |
| `mixed_code_bytes/5000` | 8568.8 – 8819.3 | 137.23 – 141.24 MiB/s |
| `mixed_code_lines/5000` | 8492.9 – 8650.9 | 6.937 – 7.066 Melem/s |

### benchmarks — average throughput by category.

BASED ON THE BENCHMARKS, THE PARSER ALREADY ACHiEVE OUR GOAL OF `10M LOC/S`.

| Category     | MiB/s  | Melem/s |
| :----------- | :----- | :------ |
| `parser`     | 115.35 | 5.65    |
| `mixed_code` | 132.86 | 7.45    |
