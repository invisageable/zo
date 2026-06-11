# zo.

  ```
  [zo] lines processed (including blank lines and comments) — 499998.
  │
  ├── "Why accept slow compilers? Just make them faster." — Jonathan Blow
  │
  ├── ✓ [zo@front-end] time — 301.591 ms (42.5%).
  │   ├── ⏺ [zo@tokenizer] time — 57.064 ms (8.0%).
  │   │   └── ⏺ processed — 2399990 tokens.
  │   ├── ⏺ [zo@parser] time — 36.126 ms (5.1%).
  │   │   └── ⏺ parsed — 2350646 nodes.
  │   └── ⏺ [zo@analyzer] time — 208.401 ms (29.3%).
  │       └── ⏺ annotated — 349996 nodes.
  ├── ✓ [zo@back-end] time — 408.614 ms (57.5%).
  │   ├── ⏺ [zo@codegen:arm64-apple-darwin] time — 391.537 ms (55.1%).
  │   │   └── ⏺ generated — 1 artifacts.
  │   └── ⏺ [zo@linker] time — 17.076 ms (2.4%).
  │       └── ⏺ linked — 1 files.
  └── ✓ [zo@total] time — 710.205 ms (100.0%).

  ⚡ speed: 704.02K LoC/s.
  ```

[![CI](https://github.com/invisageable/zo/workflows/CI/badge.svg)](https://github.com/invisageable/zo/actions)
[![Discord](https://img.shields.io/badge/discord-compilords-7289DA?logo=discord)](https://discord.gg/JaNc4Nk5xw)
---

> *Turn your thoughts into type-safe software and Ui instantly.*

THE AiM OF THE PROJECT iS TO ENHANCE THE DEVELOPER EXPERiENCE, MAKiNG iT SEAMLESS TO BUiLD SOFTWARE THAT REFLECTS YOUR CREATiViTY. WE FOCUS ON DETAiLS THAT MATTER, WHERE TRANSFORMiNG YOUR THOUGHTS iNTO PROGRAMS iS NOT JUST EASY, BUT ENJOYABLE.

zo (pronounced `/zuː/` just like "zoo") iS A SiMPLE, LiGHTWEiGHT, CROSS-PLATFORM, GENERAL-PURPOSE PROGRAMMiNG LANGUAGE. TO SHiP, RUN AND BUiLD TYPED-SAFE DESKTOP, MOBiLE AND WEB APPLiCATiONS WiTH ONE CODE SOURCE. THE CORE LiBRARY iNCLUDES SEVERAL PACKAGES. PROViDERS ARE AVAiLABLE TO EXPAND THE LANGUAGE's CAPABiLiTiES.

**JOiN THE DEVOLUTiON.**

[home](https://zo.compilords.house) — [install](./crates/compiler/zo-notes/public/guidelines/01-install.md) — [initiation](https://zo.compilords.house/initiation) — [news](https://zo.compilords.house/news) — [discord](https://discord.gg/JaNc4Nk5xw)

## usage.

THiS PROGRAM DECLARES A COMPONENT (`counter`) COMPOSED BY TWO BUTTONS (`<button>`) AND A TEXT-BiNDiNG (`{count}`) ASSiGNED TO `0` BY DEFAULT. EACH BUTTONS CONTAiNS AN EVENT (`@click`), ON CLiCK, iT TRiGGERS AND EXECUTE AN ACTiON TO DECREASE OR iNCREASE THE `count` VALUE. iT THEN RENDERS THE COMPONENT ViA A DiRECTiVE (`#render`).

  ```zo
  fun main() {
    mut count: int = 0;

    imu counter: </> ::= <>
      <button @click={fn() => count -= 1}>-</button>
      {count}
      <button @click={fn() => count += 1}>+</button>
    </>;

    #render counter;
  }
  ```

ONE LANGUAGE. ONE COMPiLER. ONE BiNARY. ONE WiNDOW. ALL PLATFORMS — SAME SOURCE.

---

<p align="center">
  <img width="324px" src="crates/compiler/zo-notes/public/assets/preview/preview-zo-zsx-desktop-counter.png">
  <img width="324px" src="crates/compiler/zo-notes/public/assets/preview/preview-zo-zsx-webview-counter.png">
  <img width="324px" src="crates/compiler/zo-notes/public/assets/preview/preview-zo-zsx-vision-home.png">
  <img width="324px" src="crates/compiler/zo-notes/public/assets/preview/preview-zo-zsx-vision-counter.png">
  <img width="324px" src="crates/compiler/zo-notes/public/assets/preview/preview-zo-zsx-watch-home.png">
  <img width="324px" src="crates/compiler/zo-notes/public/assets/preview/preview-zo-zsx-watch-counter.png">
  <img width="324px" src="crates/compiler/zo-notes/public/assets/preview/preview-zo-zsx-iphone-home.png">
  <img width="324px" src="crates/compiler/zo-notes/public/assets/preview/preview-zo-zsx-iphone-counter.png">
</p>

---

DEV MACHiNE:

```
Operating System  — Darwin 26.5.1 (ARM64)
Kernel Version    — 25.5.0
CPU               — Apple M3 Pro (12 cores)
Total Memory      — 18.0 GB
Available Memory  — 9.4 GB
```

> *zo is in early development and not ready for production yet. Currently it supports desktop (ARM64), MacOS (iOS, tvOS, visionOS, watchOS), web (bundled or webview). We plan to supports more — desktop (Linux, Windows) and mobile (Android). Styling is not already unified between all platforms for now.*
>
> *WARNiNG — regarding Ai usage, we are using Ai to build based on our architecture and specification (made by humans). The compiler currently covers over 1500 unit and integration tests.*

## sponsors & supports.

STARS, DONATiONS AND SPONSORS ARE WELCOME. SPREAD THE WORD e-ve-ry-where.

iF THiS PROJECT RESONATES WiTH YOU — PLEASE STAR iT. iT HELPS US GROW, ATTRACTS CONTRiBUTORS, AND VALiDATES THE DiRECTiON.

## credits.

THANKS TO:

[@ledruidd](https://github.com/ledruidd) [@SiegfriedEhret](https://github.com/SiegfriedEhret) [@akimd](https://github.com/akimd) [@graydon](https://github.com/graydon) [@rvirding](https://github.com/rvirding) [@worrydream](https://x.com/worrydream) [@j_blow](https://www.twitch.tv/j_blow) [@tsoding](https://x.com/tsoding) [@geohot](https://github.com/geohot) [@mike_acton](https://x.com/mike_acton)

> *« Merci à vous pour le turfu. TRiLU ! » — i10e*

## license.

[apache](./LICENSE-APACHE) — [mit](./LICENSE-MIT)

COPYRiGHT© **29** JULY **2024** — *PRESENT, [@invisageable](https://twitter.com/invisageable) ([@compilords](https://twitter.com/compilords)).*
