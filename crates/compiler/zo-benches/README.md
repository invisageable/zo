# crates — compiler — benches.

> *A series of benches to compare compile-time between c, rust and zo.*

## about.

iT'S REALLY ABOUT CHALLENGiNG. zo DOESN'T CLAiM TO BE A DiRECT COMPETiTOR TO c OR rust. THE NUMBER OF OPTiMiSATiON AND THE QUALiTY OF THESE COMPiLERS ARE A FAR CRY FROM MY NAiVE ViSiON OF BUiLDiNG COMPiLERS. SO YES, CURRENTLY COMPiLiNG THE FAMOUS `hello, world!` iS 20-80X FASTER THAN iN C OR RUST, BUT KEEP iN MiND THAT THE PiPELiNES ARE COMPLETELY DiFFERENT. iT'S BEST NOT TO POP THE CHAMPAGNE TO CELEBRATE ANYTHiNG YET. THERE iS SO MUCH STUFF TO DO. BUT THESE BENCHES GiVE US HOPE, AND THAT'S ALL THAT MATTERS TO STAY MOTiVATED.

> *« Compile everything, every time, instantly. »*
> &emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;&emsp;— i10e

## benchmark.

### benchmark — results.

#### hello.

@RUN: `just zo_bench hello`

### hello world compilation speed.

| Compiler | Run 1 | Run 2 | Run 3 | Run 4 | Run 5 | Average     | Speed vs zo      |
| :------- | :---- |:----- | :---- | :---- | :---- | :---------- | :--------------- |
| **zo**   | 16ms  | 6ms   | 5ms   | 6ms   | 6ms   | **7.8ms**   | **1.0x**         |
| clang    | 122ms | 44ms  | 43ms  | 43ms  | 42ms  | **58.8ms**  | **7.5x slower**  |
| rustc    | 910ms | 93ms  | 84ms  | 81ms  | 84ms  | **250.4ms** | **32.1x slower** |

**-AWTY—are-we-tiny-yet?**

| Compiler | Size   |
| :------- | :----- |
| **zo**   | 33 KB  |
| clang    | 33 KB  |
| rustc    | 441 KB |

### arithmetic operations compilation speed.

@RUN: `just zo_bench arithmetic`

| Compiler | Run 1 | Run 2 | Run 3 | Run 4 | Run 5 | Average     | Speed vs zo      |
| :------- | :---- |:----- | :---- | :---- | :---- | :---------- | :--------------- |
| **zo**   | 18ms  | 10ms  | 12ms  | 8ms   | 5ms   | **10.6ms**  | **1.0x**         |
| clang    | 94ms  | 44ms  | 44ms  | 42ms  | 42ms  | **53.2ms**  | **5.0x slower**  |
| rustc    | 910ms | 95ms  | 90ms  | 89ms  | 90ms  | **254.8ms** | **24.0x slower** |

**-AWTY—are-we-tiny-yet?**

| Compiler | Size   |
| :------- | :----- |
| clang    | 17 KB  |
| **zo**   | 33 KB  |
| rustc    | 439 KB |

### munchhausen numbers compilation speed.

@RUN: `just zo_bench munchhausen`

| Compiler | Run 1    | Run 2    | Run 3    | Run 4    | Run 5    | Average     | Speed vs zo     |
| :------- | :------- | :------- | :------- | :------- | :------- | :---------- | :-------------- |
| **zo**   | 190.21ms | 9.35ms   | 7.92ms   | 9.83ms   | 8.56ms   | **45.2ms**  | **1.0x**        |
| clang    | 68.83ms  | 62.96ms  | 60.40ms  | 59.30ms  | 59.23ms  | **62.1ms**  | **1.4x slower** |
| rustc    | 191.53ms | 103.99ms | 105.61ms | 96.02ms  | 104.20ms | **120.3ms** | **2.7x slower** |

> *Hot averages (runs 2–5): zo **8.9ms**, clang **60.5ms** (6.8× slower), rustc **102.5ms** (11.5× slower).*

### benchmark — summary.

| Benchmark     | `zo` vs `c`      | `zo` vs `rust`   |
| ------------- | ---------------- | ---------------- |
| `hello`       | __25.5x__ faster | __47.5x__ faster |
| `arithmetic`  | __53x__ faster   | __86x__ faster   |
| `munchhausen` | __1.4x__ faster  | __2.7x__ faster  |
