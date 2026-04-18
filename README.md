# zo.

[![invisage/zo](https://img.shields.io/badge/github-invisageable/zo-black?logo=github)](https://github.com/invisageable/zo)
![license: MIT/APACHE](https://img.shields.io/badge/license-MIT-blue?style=flat-square)
[![CI](https://github.com/invisageable/zo/workflows/CI/badge.svg)](https://github.com/invisageable/zo/actions)
[![Discord](https://img.shields.io/badge/discord-compilords-7289DA?logo=discord)](https://discord.gg/JaNc4Nk5xw)
---

> *The symphonie of compilers.*

[home](https://github.com/invisageable/zo) — [install](./crates/compiler/zo-notes/public/guidelines/02-install.md) — [how-to](./crates/compiler/zo-how-zo) — [tests](./crates/compiler/zo-tests) — [benches](./crates/compiler/zo-benches) — [speeches](./notes/speeches) — [license](#license)  

  <!-- pub $: {
    .count-value {
      c: navy;
      fs: 1rem;
      fw: bold;
    }
  } -->

  ```ts
  fun main() {
    mut count: int = 0;

    imu counter: </> ::= <>
      <button @click={fn() => count -= 1}>-</button>
      <span class="count-value">{count}</span>
      <button @click={fn() => count += 1}>+</button>
    </>;

    #dom counter;
  }
  ```

<!-- ## warning.

**wip wip wip wip wip wip wip wip wip wip wip wip wip wip wip wip wip wip.**

THiS iS A LiViNG PROJECT, FORGED iN THE OPEN. EXPECT ROUGH EDGES AND RADiCAL iDEAS.   

WE ARE BUiLDiNG THE CATHEDRAL. THE FOUNDATiON iS LAiD, BUT THE SPiRES ARE STiLL REACHiNG FOR THE SKY. iF YOU ARE A PiONEER, A BUiLDER, OR A FELLOW COMPiLER NERD, YOU'VE ARRiVED AT THE PERFECT TiME.   

JOiN THE DEVOLUTiON.  

**wip wip wip wip wip wip wip wip wip wip wip wip wip wip wip wip wip wip.**  

> *If you’re curious, do take the time to explore the project in its entirety.* -->

## about.

zo (PRONOUNCED `/zuː/`) iS A COMPiLER OF A COMPiLER iNSiDE ANOTHER GiANT COMPiLER THAT iS iTSELF iNSiDE A GiGANTiC COMPiLER.    

THE AiM OF THE PROJECT iS TO ENHANCE THE DEVELOPER EXPERiENCE, MAKiNG iT SEAMLESS TO BUiLD SOFTWARE THAT REFLECTS YOUR CREATiViTY. WE FOCUS ON DETAiLS THAT MATTER, OPENiNG NEW DiMENSiONS iN THE SOFTWARE UNiVERSE SPACE WHERE TRANSFORMiNG YOUR THOUGHTS iNTO PROGRAMS iS NOT JUST EASY, BUT ENJOYABLE.    

zo iS A COMPLETE ECOSYSTEM THAT ONLY GiVES YOU THE KEYS. YOU FiNALLY HAVE CONTROL OVER YOUR WORKSTATiON. YOU'LL NEVER HAVE TO WORK BLiND AGAiN. OUR TOOLS ARE DESiGNED TO PROViDE YOU WiTH ALL THE iNFORMATiON YOU NEED FOR YOUR PROGRAM, FROM DESiGN TO DELiVERY.   

OUR GOAL iS TO CREATE HiGH-PERFORMANCE, NEXT-GENERATiON SOFTWARE BY REDEFiNiNG USER iNTERFACES. OUR AiM iS NOT TO CREATE MERE TOOLS, BUT TO BE THE MAiN PLAYER iN THE DEVELOPER TOOLS OF TOMORROW ON A GLOBAL SCALE. WE ARE AGAiNST ABUNDANT SOFTWARE UNiFORMiTY. zo UNiFiES THE WEB AND THE GPU NOT BY FORCiNG THE WEB iNTO A CANVAS, BUT BY HARMONiZiNG FLEXBOX LAYOUTS WiTH RAW GPU POWER, WHiCH iS WHY WE WiLL DO EVERYTHiNG WE CAN TO PUSH THE BOUNDARiES OF iNNOVATiON TO THE LiMiT.

JOiN THE DEVOLUTiON.

## features.

**-developer-experience**

  - zsx (zo SYNTAX EXTENSiON) — *ui syntax inspired by `E4X`, fully `type-safe`.*
    - NATiVE TARGET — *zero-cost `gpu` rendering via `egui`.*
    - WEB TARGET — *optimized `js/dom` nodes (no canvas bloat) for seo & a11y.*
    - UNiFiED LAYOUT — *`flexbox` parity across `gpu` and `dom` via `taffy`.*
  - USER-FRiENDLY ERROR MESSAGES — *like elm, for faster debugging.*
  - EXPRESSiVE & CONCiSE — *syntax designed for readability.*
  - FULL-BATTERY TOOLS — *native REPL, code editor, package manager, etc.*

**-type-safety**

  - STATiCALLY, STRONGLY TYPED — *total control over your program from A to Z.*
  - METiCULOUS TYPE SYSTEM — *type inference, monomorphization, `typestate`.*
  - SAFE CONCURRENCY — *robust erlang-like actor model.*
  - META-LANGUAGE — *run code at `compile-time` via `#asm`, `#dom`, `#run` directives.*

**-performance-and-compilation**

  - HiGH-SPEED COMPiLATiON-TiME — *insanely faster, Usain Bolt would be jealous.*
  - ALGEBRAiC OPTiMiZATiON — *folding, propagation.*
  - TARGET SUPPORT — *`arm64-apple-darwin`, `arm64-unknown-linux-gnu`*.

## install.

  1. YOU JUST HAVE TO RUN:

  ```sh
  curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/invisageable/zo/main/tasks/zo-install.sh | sh
  ```

  2. THEN:

  ```
  zo x.x.x
  ```

  3. ET VOiLÀ! YOU ARE GOOD TO GO. FOR MORE DETAiLS:

  - @SEE — [`01-install`](./crates/compiler/zo-notes/public/guidelines/01-install.md)

## ecosystem.

THiS MONO-REPO POWERS AN ECOSYSTEM OF CRATES:

**-sources**

| NAME                                               | DESCRiPTiON                                                         |
| :------------------------------------------------- | :------------------------------------------------------------------ |
| [eazy](./sources/tweener/eazy)                     | THE HiGH-PERFORMANCE TWEENiNG & EASiNG FUNCTiONS KiT FOR ANiMATiON. |
| [swisskit](./sources/crafter/swisskit)             | THE SWiSS-ARMY-KNiFE KiT FOR WRiTiNG ROBUST PROGRAM.                |
| [tree-sitter-zo](./sources/crafter/tree-sitter-zo) | THE zo GRAMMAR FOR tree-sitter.                                     |

**-crates**

| NAME                           | DESCRiPTiON                                             |
| :----------------------------- | :------------------------------------------------------ |
| [fret](./crates/packager/fret) | THE PACKAGE MANAGER FOR THE `zo` PROGRAMMiNG LANGUAGE.   |
| [zo](./crates/compiler/zo)     | THE NEXT-GEN COMPiLER FOR THE `zo` PROGRAMMiNG LANGUAGE. |

> *More crates are coming. the architecture is modular and composable. Be gentle.*

## contributing.

WE LOVE CONTRiBUTORS. THiS iS A PLAYGROUND FOR COMPiLER __NERDS__, FRONTEND __HACKERS__, AND __CREATIVE__.    

FEEL FREE TO OPEN AN iSSUE iF YOU WANT TO CONTRiBUTE OR COME TO SAY HELLO ON [discord](https://discord.gg/JaNc4Nk5xw). ALSO YOU CAN CONTACT US AT `echo -n 'dGhlQGNvbXBpbG9yZHMuaG91c2U=' | base64 --decode`.   

## sponsors.

STARS, DONATiON AND SPONSORS ARE WELCOMiNG. SPREAD THE WORD e-ve-ry-where.    

## supports.

iF THiS PROJECT RESONATES WiTH YOU — PLEASE STAR iT. iT HELPS US GROW, ATTRACTS CONTRiBUTORS, AND VALiDATES THE DiRECTiON.    

## credits.

THANKS TO:    

[@ledruidd](https://github.com/ledruidd) [@SiegfriedEhret](https://github.com/SiegfriedEhret) [@akimd](https://github.com/akimd) [@graydon](https://github.com/graydon) [@rvirding](https://github.com/rvirding) [@worrydream](https://x.com/worrydream) [@j_blow](https://www.twitch.tv/j_blow) [@tsoding](https://x.com/tsoding) [@geohot](https://github.com/geohot) [@mike_acton](https://x.com/mike_acton)

> *« DE PRÈS COMME DE LOiN VOUS M'AVEZ iNSPiRÉ À MENER CE PROJET. TRiLU ! »* — *i10e*.    

## license.

[apache](./LICENSE-APACHE) — [mit](./LICENSE-MIT)

COPYRiGHT© **29** JULY **2024** — *PRESENT, [@invisageable](https://twitter.com/invisageable) — [@compilords](https://twitter.com/compilords) team.*
