# eazy.

[![invisageable/zo](https://img.shields.io/badge/github-invisageable/zo-black?logo=github)](https://github.com/invisageable/zo)
![license: MIT/APACHE](https://img.shields.io/badge/license-MIT-blue?style=flat-square)
[![Latest version](https://img.shields.io/crates/v/eazy.svg)](https://crates.io/crates/eazy)
[![Documentation](https://docs.rs/eazy/badge.svg)](https://docs.rs/eazy)
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

eazy iS AMONG THE FASTEST, ATOMiC AND ENHANCED `easing` FUNCTiONS KiT FOR CREATiVE, GAMERS, PROGRAMMERS, SCiENTiSTS, ETC WHO CARES ABOUT METRiCS — @see [benchmark](https://invisageable.github.io/zo/eazy/benches).    

eazy iS METiCULOUSLY OPTiMiZED iT'S A PERFECT MATCH FOR PRECiSiON OR SOPHiSTiCATED PROGRAMS — SUCH AS GUi, GAME ENGiNE, PLOTS, WEB AND NATiVE APPLiCATiON. VERY USEFUL FOR iMMEDiATE MODE GUi.

> OK-AY, OK-AY — BUT WHY eazy iS OUTPERFORMiNG?

- MiNiMAL OVERHEAD ABSTRACTiON — *inlining, zero vtable lookups.*
- MATHEMATICALLY EFFiCiENT — *no branches, no inttermediate allocations.*
- PROPER BENCHMARK DiSCiPLiNE — *performance matter.*

## quick start.

```rs
use eazy::{Curve, OutBounce};

fn main() {
  for time in (0..=100).map(|x| x as f32 / 100.0) {
    let p = OutBounce.y(time);
    println!("p (value): {p:.3}");
  }
}
```

## easing functions.

96 EASiNG CURVES ACROSS 7 CATEGORiES, EACH WiTH `In`, `Out`, AND `InOut` VARiANTS.

**polynomial** — linear, quadratic, cubic, quartic, quintic, sextic, septic, octic, nonic, decic, hectic.
**trigonometric** — sine, circle.
**exponential** — expo2, expoe.
**logarithmic** — log10.
**root** — sqrt.
**oscillatory** — elastic, bounce, spring.
**backtracking** — back.

USE THEM DiRECTLY AS ZERO-COST STRUCTS OR THROUGH THE `Easing` ENUM:

```rs
use eazy::{Curve, Easing};
use eazy::polynomial::quadratic::InQuadratic;

// Zero-cost struct (monomorphized, inlined).
let value = InQuadratic.y(0.5);

// Easing enum (dynamic dispatch).
let value = Easing::OutBounce.y(0.5);
```

## the `ease()` function.

iNTERPOLATES BETWEEN TWO VALUES USiNG ANY EASiNG CURVE:

```rs
use eazy::{ease, Curve, Easing};
use eazy::polynomial::quadratic::InQuadratic;

// Static type (zero-cost, inlined).
let value = ease(InQuadratic, 0.5, 0.0, 100.0);

// Easing enum.
let value = ease(Easing::InQuadratic, 0.5, 0.0, 100.0);

// Trait object.
let curve: &dyn Curve = &Easing::InElastic;
let value = ease(curve, 0.5, 0.0, 100.0);
```

## interpolation functions.

BEYOND STANDARD EASiNG — SMOOTHSTEP, RATIONAL, PiECEWiZE, AND TRiGONOMETRiC iNTERPOLATiON:

```rs
use eazy::interpolation::Interpolation;
use eazy::Curve;

// Smoothstep (Hermite).
let p = Interpolation::InOutSmooth.y(0.5);

// Smootherstep (Ken Perlin).
let p = Interpolation::InOutSmoother.y(0.5);

// Quartic (Inigo Quilez).
let p = Interpolation::Quartic.y(0.5);

// Sinusoidal (Inigo Quilez).
let p = Interpolation::Sinusoidal.y(0.5);

// Rational cubic.
let p = Interpolation::InRationalCubic.y(0.5);
```

## tweens.

GSAP-LiKE ANiMATiON RUNTiME. TWEEN BETWEEN ANY TWO VALUES WiTH FiNE-GRAiNED CONTROL:

```rs
use eazy::{Tween, Controllable, Easing};

let mut tween = Tween::to(0.0_f32, 100.0)
  .duration(1.0)
  .easing(Easing::OutBounce)
  .delay(0.5)
  .on_complete(|| println!("done!"))
  .build();

tween.play();

// In your update loop (~60 FPS):
while tween.tick(0.016) {
  let value = tween.value();
  // Apply value to your target.
}
```

### repeat & yoyo.

```rs
use eazy::{Tween, Controllable, Easing, Repeat};

// Repeat 3 times with yoyo (ping-pong).
let mut tween = Tween::to(0.0_f32, 100.0)
  .duration(1.0)
  .easing(Easing::InOutQuadratic)
  .repeat(3u32)
  .yoyo(true)
  .build();

tween.play();

// Infinite repeat.
let mut tween = Tween::to(0.0_f32, 100.0)
  .duration(1.0)
  .repeat(-1i32)
  .build();
```

### time scale.

```rs
use eazy::{Tween, Controllable};

let mut tween = Tween::to(0.0_f32, 100.0)
  .duration(1.0)
  .time_scale(2.0) // 2x speed.
  .build();
```

### tween arrays & tuples.

```rs
use eazy::{Tween, Controllable};

// Tween a 3D position.
let mut tween = Tween::new(
  [0.0_f32, 0.0, 0.0],
  [100.0, 200.0, 300.0],
  1.0,
);

tween.play();
tween.tick(0.5);

let pos = tween.value(); // [50.0, 100.0, 150.0]
```

## timelines.

SEQUENCE MULTiPLE TWEENS WiTH PRECiSE TiMiNG — SEQUENTIAL, PARALLEL, OR OVERLAPPiNG:

```rs
use eazy::{Timeline, Tween, Position, Controllable, Easing};

let mut timeline = Timeline::builder()
  // First tween: 0.0s -> 1.0s.
  .push(Tween::to(0.0_f32, 100.0).duration(1.0).build())
  // Parallel with previous (starts at same time).
  .push_at(
    Tween::to(0.0_f32, 50.0).duration(0.5).build(),
    Position::WithPrevious,
  )
  // Overlap by 0.2s (starts before previous ends).
  .push_at(
    Tween::to(100.0_f32, 0.0).duration(1.0).build(),
    Position::Relative(-0.2),
  )
  .build();

timeline.play();

// Tick in your update loop.
while timeline.tick(0.016) {
  let progress = timeline.progress();
}
```

### labels.

```rs
use eazy::{Timeline, Tween, Position, Controllable};

let timeline = Timeline::builder()
  .push(Tween::to(0.0_f32, 100.0).duration(1.0).build())
  .label("middle")
  .push(Tween::to(0.0_f32, 50.0).duration(0.5).build())
  .build();

// Jump to label.
let time = timeline.get_label("middle"); // Some(1.0)
```

### GSAP-style position strings.

```rs
use eazy::Position;

let pos = Position::from("<");     // WithPrevious
let pos = Position::from(">");     // AfterPrevious
let pos = Position::from("+=0.5"); // Relative(0.5)
let pos = Position::from("-=0.3"); // Relative(-0.3) — overlap
```

## staggers.

CASCADiNG ANiMATiONS — DOMiNO FALLS, WAVE EFFECTS, RiPPLES:

```rs
use eazy::{Timeline, Tween, Stagger, StaggerFrom, Controllable, Easing};

let tweens: Vec<_> = (0..10)
  .map(|_| {
    Tween::to(0.0_f32, 1.0)
      .duration(0.5)
      .easing(Easing::OutBounce)
      .build()
  })
  .collect();

// 0.1s between each, starting from center outward.
let mut timeline = Timeline::builder()
  .push_staggered(tweens, Stagger::each(0.1).from(StaggerFrom::Center))
  .build();

timeline.play();
```

### stagger directions.

```rs
use eazy::{Stagger, StaggerFrom};

// First to last (default).
Stagger::each(0.1).from(StaggerFrom::Start);

// Last to first.
Stagger::each(0.1).from(StaggerFrom::End);

// Center outward.
Stagger::each(0.1).from(StaggerFrom::Center);

// Edges inward.
Stagger::each(0.1).from(StaggerFrom::Edges);

// Distribute across a total duration.
Stagger::total(1.0); // 10 items = 0.1s between each.

// Eased distribution.
Stagger::each(0.1).ease(eazy::Easing::OutQuadratic);
```

## keyframes.

DEFiNE COMPLEX ANiMATiONS WiTH VALUES AT SPECiFiC TiME POiNTS:

```rs
use eazy::{KeyframeTrack, Keyframe, Easing, Curve};

let track = KeyframeTrack::new()
  .keyframe(0.0, 0.0_f32)
  .keyframe_eased(0.5, 100.0, Easing::OutBounce)
  .keyframe(1.0, 50.0);

let value = track.sample(0.25); // Interpolated between keyframes.
let value = track.sample(0.75); // OutBounce easing applied.
```

### keyframes! macro.

```rs
use eazy::{keyframes, Easing};

// Concise syntax.
let track = keyframes![
  (0.0, 0.0_f32),
  (0.5, 100.0, Easing::OutBounce),
  (1.0, 50.0),
];

// Works with arrays too (positions, colors).
let track = keyframes![
  (0.0, [0.0_f32, 0.0]),
  (0.5, [100.0, 50.0], Easing::OutElastic),
  (1.0, [0.0, 100.0]),
];

let pos = track.sample(0.75);
```

## derive tweenable.

MAKE ANY STRUCT ANiMATABLE WiTH `#[derive(Tweenable)]`:

```rs
use eazy::Tweenable;

#[derive(Clone, Copy, Tweenable)]
struct Position {
  x: f32,
  y: f32,
}

#[derive(Clone, Copy, Tweenable)]
struct Color {
  r: f32,
  g: f32,
  b: f32,
  a: f32,
}

let red = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
let blue = Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };
let purple = red.lerp(blue, 0.5);
// Color { r: 0.5, g: 0.0, b: 0.5, a: 1.0 }

// Works with tuple structs too.
#[derive(Clone, Copy, Tweenable)]
struct Vec2(f32, f32);

let mid = Vec2(0.0, 0.0).lerp(Vec2(100.0, 200.0), 0.5);
// Vec2(50.0, 100.0)
```

## egui integration.

BOUNCiNG BALL WiTH eazy + egui:

```rs
use eazy::{Curve, Easing};
use eframe::egui;
use std::time::Instant;

pub struct BounceApp {
  start_time: Instant,
}

impl eframe::App for BounceApp {
  fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    let elapsed = self.start_time.elapsed().as_secs_f32();
    let t = (elapsed % 2.0) / 2.0;
    let bounce = Easing::OutBounce.y(t);

    egui::CentralPanel::default().show(ctx, |ui| {
      let painter = ui.painter();
      let y = 300.0 - bounce * 260.0;

      painter.circle_filled(
        egui::pos2(150.0, y),
        20.0,
        egui::Color32::LIGHT_GREEN,
      );
    });

    ctx.request_repaint();
  }
}
```

STAGGERED TiMELiNE WiTH egui:

```rs
use eazy::{Controllable, Curve, Easing, Stagger, StaggerFrom, Timeline, Tween};
use std::time::Instant;

// Create 5 balls with staggered bounce from center outward.
let tweens = (0..5)
  .map(|_| {
    Tween::to(0.0_f32, 1.0)
      .duration(1.0)
      .easing(Easing::OutBounce)
      .build()
  })
  .collect::<Vec<_>>();

let mut timeline = Timeline::builder()
  .push_staggered(tweens, Stagger::each(0.15).from(StaggerFrom::Center))
  .build();

timeline.play();

// In your update loop:
let delta = last_frame.elapsed().as_secs_f32();
timeline.tick(delta);
let progress = timeline.progress();
```

## callbacks.

SYNC AND ASYNC LiFECYCLE CALLBACKS:

```rs
use eazy::{Tween, Controllable, Easing};

let mut tween = Tween::to(0.0_f32, 100.0)
  .duration(1.0)
  .easing(Easing::InOutCubic)
  .on_start(|| println!("started!"))
  .on_update(|| println!("tick"))
  .on_complete(|| println!("done!"))
  .on_repeat(|| println!("repeating..."))
  .build();
```

## SiMD.

BATCH PROCESS 8 VALUES AT ONCE WiTH `CurveSIMD` AND `wide::f32x8`:

```rs
use eazy::CurveSIMD;
use wide::f32x8;

// All easing functions have SIMD variants
// for processing 8 values simultaneously.
```

## features.

ENABLiNG DERiVE MACRO:

```toml
[dependencies]
eazy = { version = "0.0.1", features = ["derive"] }
```

## examples.

MORE EXAMPLES [`here`](https://github.com/invisageable/zo/tree/main/sources/tweener/eazy-examples).

- supports for egui, bevy.

## benches.

> *beat'em up!*

BENCHES ARE DONE iN COMPARiSON BETWEEN `easings`, `emath`<sup>egui</sup> , `glissade`, `interpolation`<sup>pisthon</sup>, `keyframe`, `nova-easing`, `simple-easing2` CRATES. MOST OF THEM ARE FOLLOW THE ROBERT PENNER'S EASiNG FUNCTiONS, THEY ONLY iMPLEMENTED THE BASiCS ONE. REGARDiNG PERFORMANCE SOME OF OUR iMPLEMENTATiONS ARE SLiGHTLY FASTER AND STABLE, SO DEPENDiNG YOUR NEEDED, YOU SHOULD TRY eazy. THE SAMPLE BELOW CONFiRM THAT OUR EASiNG FUNCTiONS ARE PRETTY WELL OPTiMiZED.

@see [@benchmark-reports](https://invisageable.github.io/zo/eazy/benches).

## contributing.

WE LOVE CONTRiBUTORS.   

FEEL FREE TO OPEN AN iSSUE iF YOU WANT TO CONTRiBUTE OR COME TO SAY HELLO ON [discord](https://discord.gg/JaNc4Nk5xw). ALSO YOU CAN CONTACT US AT THE [at] COMPiLORDS [dot] HOUSE. THiS iS A PLAYGROUND FOR COMPiLER __NERDS__, FRONTEND __HACKERS__, AND __CREATIVE__.    

## license.

[APACHE](https://github.com/invisageable/zo/blob/main/LICENSE-APACHE) — [MIT](https://github.com/invisageable/zo/blob/main/LICENSE-MIT)   

COPYRiGHT© **10** JULY **2024** — *PRESENT, [@invisageable](https://github.com/invisageable).*     