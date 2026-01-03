// mod bench;

// // use bench::easing::backtracking::back;
// // use bench::easing::bezier;
// // use bench::easing::exponential::{expo2, expoe};
// // use bench::easing::logarithmic::log10;
// // use bench::easing::oscillatory::{bounce, elastic};
// // use bench::easing::root::sqrt;

// // use bench::easing::polynomial::{cubic, hectic, quadratic, quintic};
// // use bench::easing::trigonometric::{circle, sine};

// use eazy::Curve;

// use criterion::{Criterion, criterion_group, criterion_main};

// use std::hint::black_box;

// macro_rules! benchmark_curves {
//   ($group:expr, [$(($name:expr, $curve:expr)),* $(,)?]) => {
//     $(
//       $group.bench_function($name, |b| {
//         use nanorand::{Rng, WyRand};

//         let nums = (0..10_000)
//           .map(|_| WyRand::new().generate::<f32>())
//           .collect::<Vec<_>>();

//         b.iter(|| {
//         for num in nums.iter() {
//           black_box($curve.y(*num));
//         }
//         });
//       });
//     )*
//   };
// }

// fn bench_all_easing_functions(c: &mut Criterion) {
//   let mut group = c.benchmark_group("all");

//   benchmark_curves!(
//     group,
//     [
//       ("InCubic", eazy::polynomial::cubic::InCubic),
//       ("OutBounce", eazy::oscillatory::bounce::OutBounce),
//       ("InOutBack", eazy::backtracking::back::InOutBack),
//       // note(ivs) — keep it for onboarding tasks.
//     ]
//   );

//   group.finish();
// }

// #[rustfmt::skip]
// criterion_group!(
//   benches,
//   // // easing:all-easings.
//   // bench_all_easing_functions,
//   // // easing:internal::behavior.
//   // bench::internal::bench_internal_behavior,
//   // // easing:polynomial:quadratic.
//   // quadratic::bench_in_quadratic::in_quadratic,
//   // quadratic::bench_in_out_quadratic::in_out_quadratic,
//   // quadratic::bench_out_quadratic::out_quadratic,
//   // // easing:polynomial:cubic.
//   // cubic::bench_in_cubic::in_cubic,
//   // cubic::bench_in_out_cubic::in_out_cubic,
//   // cubic::bench_out_cubic::out_cubic,
//   // // easing:polynomial:quintic.
//   // quintic::bench_in_quintic::in_quintic,
//   // quintic::bench_in_out_quintic::in_out_quintic,
//   // quintic::bench_out_quintic::out_quintic,
//   // // easing:polynomial:hectic.
//   // hectic::bench_in_hectic::in_hectic,
//   // hectic::bench_in_out_hectic::in_out_hectic,
//   // hectic::bench_out_hectic::out_hectic,

//   // // easing:trigonometric:sine.
//   // sine::bench_in_sine::in_sine,
//   // sine::bench_in_out_sine::in_out_sine,
//   // sine::bench_out_sine::out_sine,
//   // // easing:trigonometric:circle.
//   // circle::bench_in_circle::in_circle,
//   // circle::bench_in_out_circle::in_out_circle,
//   // circle::bench_out_circle::out_circle,

//   // // // easing:exponential:expo2.
//   // // expo2::bench_in_expo2::in_expo2,
//   // // expo2::bench_in_out_expo2::in_out_expo2,
//   // // expo2::bench_out_expo2::out_expo2,
//   // // // easing:exponential:expoe.
//   // // expoe::bench_in_expoe::in_expoe,
//   // // expoe::bench_in_out_expoe::in_out_expoe,
//   // // expoe::bench_out_expoe::out_expoe,

//   // // // easing:logarithmic:log10.
//   // // log10::bench_in_log10::in_log10,
//   // // log10::bench_in_out_log10::in_out_log10,
//   // // log10::bench_out_log10::out_log10,

//   // // // easing:root:sqrt.
//   // // sqrt::bench_in_sqrt::in_sqrt,
//   // // sqrt::bench_in_out_sqrt::in_out_sqrt,
//   // // sqrt::bench_out_sqrt::out_sqrt,

//   // // // easing:oscillatory:bounce.
//   // // bounce::bench_in_bounce::in_bounce,
//   // // bounce::bench_in_out_bounce::in_out_bounce,
//   // // bounce::bench_out_bounce::out_bounce,
//   // // // easing:oscillatory:elastic.
//   // // elastic::bench_in_elastic::in_elastic,
//   // // elastic::bench_in_out_elastic::in_out_elastic,
//   // // elastic::bench_out_elastic::out_elastic,

//   // // easing:backtracking:back.
//   // back::bench_in_back::in_back,
//   // back::bench_in_out_back::in_out_back,
//   // // back::bench_out_back::out_back,

//   // // // easing:betier:cubic_bezier.
//   // // bezier::cubic_bezier::cubic_bezier,
// );

// criterion_main!(benches);
