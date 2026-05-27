# crates — compiler — benches.

> *A series of benches to compare compile-time between c, rust, odin and zo.*

## about.

iT'S REALLY ABOUT CHALLENGiNG. zo DOESN'T CLAiM TO BE A DiRECT COMPETiTOR TO c, rust OR odin. THE NUMBER OF OPTiMiSATiON AND THE QUALiTY OF THESE COMPiLERS ARE A FAR CRY FROM MY NAiVE ViSiON OF BUiLDiNG COMPiLERS. SO YES, CURRENTLY COMPiLiNG THE FAMOUS `hello, world!` iS 8-28X FASTER THAN iN c, rust OR odin, BUT KEEP iN MiND THAT THE PiPELiNES ARE COMPLETELY DiFFERENT. iT'S BEST NOT TO POP THE CHAMPAGNE TO CELEBRATE ANYTHiNG YET. THERE iS SO MUCH STUFF TO DO. BUT THESE BENCHES GiVE US HOPE, AND THAT'S ALL THAT MATTERS TO STAY MOTiVATED.

> *« Compile everything, every time, instantly. »*
> &emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;— i10e

## dev.

iNSTALL ALL PROGRAMMiNG LANGUAGES REQUiRED:

  - @SEE — [clang](https://clang.llvm.org/get_started.html) OR [gcc](https://gcc.gnu.org/install)
  - @SEE — [go](https://go.dev/dl)
  - @SEE — [oding](https://odin-lang.org/docs/install)
  - @SEE — [rust](https://rust-lang.org/tools/install)

## benchmark.

> *Measurements: `arm64-apple-darwin`. odin nightly `dev-2026-05`. 5 runs per bench.*

### benchmark — results.

#### ackermann.

@RUN: `just zo_bench ackermann`

| Compiler | Run 1    | Run 2   | Run 3   | Run 4   | Run 5   | Average     | Speed vs zo      |
| :------- | :------- | :------ | :------ | :------ | :------ | :---------- | :--------------- |
| **zo**   | 7.59ms   | 6.43ms  | 6.04ms  | 5.89ms  | 5.62ms  | **6.32ms**  | **1.0x**         |
| clang    | 66.67ms  | 41.95ms | 38.26ms | 40.30ms | 36.71ms | **44.78ms** | **7.1x slower**  |
| rustc    | 78.54ms  | 67.90ms | 66.90ms | 62.96ms | 63.91ms | **68.04ms** | **10.8x slower** |
| odin     | 82.21ms  | 69.02ms | 69.69ms | 68.28ms | 67.13ms | **71.27ms** | **11.3x slower** |

**-AWTY—are-we-tiny-yet?**

| Compiler | Size     |
| :------- | :------- |
| **zo**   | 32.2 KB  |
| clang    | 32.7 KB  |
| odin     | 71.7 KB  |
| rustc    | 453.3 KB |

#### arithmetic.

@RUN: `just zo_bench arithmetic`

| Compiler | Run 1    | Run 2    | Run 3    | Run 4    | Run 5    | Average      | Speed vs zo      |
| :------- | :------- | :------- | :------- | :------- | :------- | :----------- | :--------------- |
| **zo**   | 8.06ms   | 7.16ms   | 6.17ms   | 6.21ms   | 5.82ms   | **6.68ms**   | **1.0x**         |
| clang    | 56.41ms  | 47.04ms  | 45.73ms  | 42.27ms  | 40.70ms  | **46.43ms**  | **7.0x slower**  |
| rustc    | 70.99ms  | 62.33ms  | 60.93ms  | 61.11ms  | 61.72ms  | **63.42ms**  | **9.5x slower**  |
| odin     | 162.77ms | 174.88ms | 154.65ms | 147.32ms | 149.02ms | **157.73ms** | **23.6x slower** |

**-AWTY—are-we-tiny-yet?**

| Compiler | Size     |
| :------- | :------- |
| **zo**   | 32.2 KB  |
| clang    | 32.7 KB  |
| odin     | 319.7 KB |
| rustc    | 456.3 KB |

#### fibonacci.

@RUN: `just zo_bench fibonacci`

| Compiler | Run 1    | Run 2    | Run 3    | Run 4    | Run 5    | Average      | Speed vs zo      |
| :------- | :------- | :------- | :------- | :------- | :------- | :----------- | :--------------- |
| **zo**   | 6.99ms   | 5.79ms   | 5.55ms   | 5.07ms   | 5.06ms   | **5.69ms**   | **1.0x**         |
| clang    | 56.80ms  | 45.23ms  | 43.49ms  | 40.42ms  | 38.95ms  | **44.98ms**  | **7.9x slower**  |
| rustc    | 78.61ms  | 68.84ms  | 66.22ms  | 62.21ms  | 62.53ms  | **67.68ms**  | **11.9x slower** |
| odin     | 171.06ms | 168.63ms | 159.61ms | 146.89ms | 150.68ms | **159.37ms** | **28.0x slower** |

**-AWTY—are-we-tiny-yet?**

| Compiler | Size     |
| :------- | :------- |
| **zo**   | 32.2 KB  |
| clang    | 32.7 KB  |
| odin     | 319.7 KB |
| rustc    | 457.5 KB |

#### hello.

@RUN: `just zo_bench hello`

| Compiler | Run 1    | Run 2    | Run 3    | Run 4    | Run 5    | Average      | Speed vs zo      |
| :------- | :------- | :------- | :------- | :------- | :------- | :----------- | :--------------- |
| **zo**   | 6.39ms   | 5.75ms   | 5.66ms   | 4.90ms   | 5.11ms   | **5.56ms**   | **1.0x**         |
| clang    | 53.08ms  | 41.28ms  | 42.00ms  | 42.99ms  | 42.01ms  | **44.27ms**  | **8.0x slower**  |
| rustc    | 75.72ms  | 62.08ms  | 61.88ms  | 76.28ms  | 59.24ms  | **67.04ms**  | **12.1x slower** |
| odin     | 153.82ms | 151.34ms | 159.92ms | 166.96ms | 152.00ms | **156.81ms** | **28.2x slower** |

**-AWTY—are-we-tiny-yet?**

| Compiler | Size     |
| :------- | :------- |
| **zo**   | 32.2 KB  |
| clang    | 32.6 KB  |
| odin     | 319.6 KB |
| rustc    | 455.5 KB |

#### munchhausen numbers.

@RUN: `just zo_bench munchhausen`

| Compiler | Run 1    | Run 2    | Run 3    | Run 4    | Run 5    | Average      | Speed vs zo      |
| :------- | :------- | :------- | :------- | :------- | :------- | :----------- | :--------------- |
| **zo**   | 20.22ms  | 4.78ms   | 4.31ms   | 4.52ms   | 4.93ms   | **7.75ms**   | **1.0x**         |
| clang    | 56.26ms  | 44.06ms  | 42.37ms  | 39.05ms  | 41.75ms  | **44.70ms**  | **5.8x slower**  |
| rustc    | 87.94ms  | 70.30ms  | 68.92ms  | 66.66ms  | 65.60ms  | **71.88ms**  | **9.3x slower**  |
| odin     | 280.14ms | 155.56ms | 151.32ms | 147.46ms | 156.76ms | **178.25ms** | **23.0x slower** |

**-AWTY—are-we-tiny-yet?**

| Compiler | Size     |
| :------- | :------- |
| **zo**   | 32.3 KB  |
| clang    | 32.9 KB  |
| odin     | 319.8 KB |
| rustc    | 458.3 KB |

#### rule 110 cellular automaton.

@RUN: `just zo_bench rule_110`

| Compiler | Run 1    | Run 2    | Run 3    | Run 4    | Run 5    | Average      | Speed vs zo      |
| :------- | :------- | :------- | :------- | :------- | :------- | :----------- | :--------------- |
| **zo**   | 6.66ms   | 6.53ms   | 6.30ms   | 6.49ms   | 5.86ms   | **6.37ms**   | **1.0x**         |
| clang    | 52.13ms  | 39.25ms  | 39.92ms  | 38.67ms  | 41.29ms  | **42.25ms**  | **6.6x slower**  |
| rustc    | 183.90ms | 71.22ms  | 70.09ms  | 70.25ms  | 68.95ms  | **92.88ms**  | **14.6x slower** |
| odin     | 279.17ms | 155.46ms | 153.92ms | 155.31ms | 157.59ms | **180.29ms** | **28.3x slower** |

**-AWTY—are-we-tiny-yet?**

| Compiler | Size     |
| :------- | :------- |
| **zo**   | 32.3 KB  |
| clang    | 32.8 KB  |
| odin     | 319.8 KB |
| rustc    | 459.0 KB |

#### stress_fun_10k.

Workload: 10,000 lines of code.

@RUN: `just zo_bench stress_fun_10k`

| Compiler | Run 1    | Run 2    | Run 3    | Run 4    | Run 5    | Average      | Speed vs zo     |
| :------- | :------- | :------- | :------- | :------- | :------- | :----------- | :-------------- |
| **zo**   | 60.17ms  | 51.43ms  | 51.38ms  | 51.73ms  | 51.62ms  | **53.27ms**  | **1.0x**        |
| clang    | 105.23ms | 96.81ms  | 94.96ms  | 97.49ms  | 96.87ms  | **98.27ms**  | **1.8x slower** |
| odin     | 199.62ms | 186.51ms | 182.72ms | 191.62ms | 184.23ms | **188.94ms** | **3.5x slower** |
| rustc    | 219.27ms | 206.80ms | 204.89ms | 214.84ms | 232.21ms | **215.60ms** | **4.0x slower** |

**-AWTY—are-we-tiny-yet?**

| Compiler | Size     |
| :------- | :------- |
| clang    | 165.3 KB |
| **zo**   | 272.7 KB |
| odin     | 426.3 KB |
| rustc    | 661.3 KB |

### benchmark — runtime.

| Benchmark         | clang     | rustc       | odin        | zo        |
| :---------------- | :-------- | :---------- | :---------- | :-------- |
| `ackermann`       | 28.60ms   | 29.15ms     | 32.21ms     | 29.91ms   |
| `arithmetic`      | 30.27ms   | 30.05ms     | 29.14ms     | 28.22ms   |
| `fibonacci`       | 28.41ms   | 29.81ms     | 28.62ms     | 28.22ms   |
| `hello`           | 28.71ms   | 29.40ms     | 28.94ms     | 28.19ms   |
| **`munchhausen`** | **5.86s** | **14.65s**  | **15.02s**  | **7.38s** |
| `rule_110`        | 46.92ms   | 34.78ms     | 34.02ms     | 28.29ms   |
| `stress_fun_10k`  | 28.66ms   | 29.23ms     | 29.70ms     | 28.47ms   |

### benchmark — summary.

| Benchmark        | `zo` vs `c`     | `zo` vs `rust`   | `zo` vs `odin`   |
| :--------------- | :-------------- | :--------------- | :--------------- |
| `ackermann`      | __7.1x__ faster | __10.8x__ faster | __11.3x__ faster |
| `arithmetic`     | __7.0x__ faster | __9.5x__ faster  | __23.6x__ faster |
| `fibonacci`      | __7.9x__ faster | __11.9x__ faster | __28.0x__ faster |
| `hello`          | __8.0x__ faster | __12.1x__ faster | __28.2x__ faster |
| `munchhausen`    | __5.8x__ faster | __9.3x__ faster  | __23.0x__ faster |
| `rule_110`       | __6.6x__ faster | __14.6x__ faster | __28.3x__ faster |
| `stress_fun_10k` | __1.8x__ faster | __4.0x__ faster  | __3.5x__ faster  |
