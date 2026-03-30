# zo — tokenizer.

> *The lexical analysis stage of the zo compiler.*

## about.

THE TOKENiZER FOLLOW DATA-ORiENTED DESiGN FOR TOKENiZATiON.

## goals.

THE TOKENiZER MUST BE ABLE TO SCAN `10M LOC/S` iN PROGRAMMiNG AND TEMPLATiNG MODE OR BOTH.

### tests.

tests are separate by concepts:

- `fuzzing` — contains fuzz tests using `cargo fuzz` and `proptest`.
- `snapshots` — contains snapshot tests using `insta`.
  - all snapshots files are located in [`tests/snapshots`](./tests/snapshots/).
- `tokenization` — contains integration tests using `proptest`.
  - all regressions files are located in [`tests/tokenization`](./tests/tokenization/).

## benchmarks.

@RUN: `cargo bench --package zo-tokenizer --bench tokenize`

### benchmarks — results.

**template heavy**

| Test Name                   | Time (µs)       | Throughput              |
| :-------------------------- | :-------------- | :---------------------- |
| `template_heavy_bytes/10`   | 18.505 – 19.033 | 483.02 – 496.82 MiB/s   |
| `template_heavy_lines/10`   | 18.516 – 18.586 | 17.809 – 17.877 Melem/s |
| `template_heavy_bytes/50`   | 90.253 – 90.650 | 508.35 – 510.58 MiB/s   |
| `template_heavy_lines/50`   | 89.836 – 90.441 | 18.255 – 18.378 Melem/s |
| `template_heavy_bytes/100`  | 183.24 – 183.49 | 502.44 – 503.11 MiB/s   |
| `template_heavy_lines/100`  | 183.32 – 184.09 | 17.931 – 18.007 Melem/s |

**mixed code**

| Test Name                   | Time (µs)       | Throughput              |
| :-------------------------- | :-------------- | :---------------------- |
| `mixed_code_bytes/20`       | 10.378 – 10.403 | 463.60 – 464.72 MiB/s   |
| `mixed_code_lines/20`       | 10.492 – 10.612 | 19.601 – 19.824 Melem/s |
| `mixed_code_bytes/100`      | 50.208 – 50.346 | 479.89 – 481.20 MiB/s   |
| `mixed_code_lines/100`      | 50.771 – 50.964 | 20.309 – 20.386 Melem/s |
| `mixed_code_bytes/200`      | 101.80 – 102.51 | 473.13 – 476.42 MiB/s   |
| `mixed_code_lines/200`      | 101.28 – 101.69 | 20.336 – 20.418 Melem/s |

**mode transitions**

| Test Name                   | Time (µs)       | Throughput              |
| :-------------------------- | :-------------- | :---------------------- |
| `mode_transitions_bytes/20` | 14.669 – 14.736 | 305.46 – 306.86 MiB/s   |
| `mode_transitions_lines/20` | 14.649 – 14.727 | 16.364 – 16.451 Melem/s |

### benchmarks — average throughput by category.

BASED ON THE BENCHMARKS, THE TOKENiZER ALREADY ACHiEVE OUR GOAL OF `10M LOC/S`.

| Category            | MiB/s    | Melem/s |
| :------------------ | :------- | :------ |
| `template\_heavy`   | `499.05` | `18.09` |
| `mixed\_code`       | `472.27` | `20.11` |
| `mode\_transitions` | `306.18` | `16.41` |
