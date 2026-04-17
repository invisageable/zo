# zo.

[![invisage/zo](https://img.shields.io/badge/github-invisageable/zo-black?logo=github)](https://github.com/invisageable/zo)
![license: MIT/APACHE](https://img.shields.io/badge/license-MIT-blue?style=flat-square)
[![CI](https://github.com/invisageable/zo/workflows/CI/badge.svg)](https://github.com/invisageable/zo/actions)
[![Discord](https://img.shields.io/badge/discord-compilords-7289DA?logo=discord)](https://discord.gg/JaNc4Nk5xw)
---

> *The symphonie of compilers.*

[home](https://github.com/invisageable/zo) — [install](./crates/compiler/zo-notes/public/guidelines/02-install.md) — [how-to](./crates/compiler/zo-how-zo) — [tests](./crates/compiler/zo-tests) — [benches](./crates/compiler/zo-benches) — [speeches](./notes/speeches) — [license](#license)  

  ```ts
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

## warning.

**wip wip wip wip wip wip wip wip wip wip wip wip wip wip wip wip wip wip.**

THiS iS A LiViNG PROJECT, FORGED iN THE OPEN. EXPECT ROUGH EDGES AND RADiCAL iDEAS.   

WE ARE BUiLDiNG THE CATHEDRAL. THE FOUNDATiON iS LAiD, BUT THE SPiRES ARE STiLL REACHiNG FOR THE SKY. iF YOU ARE A PiONEER, A BUiLDER, OR A FELLOW COMPiLER NERD, YOU'VE ARRiVED AT THE PERFECT TiME.   

JOiN THE DEVOLUTiON.  

**wip wip wip wip wip wip wip wip wip wip wip wip wip wip wip wip wip wip.**  

> *If you’re curious, do take the time to explore the project in its entirety.*

## about.

zo (PRONOUNCED `/zuː/`) iS A COMPiLER OF A COMPiLER iNSiDE ANOTHER GiANT COMPiLER THAT iS iTSELF iNSiDE A GiGANTiC COMPiLER.    

THE AiM OF THE PROJECT iS TO ENHANCE THE DEVELOPER EXPERiENCE, MAKiNG iT SEAMLESS TO BUiLD SOFTWARE THAT REFLECTS YOUR CREATiViTY. WE FOCUS ON DETAiLS THAT MATTER, OPENiNG NEW DiMENSiONS iN THE SOFTWARE UNiVERSE SPACE WHERE TRANSFORMiNG YOUR THOUGHTS iNTO PROGRAMS iS NOT JUST EASY, BUT ENJOYABLE.    

zo iS A COMPLETE ECOSYSTEM THAT ONLY GiVES YOU THE KEYS. YOU FiNALLY HAVE CONTROL OVER YOUR WORKSTATiON. YOU'LL NEVER HAVE TO WORK BLiND AGAiN. OUR TOOLS ARE DESiGNED TO PROViDE YOU WiTH ALL THE iNFORMATiON YOU NEED FOR YOUR PROGRAM, FROM DESiGN TO DELiVERY.   

OUR GOAL iS TO CREATE HiGH-PERFORMANCE, NEXT-GENERATiON SOFTWARE BY REDEFiNiNG USER iNTERFACES. OUR AiM iS NOT TO CREATE MERE TOOLS, BUT TO BE THE MAiN PLAYER iN THE DEVELOPER TOOLS OF TOMORROW ON A GLOBAL SCALE. WE ARE AGAiNST ABUNDANT SOFTWARE UNiFORMiTY, WHiCH iS WHY WE WiLL DO EVERYTHiNG WE CAN TO PUSH THE BOUNDARiES OF iNNOVATiON TO THE LiMiT.

JOiN THE DEVOLUTiON.

## features.

**-developer-experience**
  - zsx (zo syntax extension) — *ui syntax inspired by `E4X` but fully `type-safe`.*
    - build native apps — *`gpu` (egui) and `js` (wry) with a single codebase.*
  - user-friendly `error` messages — *like elm, for faster debugging.*
  - expressive and concise — *syntax designed for readability.*
  - full-battery `tools` — *native REPL, code editor, package manager, etc.*

**-type-safety**
  - statically, strongly typed — *total control over your program from A to Z.*
  - meticulous `type system` — *type inference, monomorphization, `typestate`.*
  - safe concurrency — *robust erlang-like actor model.*
  - meta-language — *run code at compile-time via `#asm`, `#dom`, `#run` (directives).*

**-performance-and-compilation**
  - high-speed `compilation-time` — *insanely faster, Usain Bolt would be jealous.*
  - algebraic `optimization` — *folding, propagation.*
  - target support — *`arm64-apple-darwin`, `arm64-unknown-linux-gnu`*.

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

> *More crates are coming. the architecture is modular and composable. be gentle.*

## goals.

iF YOU ARE THiNKiNG WHY?! HERE iS OUR ANSWERS...

  - GPU RENDERiNG <sup>egui</sup> (NATiVE APP ONLY) — *low memory footprint, zero runtime fluff*.
  - BYE-BYE `electron` (WEB APP ONLY) — *building lightweight web apps without the bloat.*.

JOiN THE DEVOLUTiON.

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

> « DE PRÈS COMME DE LOiN VOUS M'AVEZ iNSPiRÉ À MENER CE PROJET. TRiLU ! » — *i10e*.    

## license.

[apache](./LICENSE-APACHE) — [mit](./LICENSE-MIT)

COPYRiGHT© **29** JULY **2024** — *PRESENT, [@invisageable](https://twitter.com/invisageable) — [@compilords](https://twitter.com/compilords) team.*
