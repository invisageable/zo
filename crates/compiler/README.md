# crates — compiler.

```
[zo@compiler] compiling...
✓ read 7M lines.
✓ processed 41M tokens.
✓ parsed 3M nodes.
✓ annotated 3M nodes.
✓ generated `n` artifacts.
✓ linked `n` artifacts.
✓ total in 1.341838 seconds.

⚡ speed: 5.22M LoC/s.
```

> *become the __programmer__ you __think__ you are*.

## about.

zo iS A HYBRiD PROGRAMMiNG LANGUAGE THAT i'VE HAD iN MiND FOR SEVERAL YEARS. THE iDEA iS TO COMBiNE __PROGRAMMiNG__ AND __TEMPLATiNG__ TO CREATE __DESKTOP__ AND __WEB__ APPLiCATiONS.    

zo iS A NEW, GENERAL-PURPOSE PROGRAMMiNG LANGUAGE DESiGNED FROM FiRST PRiNCiPLES FOR THE NEXT-GEN OF CREATiVE AND iNTERACTiVE SOFTWARE. iT iS A MULTi-PARADiGM LANGUAGE, SEAMLESSLY BLENDiNG HiGH-PERFORMANCE SYSTEMS PROGRAMMiNG WiTH A HiGH-LEVEL, DECLARATiVE, AND REACTiVE Ui FRAMEWORK.   

zo GiVES YOU BACK CONTROL, GUARANTEES STRONG STATiC TYPiNG WiTH EXCEPTiONAL EXECUTiON SPEED. WHETHER YOU'RE A __CREATiVE__, A __HACKER__, A __CODE GOLFER__ OR A __PROGRAMMER__, AS LONG AS YOU'RE PASSiONATE ABOUT WRiTiNG ROBUST SOFTWARE, YOU'LL FiND iN zo — THE DARK SiDE OF THE FORCE.   

iN SHORT, zo iS THE FAVOURiTE LANGUAGE OF YOUR FAVOURiTE LANGUAGE.         
    
## goals.

- [x] statically, strongly typed.
- [ ] meticulous `type system` — *type checking, inference, type state*.
- [ ] algebraic `optimization` — *folding, propagation*.
- [x] user-friendly `error` messages — *like elm*.
- [ ] target support — *`arm64-apple-darwin`, `arm64-unknown-linux-gnu`*.
- [x] meta-language — *`#asm`, `#dom`, `#run` (directives)*.
- [x] templating syntax — *like the abandoned `E4X`*.
- [x] build native apps — *`gpu` (egui) and `wasm` (web-sys)*.
- [ ] safe concurrency model — *actor model erlang-like*.
- [x] fast `compilation-time` — *insanely faster, usain is jealous*.
- [ ] powerful `tools` — *native REPL, code editor, packager, etc*.

## what's next?

iF YOU'RE READiNG THiS, YOU'RE EARLY. COME BACK SOON. OR BETTER — *stay*.

WE SHARED THE SAME ENGiNEERiNG PHiLOSOPHY THAN __JONATHAN BLOW__, __MiKE ACTON__, __CHANDLER CARRUTH__, __GRAYDON HOARE__ AND __BRET ViCTOR__.

> *be ahead, JOiN THE DEVOLUTiON.*s

## commands.

```bash
# programming mode.
cargo run --bin zo -- build crates/compiler/zo-tests/build-pass/programming/hello.zo -o crates/compiler/zo-tests/build-pass/programming/hello
# template mode.
cargo run --bin zo -- run crates/compiler/zo-tests/build-pass/templating/hello.zo
```

## benchmarks.

### hello world compilation speed.

| Compiler | Run 1 | Run 2 | Run 3 | Run 4 | Run 5 | Average     | Speed vs zo      |
| :------- | :---- |:----- | :---- | :---- | :---- | :---------- | :--------------- |
| **zo**   | 16ms  | 6ms   | 5ms   | 6ms   | 6ms   | **7.8ms**   | **1.0x**         |
| clang    | 122ms | 44ms  | 43ms  | 43ms  | 42ms  | **58.8ms**  | **7.5x slower**  |
| rustc    | 910ms | 93ms  | 84ms  | 81ms  | 84ms  | **250.4ms** | **32.1x slower** |

**AWTY — Are we tiny yet?**

| Compiler | Size   |
| :------- | :----- |
| **zo**   | 33 KB  |
| clang    | 33 KB  |
| rustc    | 441 KB |

*Test program: `showln("Hello, World!");` (3 lines)*

### arithmetic operations compilation speed.

| Compiler | Run 1 | Run 2 | Run 3 | Run 4 | Run 5 | Average     | Speed vs zo      |
| :------- | :---- |:----- | :---- | :---- | :---- | :---------- | :--------------- |
| **zo**   | 18ms  | 10ms  | 12ms  | 8ms   | 5ms   | **10.6ms**  | **1.0x**         |
| clang    | 94ms  | 44ms  | 44ms  | 42ms  | 42ms  | **53.2ms**  | **5.0x slower**  |
| rustc    | 910ms | 95ms  | 90ms  | 89ms  | 90ms  | **254.8ms** | **24.0x slower** |

**AWTY — Are we tiny yet?**

| Compiler | Size   |
| :------- | :----- |
| clang    | 17 KB  |
| **zo**   | 33 KB  |
| rustc    | 439 KB |

*Test program: `return 10 + 20 * 3 - 15;` (Result: 55)*

### bench summary.

- zo achieves **sub-10ms compilation** for typical programs.
- zo is **5-7x faster** than `clang` (`c` compiler).
- zo is **24-32x faster** than `rustc` (`rust` compiler).

## examples.

**[-hello](./zo-samples/examples/hello.zo)**

THE SUPERSTAR [`hello, world!`](https://en.wikipedia.org/wiki/%22Hello,_World!%22_program) — *PRiNTS `hello, world!`.*

![hello](./zo-notes/public/preview/preview-zo-hello.png)

> « The universe needs to preserve guys like Graydon Hoare! » — *because they challenge us to reject "accepted" solutions and build something better*.
