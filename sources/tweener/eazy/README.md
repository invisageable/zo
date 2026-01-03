# eazy.

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

> *eazy — THE BLAZiNGLY FAST AND MATHEMATiCALLY OPTiMiZED `EASiNG FUNCTiONS` KiT.*

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

`cargo run -p eazy --example derive-tweenable`

MORE EXAMPLES [`here`](../eazy-examples).    

## embbedable.

YOU ARE A bevy GAMER USER? @see — [eazy-bevy](../eazy-bevy).    
YOU ARE A egui CRAFTER USER? @see — [eazy-egui](../eazy-egui).    
YOU ARE A wasm HACKER USER? @see — [eazy-wasm](../eazy-wasm).   
YOU ARE A low-level PROGRAMMER USER? @see — [eazy-wasm](../eazy-wasm).    

> those contains everything you need to use eazy in without changing your paradigme.

## functions.

**easing.**

- [x] POLYNOMiAL.
  - [x] LiNEAR.
  - [x] QuADRATiC.
  - [x] CuBiC.
  - [x] QuARTiC.
  - [x] QuiNTiC.
  - [x] SEXTiC.
  - [x] SEPTiC.
  - [x] OCTiC.
  - [x] NONiC.
  - [x] DECiC.
  - [x] HECTiC.

- [x] TRiGONOMETRiC.
  - [x] SiNE.
  - [x] CiRCLE.

- [x] EXPONENTiAL.
  - [x] EXPO2.
  - [x] EXPOE.

- [x] LOGARiTHMiC.
  - [x] LOG10.
- [x] ROOT.
  - [x] SQRT.

- [x] OSCiLLATORY.
  - [x] ELASTiC.
  - [x] BOuNCE.

- [x] BACKTRACKiNG.
  - [x] BACK.

**interpolation.**

- [x] LiNEAR.
  - [x] LERP.
  
- [x] POLYNOMiAL.
  - [x] SMOOTHSTEP.
  - [x] SMOOTHERSTEP
  - [x] CuBiC.
  - [x] QuARTiC.
  - [x] QuiNTiC.
  - [ ] LAGRANGE.
  - [ ] NEWTON.

- [x] RATiONAL.
  - [x] QuADRATiC
  - [x] CuBiC.

- [x] PiECEWiZE.
  - [ ] POLYNOMiAL
  - [x] QuADRATiC

- [x] TRiGONOMETRiC
  - [x] SiNuSOiDAL.

## benches.

> *beat'em up!*

BENCHES ARE DONE iN COMPARiSON BETWEEN `bevy_tween`, `easings`, `emath` (FROM `egui`) , `glissade`, `interpolation`, `keyframe`, `simple-easing2` CRATES. MOST OF THEM ARE FOLLOW THE ROBERT PENNER'S EASiNG FuNCTiONS, THEY ONLY iMPLEMENTED THE BASiCS ONE. REGARDiNG PERFORMANCE SOME OF OuR iMPLEMENTATiONS ARE SLiGHTLY FASTER AND STABLE, SO DEPENDiNG YOUR NEEDED, YOu SHOuLD TRY eazy. THE SAMPLE BELOW CONFiRM THAT OuR EASiNG FuNCTiONS ARE PRETTY WELL OPTiMiZED.

![bench-in-back-average-time](../eazy-notes/assets/image/benchmark/bench-average-time-in-back.png)

**what's next?**

- COMPARE WiTH OTHERS LANGUAGES — *`js`, `python`, `c++`, `c#`.*

## contributing.

FEEL FREE TO OPEN AN iSSUE iF YOU WANT TO CONTRiBUTE. ALSO YOU CAN CONTACT US — THE [AT] COMPiLORDS [DOT] HOUSE.   

## license.

[APACHE](https://github.com/invisageable/zov/blob/main/.github/LICENSE-APACHE) — [MIT](https://github.com/invisageable/zov/blob/main/.github/LICENSE-MIT)   

COPYRiGHT© **10** JULY **2024** — *PRESENT, [@invisageable](https://github.com/invisageable).*     
