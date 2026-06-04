# crates — compiler — benches.

> *A series of benches to compare compile-time between c, go, rust, odin, gleam and zo.*

## about.

iT'S REALLY ABOUT CHALLENGiNG. zo DOESN'T CLAiM TO BE A DiRECT COMPETiTOR TO c, rust OR odin. THE NUMBER OF OPTiMiSATiON AND THE QUALiTY OF THESE COMPiLERS ARE A FAR CRY FROM MY NAiVE ViSiON OF BUiLDiNG COMPiLERS. SO YES, CURRENTLY COMPiLiNG THE FAMOUS `hello, world!` iS 8-28X FASTER THAN iN c, rust OR odin, BUT KEEP iN MiND THAT THE PiPELiNES ARE COMPLETELY DiFFERENT. iT'S BEST NOT TO POP THE CHAMPAGNE TO CELEBRATE ANYTHiNG YET. THERE iS SO MUCH STUFF TO DO. BUT THESE BENCHES GiVE US HOPE, AND THAT'S ALL THAT MATTERS TO STAY MOTiVATED.

> *« Compile everything, every time, instantly. » — i10e*

## dev.

iNSTALL ALL PROGRAMMiNG LANGUAGES REQUiRED:

  - @SEE — [clang](https://clang.llvm.org/get_started.html) OR [gcc](https://gcc.gnu.org/install)
  - @SEE — [gleam](https://gleam.run/getting-started/installing)
  - @SEE — [go](https://go.dev/dl)
  - @SEE — [oding](https://odin-lang.org/docs/install)
  - @SEE — [rust](https://rust-lang.org/tools/install)

## benchmark.

> *Measurements: `arm64-apple-darwin`. 5 runs per bench.*

> *gleam compiles to BEAM bytecode, not a native binary — it appears in the comptime and runtime tables but not the size tables. Its runtime carries the BEAM VM startup floor (~95 ms), and BEAM is not tuned for tight numeric loops.*

### benchmark — comptime.

WE ARE MEASURiNG HOW LONG DiD iT TOOK TO **BUiLD** AN EXECUTABLE FiLE.

#### ackermann.

@RUN: `just zo_bench ackermann`

| Compiler | Run 1    | Run 2   | Run 3   | Run 4   | Run 5   | Average     | Speed vs zo      |
| :------- | :------- | :------ | :------ | :------ | :------ | :---------- | :--------------- |
| **zo**   | 7.59ms   | 6.43ms  | 6.04ms  | 5.89ms  | 5.62ms  | **6.32ms**  | **1.0x**         |
| clang    | 66.67ms  | 41.95ms | 38.26ms | 40.30ms | 36.71ms | **44.78ms** | **7.1x slower**  |
| go       | 265.33ms | 63.85ms | 66.68ms | 65.32ms | 65.51ms | **105.34ms** | **16.7x slower** |
| rustc    | 78.54ms  | 67.90ms | 66.90ms | 62.96ms | 63.91ms | **68.04ms** | **10.8x slower** |
| odin     | 82.21ms  | 69.02ms | 69.69ms | 68.28ms | 67.13ms | **71.27ms** | **11.3x slower** |

**-AWTY—are-we-tiny-yet?**

| Compiler | Size      |
| :------- | :-------- |
| **zo**   | 32.2 KB   |
| clang    | 32.7 KB   |
| odin     | 71.7 KB   |
| rustc    | 453.3 KB  |
| go       | 1708.3 KB |

#### arithmetic.

@RUN: `just zo_bench arithmetic`

| Compiler | Run 1    | Run 2    | Run 3    | Run 4    | Run 5    | Average      | Speed vs zo      |
| :------- | :------- | :------- | :------- | :------- | :------- | :----------- | :--------------- |
| **zo**   | 8.06ms   | 7.16ms   | 6.17ms   | 6.21ms   | 5.82ms   | **6.68ms**   | **1.0x**         |
| clang    | 56.41ms  | 47.04ms  | 45.73ms  | 42.27ms  | 40.70ms  | **46.43ms**  | **7.0x slower**  |
| go       | 147.84ms | 88.28ms  | 86.15ms  | 88.62ms  | 87.28ms  | **99.63ms**  | **14.9x slower** |
| rustc    | 70.99ms  | 62.33ms  | 60.93ms  | 61.11ms  | 61.72ms  | **63.42ms**  | **9.5x slower**  |
| odin     | 162.77ms | 174.88ms | 154.65ms | 147.32ms | 149.02ms | **157.73ms** | **23.6x slower** |

**-AWTY—are-we-tiny-yet?**

| Compiler | Size      |
| :------- | :-------- |
| **zo**   | 32.2 KB   |
| clang    | 32.7 KB   |
| odin     | 319.7 KB  |
| rustc    | 456.3 KB  |
| go       | 2434.0 KB |

#### fibonacci.

@RUN: `just zo_bench fibonacci`

| Compiler | Run 1    | Run 2    | Run 3    | Run 4    | Run 5    | Average      | Speed vs zo      |
| :------- | :------- | :------- | :------- | :------- | :------- | :----------- | :--------------- |
| **zo**   | 6.99ms   | 5.79ms   | 5.55ms   | 5.07ms   | 5.06ms   | **5.69ms**   | **1.0x**         |
| clang    | 56.80ms  | 45.23ms  | 43.49ms  | 40.42ms  | 38.95ms  | **44.98ms**  | **7.9x slower**  |
| go       | 92.56ms  | 85.45ms  | 89.30ms  | 87.17ms  | 88.96ms  | **88.69ms**  | **15.6x slower** |
| rustc    | 78.61ms  | 68.84ms  | 66.22ms  | 62.21ms  | 62.53ms  | **67.68ms**  | **11.9x slower** |
| odin     | 171.06ms | 168.63ms | 159.61ms | 146.89ms | 150.68ms | **159.37ms** | **28.0x slower** |

**-AWTY—are-we-tiny-yet?**

| Compiler | Size      |
| :------- | :-------- |
| **zo**   | 32.2 KB   |
| clang    | 32.7 KB   |
| odin     | 319.7 KB  |
| rustc    | 457.5 KB  |
| go       | 2434.1 KB |

#### hello.

@RUN: `just zo_bench hello`

| Compiler | Run 1    | Run 2    | Run 3    | Run 4    | Run 5    | Average      | Speed vs zo      |
| :------- | :------- | :------- | :------- | :------- | :------- | :----------- | :--------------- |
| **zo**   | 6.39ms   | 5.75ms   | 5.66ms   | 4.90ms   | 5.11ms   | **5.56ms**   | **1.0x**         |
| clang    | 53.08ms  | 41.28ms  | 42.00ms  | 42.99ms  | 42.01ms  | **44.27ms**  | **8.0x slower**  |
| go       | 88.89ms  | 84.18ms  | 88.98ms  | 87.95ms  | 109.92ms | **91.98ms**  | **16.5x slower** |
| rustc    | 75.72ms  | 62.08ms  | 61.88ms  | 76.28ms  | 59.24ms  | **67.04ms**  | **12.1x slower** |
| odin     | 153.82ms | 151.34ms | 159.92ms | 166.96ms | 152.00ms | **156.81ms** | **28.2x slower** |

**-AWTY—are-we-tiny-yet?**

| Compiler | Size      |
| :------- | :-------- |
| **zo**   | 32.2 KB   |
| clang    | 32.6 KB   |
| odin     | 319.6 KB  |
| rustc    | 455.5 KB  |
| go       | 2434.0 KB |

#### mandelbrot.

@RUN: `just zo_bench mandelbrot`

| Compiler | Run 1    | Run 2    | Run 3    | Run 4    | Run 5    | Average      | Speed vs zo      |
| :------- | :------- | :------- | :------- | :------- | :------- | :----------- | :--------------- |
| **zo**   | 20.00ms  | 5.82ms   | 5.88ms   | 6.38ms   | 7.09ms   | **9.03ms**   | **1.0x**         |
| clang    | 129.80ms | 42.39ms  | 39.71ms  | 41.26ms  | 40.32ms  | **58.70ms**  | **6.5x slower**  |
| go       | 86.74ms  | 87.03ms  | 83.72ms  | 86.09ms  | 86.07ms  | **85.93ms**  | **9.5x slower**  |
| rustc    | 69.46ms  | 71.83ms  | 69.06ms  | 69.55ms  | 72.20ms  | **70.42ms**  | **7.8x slower**  |
| odin     | 311.04ms | 151.96ms | 153.38ms | 152.06ms | 150.62ms | **183.81ms** | **20.4x slower** |
| gleam    | 249.08ms | 203.95ms | 203.75ms | 213.00ms | 207.23ms | **215.40ms** | **23.9x slower** |

**-AWTY—are-we-tiny-yet?**

| Compiler | Size      |
| :------- | :-------- |
| **zo**   | 32.6 KB   |
| clang    | 32.6 KB   |
| odin     | 185.6 KB  |
| rustc    | 455.3 KB  |
| go       | 2434.2 KB |

#### munchhausen numbers.

@RUN: `just zo_bench munchhausen`

| Compiler | Run 1    | Run 2    | Run 3    | Run 4    | Run 5    | Average      | Speed vs zo      |
| :------- | :------- | :------- | :------- | :------- | :------- | :----------- | :--------------- |
| **zo**   | 20.22ms  | 4.78ms   | 4.31ms   | 4.52ms   | 4.93ms   | **7.75ms**   | **1.0x**         |
| clang    | 56.26ms  | 44.06ms  | 42.37ms  | 39.05ms  | 41.75ms  | **44.70ms**  | **5.8x slower**  |
| go       | 91.92ms  | 87.01ms  | 88.67ms  | 86.80ms  | 87.27ms  | **88.33ms**  | **11.4x slower** |
| rustc    | 87.94ms  | 70.30ms  | 68.92ms  | 66.66ms  | 65.60ms  | **71.88ms**  | **9.3x slower**  |
| odin     | 280.14ms | 155.56ms | 151.32ms | 147.46ms | 156.76ms | **178.25ms** | **23.0x slower** |
| gleam    | 242.19ms | 211.24ms | 207.36ms | 205.27ms | 205.45ms | **214.30ms** | **27.7x slower** |

**-AWTY—are-we-tiny-yet?**

| Compiler | Size      |
| :------- | :-------- |
| **zo**   | 32.3 KB   |
| clang    | 32.9 KB   |
| odin     | 319.8 KB  |
| rustc    | 458.3 KB  |
| go       | 2434.0 KB |

#### n-body simulation.

@RUN: `just zo_bench n-body`

| Compiler | Run 1    | Run 2    | Run 3    | Run 4    | Run 5    | Average      | Speed vs zo      |
| :------- | :------- | :------- | :------- | :------- | :------- | :----------- | :--------------- |
| **zo**   | 7.84ms   | 6.87ms   | 6.47ms   | 7.51ms   | 7.45ms   | **7.23ms**   | **1.0x**         |
| clang    | 48.50ms  | 47.76ms  | 44.37ms  | 45.77ms  | 45.31ms  | **46.34ms**  | **6.4x slower**  |
| go       | 92.07ms  | 93.46ms  | 121.81ms | 91.77ms  | 93.89ms  | **98.60ms**  | **13.6x slower** |
| rustc    | 94.83ms  | 94.09ms  | 96.04ms  | 93.05ms  | 93.06ms  | **94.21ms**  | **13.0x slower** |
| odin     | 181.35ms | 162.48ms | 163.04ms | 167.07ms | 165.51ms | **167.89ms** | **23.2x slower** |
| gleam    | 212.21ms | 213.47ms | 211.22ms | 217.17ms | 216.32ms | **214.08ms** | **29.6x slower** |

**-AWTY—are-we-tiny-yet?**

| Compiler | Size      |
| :------- | :-------- |
| **zo**   | 32.8 KB   |
| clang    | 49.0 KB   |
| odin     | 336.9 KB  |
| rustc    | 523.0 KB  |
| go       | 2555.7 KB |

#### rule 110 cellular automaton.

@RUN: `just zo_bench rule_110`

| Compiler | Run 1    | Run 2    | Run 3    | Run 4    | Run 5    | Average      | Speed vs zo      |
| :------- | :------- | :------- | :------- | :------- | :------- | :----------- | :--------------- |
| **zo**   | 6.66ms   | 6.53ms   | 6.30ms   | 6.49ms   | 5.86ms   | **6.37ms**   | **1.0x**         |
| clang    | 52.13ms  | 39.25ms  | 39.92ms  | 38.67ms  | 41.29ms  | **42.25ms**  | **6.6x slower**  |
| go       | 87.96ms  | 83.56ms  | 86.61ms  | 86.02ms  | 86.53ms  | **86.14ms**  | **13.5x slower** |
| rustc    | 183.90ms | 71.22ms  | 70.09ms  | 70.25ms  | 68.95ms  | **92.88ms**  | **14.6x slower** |
| odin     | 279.17ms | 155.46ms | 153.92ms | 155.31ms | 157.59ms | **180.29ms** | **28.3x slower** |

**-AWTY—are-we-tiny-yet?**

| Compiler | Size      |
| :------- | :-------- |
| **zo**   | 32.3 KB   |
| clang    | 32.8 KB   |
| odin     | 319.8 KB  |
| rustc    | 459.0 KB  |
| go       | 2434.0 KB |

#### stress_fun_10k.

@RUN: `just zo_bench stress_fun_10k`

| Compiler | Run 1    | Run 2    | Run 3    | Run 4    | Run 5    | Average      | Speed vs zo     |
| :------- | :------- | :------- | :------- | :------- | :------- | :----------- | :-------------- |
| **zo**   | 34.54ms  | 27.21ms  | 25.60ms  | 25.12ms  | 25.38ms  | **27.57ms**  | **1.0x**        |
| clang    | 114.21ms | 96.51ms  | 96.43ms  | 96.05ms  | 96.84ms  | **100.01ms** | **3.6x slower** |
| go       | 106.71ms | 129.31ms | 94.79ms  | 89.16ms  | 92.33ms  | **102.46ms** | **3.7x slower** |
| odin     | 202.45ms | 181.54ms | 191.23ms | 177.91ms | 178.90ms | **186.41ms** | **6.8x slower** |
| rustc    | 218.81ms | 204.97ms | 204.53ms | 205.15ms | 204.76ms | **207.64ms** | **7.5x slower** |

> *Workload: 10,000 lines of code.*

**-AWTY—are-we-tiny-yet?**

| Compiler  | Size      |
| :-------- | :-------- |
| **clang** | 165.3 KB  |
| zo        | 177.8 KB  |
| odin      | 426.3 KB  |
| rustc     | 661.3 KB  |
| go        | 2721.5 KB |

#### stress_fun_500k.

@RUN: `just zo_bench stress_fun_500k`

| Compiler | Run 1 | Run 2 | Run 3 | Run 4 | Run 5 | Average   | Speed vs zo     |
| :------- | :---- | :---- | :---- | :---- | :---- | :-------- | :-------------- |
| **zo**   | 1.81s | 1.79s | 1.79s | 1.78s | 1.81s | **1.80s** | **1.0x**        |
| clang    | 3.34s | 3.02s | 3.02s | 2.99s | 2.94s | **3.06s** | **1.7x slower** |
| go       | 3.69s | 3.45s | 3.34s | 3.31s | 3.40s | **3.44s** | **1.9x slower** |
| rustc    | —     | —     | —     | —     | —     | **crash** | **SIGBUS**      |
| odin     | 3.89s | 3.66s | 3.69s | 3.66s | 3.66s | **3.71s** | **2.1x slower** |

> *Workload: 500,000 lines of code.*

**-AWTY—are-we-tiny-yet?**

| Compiler | Size     |
| :------- | :------- |
| **odin** | 5.98 MB  |
| clang    | 7.34 MB  |
| zo       | 7.53 MB  |
| go       | 16.97 MB |
| rustc    | —        |

#### threadring.

@RUN: `just zo_bench threadring`

| Compiler | Run 1    | Run 2    | Run 3    | Run 4    | Run 5    | Average      | Speed vs zo      |
| :------- | :------- | :------- | :------- | :------- | :------- | :----------- | :--------------- |
| **zo**   | 25.01ms  | 12.09ms  | 11.53ms  | 10.46ms  | 9.08ms   | **13.63ms**  | **1.0x**         |
| clang    | 270.34ms | 76.76ms  | 81.65ms  | 66.49ms  | 61.47ms  | **111.34ms** | **8.2x slower**  |
| go       | 376.35ms | 131.52ms | 125.96ms | 143.16ms | 129.17ms | **181.23ms** | **13.3x slower** |
| rustc    | 272.33ms | 164.42ms | 147.28ms | 170.54ms | 167.56ms | **184.42ms** | **13.5x slower** |
| gleam    | 205.87ms | 206.04ms | 205.66ms | 205.48ms | 202.47ms | **205.11ms** | **15.0x slower** |
| odin     | 422.94ms | 216.09ms | 219.56ms | 228.20ms | 230.74ms | **263.50ms** | **19.3x slower** |

> *Workload: 503 tasks in a ring. A token hops node-to-node `N` times.*

**-AWTY—are-we-tiny-yet?**

| Compiler | Size      |
| :------- | :-------- |
| **zo**   | 32.9 KB   |
| clang    | 33.2 KB   |
| odin     | 357.8 KB  |
| rustc    | 653.2 KB  |
| go       | 2435.0 KB |

### benchmark — comptime (summary).

| Benchmark        | `zo` vs `c`     | `zo` vs `go`     | `zo` vs `rust`   | `zo` vs `odin`   | `zo` vs `gleam`  |
| :--------------- | :-------------- | :--------------- | :--------------- | :--------------- | :--------------- |
| `ackermann`      | __7.1x__ faster | __16.7x__ faster | __10.8x__ faster | __11.3x__ faster | —                |
| `arithmetic`     | __7.0x__ faster | __14.9x__ faster | __9.5x__ faster  | __23.6x__ faster | —                |
| `fibonacci`      | __7.9x__ faster | __15.6x__ faster | __11.9x__ faster | __28.0x__ faster | —                |
| `hello`          | __8.0x__ faster | __16.5x__ faster | __12.1x__ faster | __28.2x__ faster | —                |
| `mandelbrot`     | __6.5x__ faster | __9.5x__ faster  | __7.8x__ faster  | __20.4x__ faster | __23.9x__ faster |
| `munchhausen`    | __5.8x__ faster | __11.4x__ faster | __9.3x__ faster  | __23.0x__ faster | __27.7x__ faster |
| `n-body`         | __6.4x__ faster | __13.6x__ faster | __13.0x__ faster | __23.2x__ faster | __29.6x__ faster |
| `rule_110`       | __6.6x__ faster | __13.5x__ faster | __14.6x__ faster | __28.3x__ faster | —                |
| `stress_fun_10k` | __3.6x__ faster | __3.7x__ faster  | __7.5x__ faster  | __6.8x__ faster  | —                |
| `stress_fun_500k`| __1.7x__ faster | __1.9x__ faster  | crash            | __2.1x__ faster  | —                |
| `threadring`     | __8.2x__ faster | __13.3x__ faster | __13.5x__ faster | __19.3x__ faster | __15.0x__ faster |

### benchmark — runtime.

WE ARE MEASURiNG HOW LONG DiD iT TOOK TO **EXECUTE** AN EXECUTABLE FiLE.

| Benchmark         | clang     | go        | rustc     | odin      | zo        | gleam     |
| :---------------- | :-------- | :-------- | :-------- | :-------- | :-------- | :-------- |
| `ackermann`       | **1.3ms** | 2.0ms     | 1.7ms     | 1.8ms     | 5.2ms     | —         |
| `arithmetic`      | **1.5ms** | 2.3ms     | 1.5ms     | 1.6ms     | 4.9ms     | —         |
| `fibonacci`       | **1.4ms** | 2.5ms     | 1.4ms     | 1.9ms     | 3.2ms     | —         |
| `hello`           | 1.6ms     | 2.7ms     | 1.8ms     | **1.4ms** | 5.8ms     | —         |
| `mandelbrot`      | 70.5ms    | **30.7ms**| 71.9ms    | 86.3ms    | 75.3ms    | 368.1ms   |
| `munchhausen`     | 5.79s     | **2.06s** | 14.39s    | 14.88s    | 5.98s     | 23.32s    |
| `n-body`          | **1.8ms** | 2.2ms     | 2.0ms     | 2.5ms     | 4.7ms     | 97.3ms    |
| `rule_110`        | 2.8ms     | 2.8ms     | **1.9ms** | 2.1ms     | 5.0ms     | —         |
| `stress_fun_10k`  | 1.6ms     | 2.9ms     | **1.5ms** | 1.8ms     | 5.1ms     | —         |
| `stress_fun_500k` | **3.6ms** | 13.0ms    | —         | 5.5ms     | 8.2ms     | —         |
| `threadring`      | 11.3ms    | **3.5ms** | 10.6ms    | 13.5ms    | 7.8ms     | 97.7ms    |

> *Warm steady-state (cold first run excluded — that's dyld/disk warmup, not the program). zo links a 1.34 MB runtime dylib; clang/rustc/odin are static, hence zo's ~3 ms startup floor on trivial programs.*

### benchmark — runtime (summary).

TODO: table.