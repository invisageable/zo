# crates — compiler — benches.

> *A series of benches to compare compile-time between c, go, rust, odin, gleam and zo.*

## about.

iT'S REALLY ABOUT CHALLENGiNG. zo DOESN'T CLAiM TO BE A DiRECT COMPETiTOR TO c, rust OR odin. THE NUMBER OF OPTiMiSATiON AND THE QUALiTY OF THESE COMPiLERS ARE A FAR CRY FROM MY NAiVE ViSiON OF BUiLDiNG COMPiLERS. SO YES, CURRENTLY COMPiLiNG THE FAMOUS `hello, world!` iS FASTER THAN iN c, rust OR odin, BUT KEEP iN MiND THAT THE PiPELiNES ARE COMPLETELY DiFFERENT. iT'S BEST NOT TO POP THE CHAMPAGNE TO CELEBRATE ANYTHiNG YET. THERE iS SO MUCH STUFF TO DO. BUT THESE BENCHES GiVE US HOPE, AND THAT'S ALL THAT MATTERS TO STAY MOTiVATED.

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

#### hello.

@RUN: `just zo_bench hello`

| Compiler | min      | median     | mean     | Speed vs zo      |
| :------- | :------- | :--------- | :------- | :--------------- |
| **zo**   | 6.15ms   | **6.79ms** | 6.58ms   | **1.0x**         |
| clang    | 36.98ms  | 38.67ms    | 39.21ms  | **5.7x slower**  |
| rustc    | 66.68ms  | 67.12ms    | 67.12ms  | **9.9x slower**  |
| go       | 83.16ms  | 86.67ms    | 85.53ms  | **12.8x slower** |
| odin     | 148.42ms | 150.02ms   | 149.60ms | **22.1x slower** |

**-AWTY—are-we-tiny-yet?**

| Compiler | Size      |
| :------- | :-------- |
| **zo**   | 32.2 KB   |
| clang    | 32.6 KB   |
| odin     | 319.6 KB  |
| rustc    | 455.5 KB  |
| go       | 2434.0 KB |

#### arithmetic.

@RUN: `just zo_bench arithmetic`

| Compiler | min      | median     | mean     | Speed vs zo      |
| :------- | :------- | :--------- | :------- | :--------------- |
| **zo**   | 5.81ms   | **5.85ms** | 5.88ms   | **1.0x**         |
| clang    | 36.96ms  | 40.11ms    | 39.42ms  | **6.9x slower**  |
| rustc    | 70.35ms  | 72.65ms    | 71.82ms  | **12.4x slower** |
| go       | 84.01ms  | 87.21ms    | 86.07ms  | **14.9x slower** |
| odin     | 146.01ms | 146.62ms   | 146.46ms | **25.1x slower** |

**-AWTY—are-we-tiny-yet?**

| Compiler | Size      |
| :------- | :-------- |
| **zo**   | 32.2 KB   |
| clang    | 32.7 KB   |
| odin     | 319.7 KB  |
| rustc    | 456.3 KB  |
| go       | 2434.0 KB |

#### ackermann.

@RUN: `just zo_bench ackermann`

| Compiler | min      | median     | mean    | Speed vs zo      |
| :------- | :------- | :--------- | :------ | :--------------- |
| **zo**   | 5.80ms   | **6.10ms** | 6.05ms  | **1.0x**         |
| clang    | 35.12ms  | 37.65ms    | 37.25ms | **6.2x slower**  |
| go       | 68.94ms  | 69.65ms    | 70.18ms | **11.4x slower** |
| rustc    | 72.13ms  | 73.38ms    | 72.79ms | **12.0x slower** |
| odin     | 70.63ms  | 78.44ms    | 78.05ms | **12.9x slower** |

**-AWTY—are-we-tiny-yet?**

| Compiler | Size      |
| :------- | :-------- |
| **zo**   | 32.2 KB   |
| clang    | 32.7 KB   |
| odin     | 71.7 KB   |
| rustc    | 453.3 KB  |
| go       | 1708.3 KB |

#### fannkuch-redux.

@RUN: `just zo_bench fannkuch-redux`

| Compiler | min      | median     | mean     | Speed vs zo      |
| :------- | :------- | :--------- | :------- | :--------------- |
| **zo**   | 6.35ms   | **6.47ms** | 6.50ms   | **1.0x**         |
| clang    | 40.83ms  | 41.80ms    | 41.53ms  | **6.5x slower**  |
| rustc    | 84.58ms  | 85.43ms    | 85.49ms  | **13.2x slower** |
| go       | 83.96ms  | 86.73ms    | 85.68ms  | **13.4x slower** |
| odin     | 151.92ms | 155.87ms   | 154.32ms | **24.1x slower** |
| gleam    | 196.81ms | 203.75ms   | 202.47ms | **31.5x slower** |

**-AWTY—are-we-tiny-yet?**

| Compiler | Size      |
| :------- | :-------- |
| **zo**   | 32.9 KB   |
| clang    | 32.8 KB   |
| odin     | 321.1 KB  |
| rustc    | 487.4 KB  |
| go       | 2434.7 KB |

#### fibonacci.

@RUN: `just zo_bench fibonacci`

| Compiler | min      | median     | mean     | Speed vs zo      |
| :------- | :------- | :--------- | :------- | :--------------- |
| **zo**   | 5.76ms   | **5.91ms** | 5.88ms   | **1.0x**         |
| clang    | 36.80ms  | 39.14ms    | 39.01ms  | **6.6x slower**  |
| rustc    | 71.10ms  | 72.05ms    | 73.43ms  | **12.2x slower** |
| go       | 83.70ms  | 85.91ms    | 85.07ms  | **14.5x slower** |
| odin     | 147.46ms | 149.70ms   | 149.22ms | **25.3x slower** |

**-AWTY—are-we-tiny-yet?**

| Compiler | Size      |
| :------- | :-------- |
| **zo**   | 32.2 KB   |
| clang    | 32.7 KB   |
| odin     | 319.7 KB  |
| rustc    | 457.5 KB  |
| go       | 2434.1 KB |

#### mandelbrot.

@RUN: `just zo_bench mandelbrot`

| Compiler | min      | median     | mean     | Speed vs zo      |
| :------- | :------- | :--------- | :------- | :--------------- |
| **zo**   | 5.68ms   | **6.13ms** | 5.99ms   | **1.0x**         |
| clang    | 38.66ms  | 40.89ms    | 40.24ms  | **6.7x slower**  |
| rustc    | 70.11ms  | 72.19ms    | 71.68ms  | **11.8x slower** |
| go       | 83.56ms  | 85.10ms    | 85.28ms  | **13.9x slower** |
| odin     | 146.43ms | 156.21ms   | 154.03ms | **25.5x slower** |
| gleam    | 194.28ms | 196.10ms   | 196.88ms | **32.0x slower** |

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

| Compiler | min      | median     | mean     | Speed vs zo      |
| :------- | :------- | :--------- | :------- | :--------------- |
| **zo**   | 6.11ms   | **6.13ms** | 6.12ms   | **1.0x**         |
| clang    | 37.10ms  | 38.40ms    | 38.26ms  | **6.3x slower**  |
| rustc    | 71.96ms  | 72.30ms    | 72.27ms  | **11.8x slower** |
| go       | 83.54ms  | 84.89ms    | 84.80ms  | **13.8x slower** |
| odin     | 146.20ms | 150.78ms   | 150.33ms | **24.6x slower** |
| gleam    | 191.35ms | 193.04ms   | 194.68ms | **31.5x slower** |

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

| Compiler | min      | median     | mean     | Speed vs zo      |
| :------- | :------- | :--------- | :------- | :--------------- |
| **zo**   | 7.55ms   | **7.75ms** | 7.66ms   | **1.0x**         |
| clang    | 42.03ms  | 43.01ms    | 43.34ms  | **5.5x slower**  |
| go       | 87.60ms  | 88.55ms    | 89.01ms  | **11.4x slower** |
| rustc    | 92.71ms  | 95.90ms    | 95.51ms  | **12.4x slower** |
| odin     | 154.66ms | 156.58ms   | 157.58ms | **20.2x slower** |
| gleam    | 202.65ms | 235.11ms   | 226.48ms | **30.3x slower** |

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

| Compiler | min      | median     | mean     | Speed vs zo      |
| :------- | :------- | :--------- | :------- | :--------------- |
| **zo**   | 6.81ms   | **7.47ms** | 7.18ms   | **1.0x**         |
| clang    | 37.47ms  | 40.28ms    | 39.63ms  | **5.4x slower**  |
| rustc    | 77.49ms  | 81.91ms    | 80.62ms  | **11.0x slower** |
| go       | 87.51ms  | 88.82ms    | 88.48ms  | **11.9x slower** |
| odin     | 155.11ms | 158.18ms   | 157.44ms | **21.2x slower** |

**-AWTY—are-we-tiny-yet?**

| Compiler | Size      |
| :------- | :-------- |
| **zo**   | 32.3 KB   |
| clang    | 32.8 KB   |
| odin     | 319.8 KB  |
| rustc    | 459.0 KB  |
| go       | 2434.0 KB |

#### spectralnorm.

@RUN: `just zo_bench spectralnorm`

| Compiler | min      | median     | mean     | Speed vs zo      |
| :------- | :------- | :--------- | :------- | :--------------- |
| **zo**   | 7.20ms   | **7.37ms** | 7.34ms   | **1.0x**         |
| clang    | 41.53ms  | 45.66ms    | 44.90ms  | **6.2x slower**  |
| go       | 84.64ms  | 88.67ms    | 87.52ms  | **12.0x slower** |
| rustc    | 82.14ms  | 89.32ms    | 87.15ms  | **12.1x slower** |
| odin     | 153.06ms | 163.23ms   | 161.48ms | **22.1x slower** |
| gleam    | 197.05ms | 202.89ms   | 201.61ms | **27.5x slower** |

**-AWTY—are-we-tiny-yet?**

| Compiler | Size      |
| :------- | :-------- |
| **zo**   | 32.8 KB   |
| clang    | 32.9 KB   |
| odin     | 321.3 KB  |
| rustc    | 505.8 KB  |
| go       | 2434.7 KB |

#### stress_fun_10k.

@RUN: `just zo_bench stress_fun_10k`

| Compiler | min      | median      | mean     | Speed vs zo     |
| :------- | :------- | :---------- | :------- | :-------------- |
| **zo**   | 23.91ms  | **24.44ms** | 24.39ms  | **1.0x**        |
| go       | 88.55ms  | 89.73ms     | 90.15ms  | **3.7x slower** |
| clang    | 98.05ms  | 102.87ms    | 102.04ms | **4.2x slower** |
| odin     | 174.88ms | 183.07ms    | 179.28ms | **7.5x slower** |
| rustc    | 207.66ms | 209.19ms    | 208.65ms | **8.6x slower** |

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

| Compiler | min   | median    | mean  | Speed vs zo     |
| :------- | :---- | :-------- | :---- | :-------------- |
| **zo**   | 1.88s | **1.90s** | 1.89s | **1.0x**        |
| clang    | 3.13s | 3.15s     | 3.15s | **1.7x slower** |
| odin     | 3.30s | 3.39s     | 3.35s | **1.8x slower** |
| go       | 3.37s | 3.41s     | 3.41s | **1.8x slower** |
| rustc    | —     | crash     | —     | **SIGBUS**      |

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

| Compiler | min      | median     | mean     | Speed vs zo      |
| :------- | :------- | :--------- | :------- | :--------------- |
| **zo**   | 6.19ms   | **6.37ms** | 6.32ms   | **1.0x**         |
| clang    | 44.20ms  | 44.96ms    | 46.04ms  | **7.1x slower**  |
| go       | 82.59ms  | 85.51ms    | 85.42ms  | **13.4x slower** |
| rustc    | 104.28ms | 104.89ms   | 104.99ms | **16.5x slower** |
| odin     | 145.70ms | 151.24ms   | 149.21ms | **23.7x slower** |
| gleam    | 192.20ms | 194.09ms   | 194.13ms | **30.5x slower** |

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
| `ackermann`      | __6.2x__ faster | __11.4x__ faster | __12.0x__ faster | __12.9x__ faster | —                |
| `arithmetic`     | __6.9x__ faster | __14.9x__ faster | __12.4x__ faster | __25.1x__ faster | —                |
| `fannkuch-redux` | __6.5x__ faster | __13.4x__ faster | __13.2x__ faster | __24.1x__ faster | __31.5x__ faster |
| `fibonacci`      | __6.6x__ faster | __14.5x__ faster | __12.2x__ faster | __25.3x__ faster | —                |
| `hello`          | __5.7x__ faster | __12.8x__ faster | __9.9x__ faster  | __22.1x__ faster | —                |
| `mandelbrot`     | __6.7x__ faster | __13.9x__ faster | __11.8x__ faster | __25.5x__ faster | __32.0x__ faster |
| `munchhausen`    | __6.3x__ faster | __13.8x__ faster | __11.8x__ faster | __24.6x__ faster | __31.5x__ faster |
| `n-body`         | __5.5x__ faster | __11.4x__ faster | __12.4x__ faster | __20.2x__ faster | __30.3x__ faster |
| `rule_110`       | __5.4x__ faster | __11.9x__ faster | __11.0x__ faster | __21.2x__ faster | —                |
| `spectralnorm`   | __6.2x__ faster | __12.0x__ faster | __12.1x__ faster | __22.1x__ faster | __27.5x__ faster |
| `stress_fun_10k` | __4.2x__ faster | __3.7x__ faster  | __8.6x__ faster  | __7.5x__ faster  | —                |
| `stress_fun_500k`| __1.7x__ faster | __1.8x__ faster  | crash            | __1.8x__ faster  | —                |
| `threadring`     | __7.1x__ faster | __13.4x__ faster | __16.5x__ faster | __23.7x__ faster | __30.5x__ faster |

### benchmark — runtime.

WE ARE MEASURiNG HOW LONG DiD iT TOOK TO **EXECUTE** AN EXECUTABLE FiLE.

| Benchmark         | clang     | go        | rustc     | odin      | zo        | gleam     |
| :---------------- | :-------- | :-------- | :-------- | :-------- | :-------- | :-------- |
| `ackermann`       | **1.3ms** | 2.0ms     | 1.7ms     | 1.8ms     | 5.2ms     | —         |
| `arithmetic`      | **1.5ms** | 2.3ms     | 1.5ms     | 1.6ms     | 4.9ms     | —         |
| `fannkuch-redux`  | 239.9ms   | **146.3ms**| 965.8ms  | 624.1ms   | 306.9ms   | —         |
| `fibonacci`       | **1.4ms** | 2.5ms     | 1.4ms     | 1.9ms     | 3.2ms     | —         |
| `hello`           | 1.6ms     | 2.7ms     | 1.8ms     | **1.4ms** | 5.8ms     | —         |
| `mandelbrot`      | 69.7ms    | **33.1ms**| 72.9ms    | 85.0ms    | 78.6ms    | 363.2ms   |
| `munchhausen`     | 5.57s     | **2.08s** | 14.66s    | 14.71s    | 6.11s     | 24.39s    |
| `n-body`          | **1.8ms** | 2.2ms     | 2.0ms     | 2.5ms     | 4.7ms     | 97.3ms    |
| `rule_110`        | 2.8ms     | 2.8ms     | **1.9ms** | 2.1ms     | 5.0ms     | —         |
| `spectralnorm`    | 117.8ms   | **96.8ms**| 389.3ms   | 197.3ms   | 186.1ms   | —         |
| `stress_fun_10k`  | 1.6ms     | 2.9ms     | **1.5ms** | 1.8ms     | 5.1ms     | —         |
| `stress_fun_500k` | **3.6ms** | 13.0ms    | —         | 5.5ms     | 8.2ms     | —         |
| `threadring`      | 11.3ms    | **3.5ms** | 10.6ms    | 13.5ms    | 7.8ms     | 97.7ms    |

### benchmark — runtime (summary).

TODO: table.