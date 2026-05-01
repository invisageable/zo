# zo.

[![invisage/zo](https://img.shields.io/badge/github-invisageable/zo-black?logo=github)](https://github.com/invisageable/zo)
![license: MIT/APACHE](https://img.shields.io/badge/license-MIT-blue?style=flat-square)
[![CI](https://github.com/invisageable/zo/workflows/CI/badge.svg)](https://github.com/invisageable/zo/actions)
[![Discord](https://img.shields.io/badge/discord-compilords-7289DA?logo=discord)](https://discord.gg/JaNc4Nk5xw)
---

> *Turn your thoughts into type-safe software and Ui instantly.*

[home](https://github.com/invisageable/zo) — [install](#get-started) — [how-to](./crates/compiler/zo-how-zo) — [tests](./crates/compiler/zo-tests) — [benches](./crates/compiler/zo-benches) — [speeches](./notes/speeches) — [license](#license)

<!-- [zo.com](https://zo.com) -->

## usage.

**-zsx-counter**

  ```zo
  fun main() {
    mut count: int = 0;

    imu counter: </> ::= <>
      <button @click={fn() => count -= 1}>-</button>
      {count}
      <button @click={fn() => count += 1}>+</button>
    </>;

    #dom counter;
  }
  ```

**-concurrency**

  ```zo
  fun producer_a(tx: Tx<int>) { tx.send(10); }
  fun producer_b(tx: Tx<int>) { tx.send(20); }                                   
                                                                                  
  fun main() {                                                                   
    nursery {                                                                    
      imu (tx1, rx1) := channel(1);
      imu (tx2, rx2) := channel(1);                                              
    
      spawn producer_a(tx1);                                                     
      spawn producer_b(tx2);
                                                                                  
      select {    
        rx1 => fn(value: int) => showln("chan1: {value}"),
        rx2 => fn(value: int) => showln("chan2: {value}"),                       
      }
    }                                                                            
  }
  ```

ONE LANGUAGE. ONE COMPiLER. ONE BiNARY. ONE WiNDOW. NATiVE GPU OR THE WEB — SAME SOURCE.

## why zo?

zo iS BUiLT FROM THE GROUND UP USiNG DATA-ORiENTED DESiGN. BY HAND-ROLLiNG THE COMPiLER STAGES AND EMiTTiNG MACHiNE CODE DiRECTLY, zo ELiMiNATES THE OVERHEAD OF HEAVY ABSTRACTiONS AND EXTERNAL LiNKERS.

> *« Rust makes you wait. C makes you think. zo just lets you build. » — i10e*

### benchmarks.

*Workload: 10,000 lines of source code compiled to native ARM64 binary (including link step).*

| Compiler  | Hot Average | Throughput      | vs zo             |
| :-------- | :---------- | :-------------- | :---------------- |
| **zo**    | **60 ms**   | **~167K LoC/s** | **1× (baseline)** |
| **clang** | 148 ms      | ~67K LoC/s      | 2.4× slower       |
| **rustc** | 321 ms      | ~31K LoC/s      | 5.3× slower       |

[@methodology-and-full-numbers](./crates/compiler/zo-benches)

### our pipeline.

iN THOSE 60 MiLLiSECONDS, THE COMPiLER PERFORMS THE FOLLOWiNG PHASES SEQUENTiALLY:

  1. TOKENiZiNG — *Processes raw text into tokens at ~13M LoC/s.*
  2. PARSiNG — *Builds the parse tree using a custom, cache-friendly parser.*
  3. ANALYZiNG — *Performs Hindley-Milner type inference, monomorphization, and type checking.*
  4. OPTiMiZiNG — *Executes algebraic optimizations (constant folding, propagation, dce).*
  5. CODEGEN & LiNK — *Emits direct machine code and creates the final binary.*

> *« Insanely faster, Usain Bolt would be jealous. » — i2N*

## features.

  - **UNiFiED Ui DEVELOPMENT**
    - WRiTE USER iNTERFACES ONCE — TARGET NATiVE <sup>EGUi</sup> OR THE WEB <sup>JS/DOM</sup>.
    - FLEXBOX PARiTY ACROSS GPU AND DOM ViA TAFFY — ONE LAYOUT MODEL, TWO BACKENDS.

  - **iNSTANT FEEDBACK LOOP**
    - COMPiLE 10,000 LiNES iN MiLLiSECONDS — CODE WiTHOUT WAiTiNG FOR THE COMPiLER.
    - USER-FRiENDLY ERROR MESSAGES — LiKE elm, FOR FASTER DEBUGGiNG.

  - **EXPRESSiVE & SAFE SYNTAX**
    - STATiCALLY & STRONGLY TYPED — TOTAL CONTROL OVER YOUR PROGRAM FROM A TO Z.
    - METiCULOUS TYPE SYSTEM — TYPE iNFERENCE, MONOMORPHiZATiON, TYPESTATE.
    - SAFE CONCURRENCY — GREEN AND OS THREADS LiKE Go, WRAPPED iN NURSERY TASK SCOPES. <sup>NO LEAKED THREADS, NO DATA RACES</sup>.
    - META-LANGUAGE — RUN AT COMPiLE-TiME ViA `#asm`, `#dom`, `#run` DiRECTiVES.

  - **FULL TOOLKiT iNCLUDED**
    - PACKAGE MANAGER <sup><a href="./crates/packager/fret">fret</a></sup>, TEXT EDiTOR <sup><a href="https://github.com/invisageable/codelord">codelord</a></sup>, NATiVE BUiLD TOOLS — KEEPS YOUR WORKSTATiON LiGHTWEiGHT.
    - TARGET SUPPORT — `arm64`, `x86_64` FOR `linux`, `macos`, `windows`.

## get started.

  1. RUN THE iNSTALLATiON SCRiPT:

  ```sh
  curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/invisageable/zo/main/tasks/zo-install.sh | sh
  ```

  2. VERiFY:

  ```
  zo --version
  ```

  3. iT WiLL PRiNT:

  ```
  zo x.x.x
  ```

> *Note: zo is entirely self-contained. It requires no heavy external frameworks.*

  1. ET VOiLÀ! HEAD OVER TO [@how-to](./crates/compiler/zo-how-zo) — THE EASiEST WAY TO GET THE BASiCS OF zo.

iF YOU ENCOUNTER iSSUES, CHECK THE GUiDE:

  - @SEE — [`01-install`](./crates/compiler/zo-notes/public/guidelines/01-install.md)

## ecosystem.

THiS MONO-REPO POWERS AN ECOSYSTEM OF CRATES:

**-sources**

| NAME                                               | DESCRiPTiON                                                         |
| :------------------------------------------------- | :------------------------------------------------------------------ |
| [eazy](./sources/tweener/eazy)                     | THE HiGH-PERFORMANCE TWEENiNG & EASiNG FUNCTiONS KiT FOR ANiMATiON. |
| [swisskit](./sources/crafter/swisskit)             | THE SWiSS-ARMY-KNiFE KiT FOR WRiTiNG ROBUST PROGRAMS.                |
| [tree-sitter-zo](./sources/crafter/tree-sitter-zo) | THE zo GRAMMAR FOR tree-sitter.                                     |

**-crates**

| NAME                                         | DESCRiPTiON                                              |
| :------------------------------------------- | :------------------------------------------------------- |
| [fret](./crates/packager/fret)               | THE PACKAGE MANAGER FOR THE `zo` PROGRAMMiNG LANGUAGE.   |
| [fret-vscode](./crates/packager/fret-vscode) | THE VS CODE EXTENSiON FOR `fret` MANiFEST FiLES.         |
| [zo](./crates/compiler/zo)                   | THE NEXT-GEN COMPiLER FOR THE `zo` PROGRAMMiNG LANGUAGE. |
| [zo-vscode](./crates/compiler/zo-vscode)     | THE VS CODE EXTENSiON FOR THE `zo` PROGRAMMiNG LANGUAGE. |

> *More crates are coming. The architecture is modular and composable. Be gentle.*

## the manifesto.

zo iS A COMPiLER OF A COMPiLER iNSiDE ANOTHER GiANT COMPiLER THAT iS iTSELF iNSiDE A GiGANTiC COMPiLER.

THE AiM OF THE PROJECT iS TO ENHANCE THE DEVELOPER EXPERiENCE, MAKiNG iT SEAMLESS TO BUiLD SOFTWARE THAT REFLECTS YOUR CREATiViTY. WE FOCUS ON DETAiLS THAT MATTER, OPENiNG NEW DiMENSiONS iN THE SOFTWARE UNiVERSE WHERE TRANSFORMiNG YOUR THOUGHTS iNTO PROGRAMS iS NOT JUST EASY, BUT ENJOYABLE.

zo iS A COMPLETE ECOSYSTEM THAT GiVES YOU THE KEYS. YOU FiNALLY HAVE CONTROL OVER YOUR WORKSTATiON. YOU'LL NEVER HAVE TO WORK BLiND AGAiN. OUR TOOLS PROViDE ALL THE iNFORMATiON YOU NEED FOR YOUR PROGRAM, FROM DESiGN TO DELiVERY.

WE ARE AGAiNST ABUNDANT SOFTWARE UNiFORMiTY. zo UNiFiES THE WEB AND THE GPU — NOT BY FORCiNG THE WEB iNTO A CANVAS, BUT BY HARMONiZiNG FLEXBOX LAYOUTS WiTH RAW GPU POWER. WE WiLL DO EVERYTHiNG WE CAN TO PUSH THE BOUNDARiES OF iNNOVATiON TO THE LiMiT.

**JOiN THE DEVOLUTiON.**

## contributing.

WE LOVE CONTRiBUTORS. THiS iS A PLAYGROUND FOR COMPiLER __NERDS__, FRONTEND __HACKERS__, AND __CREATIVES__.

OPEN AN iSSUE, OR COME SAY HELLO ON [discord](https://discord.gg/JaNc4Nk5xw). YOU CAN ALSO CONTACT US AT `echo -n 'dGhlQGNvbXBpbG9yZHMuaG91c2U=' | base64 --decode`.

## sponsors & supports.

STARS, DONATiONS AND SPONSORS ARE WELCOME. SPREAD THE WORD e-ve-ry-where.

iF THiS PROJECT RESONATES WiTH YOU — PLEASE STAR iT. iT HELPS US GROW, ATTRACTS CONTRiBUTORS, AND VALiDATES THE DiRECTiON.

## credits.

THANKS TO:

[@ledruidd](https://github.com/ledruidd) [@SiegfriedEhret](https://github.com/SiegfriedEhret) [@akimd](https://github.com/akimd) [@graydon](https://github.com/graydon) [@rvirding](https://github.com/rvirding) [@worrydream](https://x.com/worrydream) [@j_blow](https://www.twitch.tv/j_blow) [@tsoding](https://x.com/tsoding) [@geohot](https://github.com/geohot) [@mike_acton](https://x.com/mike_acton)

> *« Merci à vous pour l'inspiration. TRiLU ! » — i10e*

## license.

[apache](./LICENSE-APACHE) — [mit](./LICENSE-MIT)

COPYRiGHT© **29** JULY **2024** — *PRESENT, [@invisageable](https://twitter.com/invisageable) — [@compilords](https://twitter.com/compilords) team.*
