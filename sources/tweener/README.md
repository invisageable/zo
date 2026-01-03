# sources — tweener.

[![invisage/zov](https://img.shields.io/badge/github-invisageable/zov-black?logo=github)](https://github.com/invisageable/zov)
[![Latest version](https://img.shields.io/crates/v/eazy.svg)](https://crates.io/crates/eazy)
[![Documentation](https://docs.rs/eazy/badge.svg)](https://docs.rs/eazy)
![license: APACHE](https://img.shields.io/badge/license-APACHE-blue?style=flat-square)
![license: MIT](https://img.shields.io/badge/license-MIT-blue?style=flat-square)

---

```
[zo@eazy] booting...
✓ loading easing curves...
✓ registered 96 profiles
✓ optimized easing functions
✓ vectorizing interpolations...
✓ compiling benchmarks...
✓ ready to smooth your pixels
✓ done in 0.0026s
```

> *eazy — THE TWEENiNG & EASiNG FUNCTiONS KiT FOR HiGH-PERFORMACE ANiMATiON.*

[Home](.)

## about.

eazy iS AMONG THE FASTEST, ATOMiC AND ENHANCED `easing functions` KiT WRiTTEN iN RUST FOR CREATiVE, GAMERS, PROGRAMMERS, SCiENTiSTS, ETC WHO CARES ABOUT METRiCS — @see [benchmark](../eazy-notes/docs/benchmark.md).    

eazy iS METiCULOUSLY OPTiMiZED iT'S A PERFECT MATCH FOR PRECiSiON OR SOPHiSTiCATED PROGRAMS — SUCH AS GUi, GAME ENGiNE, PLOTS, WEB AND NATiVE APPLiCATiON.    

> OK-AY, OK-AY — BUT WHY eazy iS OUTPERFORMiNG?

- MiNiMAL OVERHEAD ABSTRACTiON — *inlining, zero vtable lookups.*
- MATHEMATICALLY EFFiCiENT — *no branches, no inttermediate allocations.*
- PROPER BENCHMARK DiSCiPLiNE — *performance matter.*

## example.

```rs
use eazy::Curve;
use eazy::easing::oscillatory::bounce::OutBounce;

fn main() {
  for time in (0..=100).map(|x| x as f32 / 100.0) {
    let bounce = OutBounce.y(time);

    println!("reaction bounce: {bounce:.3}");
  }
}
```

MORE EXAMPLES [`here`](../eazy-examples).    

## embbedable.

YOU ARE A bevy GAMER USER? @see — [eazy-bevy](../eazy-bevy).    
YOU ARE A egui CRAFTER USER? @see — [eazy-egui](../eazy-egui).    
YOU ARE A wasm HACKER USER? @see — [eazy-wasm](../eazy-wasm).   
YOU ARE A low-level PROGRAMMER USER? @see — [eazy](../eazy).    

> those contains everything you need to use eazy in without changing your paradigme.

## functions.

**easing.**

- POLYNOMiAL — _linear, quadratic, cubic, quartic, quintic, sextic, septic, octic, monic, decic, hectic_.
- TRiGONOMETRiC — _sine, circle_.
- EXPONENTiAL — _expo2, expoe_.
- LOGARiTHMiC — _log10_.
- ROOT — _sqrt_.
- OSCiLLATORY — _elastic, bounce_.
- BACKTRACKiNG — _back_.

**interpolation.**

- LiNEAR — _lerp_.
- POLYNOMiAL — _smoothstep, smootherstep, cubic, quartic, quintic, lagrange, newton_.
- RATiONAL — _quadratic, cubic_.
- PiECEWiZE — _polynomial, quadratic_.
- TRiGONOMETRiC — _sinusoidal_.

## benches.

> *beat'em up!*

BENCHES ARE DONE iN COMPARiSON BETWEEN `bevy_tween`, `easings`, `emath` (FROM `egui`) , `glissade`, `interpolation`, `keyframe`, `simple-easing2` CRATES. MOST OF THEM ARE FOLLOW THE ROBERT PENNER'S EASiNG FuNCTiONS, THEY ONLY iMPLEMENTED THE BASiCS ONE. REGARDiNG PERFORMANCE SOME OF OuR iMPLEMENTATiONS ARE SLiGHTLY FASTER AND STABLE, SO DEPENDiNG YOUR NEEDED, YOu SHOuLD TRY eazy. THE SAMPLE BELOW CONFiRM THAT OuR EASiNG FuNCTiONS ARE PRETTY WELL OPTiMiZED.

![bench-in-back-average-time](./eazy-notes/assets/image/benchmark/bench-average-time-in-back.png)

**what's next?**

- COMPARE WiTH OTHERS LANGUAGES — *`js`, `python`, `c++`, `c#`.*

## contributing.

WE LOVE CONTRiBUTORS.   

FEEL FREE TO OPEN AN iSSUE iF YOU WANT TO CONTRiBUTE OR COME TO SAY HELLO ON [discord](https://discord.gg/JaNc4Nk5xw). ALSO YOU CAN CONTACT US AT THE [at] COMPiLORDS [dot] HOUSE. THiS iS A PLAYGROUND FOR COMPiLER __NERDS__, FRONTEND __HACKERS__, AND __CREATIVE__.    

## license.

[APACHE](https://github.com/invisageable/zov/blob/main/.github/LICENSE-APACHE) — [MIT](https://github.com/invisageable/zov/blob/main/.github/LICENSE-MIT)   

COPYRiGHT© **10** JULY **2024** — *PRESENT, [@invisageable](https://github.com/invisageable).*     
