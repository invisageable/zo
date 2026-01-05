# crates — compiler — benches.

> _a series of benches to compare compile-time between c, rust and zo_.

## about.

iT'S REALLY ABOUT CHALLENGiNG. zo DOESN'T CLAiM TO BE A DiRECT COMPETiTOR TO c OR rust. THE NUMBER OF OPTiMiSATiON AND THE QUALiTY OF THESE COMPiLERS ARE A FAR CRY FROM MY NAiVE ViSiON OF BUiLDiNG COMPiLERS. SO YES, CURRENTLY COMPiLiNG THE FAMOUS `hello, world!` iS 20-80X FASTER THAN iN C OR RUST, BUT KEEP iN MiND THAT THE PiPELiNES ARE COMPLETELY DiFFERENT. iT'S BEST NOT TO POP THE CHAMPAGNE TO CELEBRATE ANYTHiNG YET. THERE iS SO MUCH STUFF TO DO. BUT THESE BENCHES GiVE US HOPE, AND THAT'S ALL THAT MATTERS TO STAY MOTiVATED.

## benchmark.

### benchmark — results.

#### hello.

RUN: `cargo run --release -p zo-benches -- hello`

__compile-time (ARM64)__

| Test Name       | Time (ms) | Notes              |
| :-------------- | :-------  | :----------------- |
| `c/hello.c`     | 47 – 54   | avg: 51ms, 6 lines |
| `rust/hello.rs` | 93 – 100  | avg: 95ms, 3 lines |
| `zo/hello.zo`   | 2 – 3     | avg: 2ms, 3 lines  |

__average execution time__

| Language | Avg Time (ms) |
| :------- | :------------ |
| `c`      | `51`          |
| `rust`   | `95`          |
| `zo`     | `2`           |

#### arithmetic.

RUN: `cargo run --release -p zo-benches -- arithmetic`

__compile-time (ARM64)__

| Test Name             | Time (ms) | Notes              |
| :-------------------- | :-------- | :----------------- |
| `c/arithmetic.c`      | 37 – 115  | avg: 53ms, 3 lines |
| `rust/arithmetic.rs`  | 84 – 88   | avg: 86ms, 3 lines |
| `zo/arithmetic.zo`    | 1 – 3     | avg: 1ms, 3 lines  |

__average execution time__

| Language | Avg Time (ms) |
| :------- | :------------ |
| `c`      | `53`          |
| `rust`   | `86`          |
| `zo`     | `1`           |

### benchmark — summary.

| Benchmark    | `zo` vs `c`      | `zo` vs `rust`   |
| ------------ | ---------------- | ---------------- |
| `hello`      | __25.5x__ faster | __47.5x__ faster |
| `arithmetic` | __53x__ faster   | __86x__ faster   |
