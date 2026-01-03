//! Each functions is grouped by context:
//!
//! #### easing functions.
//!
//! |               |                                   |
//! |:--------------|:----------------------------------|
//! | polynomial    | linear, quadratic, cubic, etc.    |
//! | trigonometric | sine, circle.                     |
//! | exponential   | expo2, expoe.                     |
//! | logarithmic   | log10.                            |
//! | root          | sqrt.                             |
//! | oscillatory   | elastic, bounce.                  |
//! | backtracking  | back.                             |
//!
//! #### notes.
//!
//! *Time is a normalized percentage of the elapsed time [0,1] i.e a range
//! between 0.0 and 1.0 (both inclusively).*

pub mod backtracking;
pub mod bezier;
pub mod exponential;
pub mod logarithmic;
pub mod oscillatory;
pub mod polynomial;
pub mod root;
pub mod trigonometric;

use wide::f32x8;

/// ### The [`Curve`] Parabola Interface.
///
/// A curve that has an axis of symmetry parallel to the `y-axis`.
pub trait Curve {
  /// Computes the `y-axis` of the curve from a progress value.
  ///     
  /// #### params.
  ///   
  /// |     |               |
  /// |:----|:--------------|
  /// | `p` | The progress. |
  ///   
  /// #### returns.
  ///
  /// `f32` — The progress of the `y-axis` value.
  ///   
  /// #### examples.
  ///   
  /// ```rust
  /// use eazy::Curve;
  ///
  /// struct Linear;
  ///  
  /// impl Curve for Linear {
  ///   fn y(&self, p: f32) -> f32 {
  ///     -f32::INFINITY
  ///   }
  /// }
  ///  
  /// assert_eq!(Linear.y(0.0), -f32::INFINITY);
  /// ```
  fn y(&self, p: f32) -> f32;
}

/// Blanket impl for references, enabling `&dyn Curve` to work with `ease()`.
impl<T: Curve + ?Sized> Curve for &T {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    (**self).y(p)
  }
}

/// ### The [`Curve`] Parabola Interface using SIMD.
pub trait CurveSIMD {
  /// Computes the `y-axis` of the curve from a progress value.
  ///     
  /// #### params.
  ///   
  /// |     |               |
  /// |:----|:--------------|
  /// | `p` | The progress. |
  ///   
  /// #### returns.
  ///
  /// `f32` — The progress of the `y-axis` value.
  ///   
  /// #### examples.
  ///   
  /// ```rust
  /// use eazy::CurveSIMD;
  ///
  /// struct Constant;
  ///  
  /// impl CurveSIMD for Constant {
  ///   fn y(&self, p: f32x8) -> f32x8 {
  ///     -f32x8::ZERO
  ///   }
  /// }
  ///  
  /// assert_eq!(Constant.y(0.0), -f32::INFINITY);
  /// ```
  fn y(&self, p: f32x8) -> f32x8;
}

/// ### The [`Ease`] Function Interface.
pub trait Ease {
  /// Computes the `y-axis` of the curve from a progress value.
  ///     
  /// #### params.
  ///   
  /// |     |                   |
  /// |:----|:------------------|
  /// | `p` | The elapsed time. |
  /// | `a` | The start.        |
  /// | `b` | The end.          |
  ///
  /// #### returns.
  ///
  /// `f32` — The progress of the `y-axis` value.
  ///
  /// ```
  /// use eazy::ease;
  ///
  /// ease(0.5 / 4.0, 0.0, 1.0);
  /// ```
  fn ease(p: f32, a: f32, b: f32) -> f32;
}

/// ### The [`Easing`] Function User Access.
///
/// Wraps all easing functions in one place.
#[derive(Debug, Default, Clone)]
pub enum Easing {
  // constant.
  None,
  // polynomial:linear.
  #[default]
  Linear,
  // polynomial:in.
  InQuadratic,
  InCubic,
  InQuartic,
  InQuintic,
  InSextic,
  InSeptic,
  InOctic,
  InNonic,
  InDecic,
  InHectic,
  // polynomial:out.
  OutQuadratic,
  OutCubic,
  OutQuartic,
  OutQuintic,
  OutSextic,
  OutSeptic,
  OutOctic,
  OutNonic,
  OutDecic,
  OutHectic,
  // polynomial:in-out.
  InOutQuadratic,
  InOutCubic,
  InOutQuartic,
  InOutQuintic,
  InOutSextic,
  InOutSeptic,
  InOutOctic,
  InOutNonic,
  InOutDecic,
  InOutHectic,
  // trigonometric:in.
  InSine,
  InCircle,
  // trigonometric:out.
  OutSine,
  OutCircle,
  // trigonometric:in-out.
  InOutSine,
  InOutCircle,
  // exponential:in.
  InExpo2,
  InExpoE,
  // exponential:out.
  OutExpo2,
  OutExpoE,
  // exponential:in-out.
  InOutExpo2,
  InOutExpoE,
  // logarithmic:in.
  InLog10,
  // logarithmic:out.
  OutLog10,
  // logarithmic:in-out.
  InOutLog10,
  // root:in.
  InSqrt,
  // root:out.
  OutSqrt,
  // root:in-out.
  InOutSqrt,
  // oscillatory:in.
  InElastic,
  InBounce,
  // oscillatory:out.
  OutElastic,
  OutBounce,
  // oscillatory:in-out.
  InOutElastic,
  InOutBounce,
  // oscillatory:spring.
  Spring,
  // backtracking:in.
  InBack,
  // backtracking:out.
  OutBack,
  // backtracking:in-out.
  InOutBack,
  // cubic bezier.
  CubicBezier(bezier::Bezier),
  // interpolation.
  Interpolation(crate::interpolation::Interpolation),
  // custom easing function.
  // Custom(dyn Curve),
}

impl Curve for Easing {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    match self {
      Self::None => polynomial::none::None.y(p),
      Self::Linear => polynomial::linear::Linear.y(p),
      Self::InQuadratic => polynomial::quadratic::InQuadratic.y(p),
      Self::InCubic => polynomial::cubic::InCubic.y(p),
      Self::InQuartic => polynomial::quartic::InQuartic.y(p),
      Self::InQuintic => polynomial::quintic::InQuintic.y(p),
      Self::InSextic => polynomial::sextic::InSextic.y(p),
      Self::InSeptic => polynomial::septic::InSeptic.y(p),
      Self::InOctic => polynomial::octic::InOctic.y(p),
      Self::InNonic => polynomial::nonic::InNonic.y(p),
      Self::InDecic => polynomial::decic::InDecic.y(p),
      Self::InHectic => polynomial::hectic::InHectic.y(p),
      Self::OutQuadratic => polynomial::quadratic::OutQuadratic.y(p),
      Self::OutCubic => polynomial::cubic::OutCubic.y(p),
      Self::OutQuartic => polynomial::quartic::OutQuartic.y(p),
      Self::OutQuintic => polynomial::quintic::OutQuintic.y(p),
      Self::OutSextic => polynomial::sextic::OutSextic.y(p),
      Self::OutSeptic => polynomial::septic::OutSeptic.y(p),
      Self::OutOctic => polynomial::octic::OutOctic.y(p),
      Self::OutNonic => polynomial::nonic::OutNonic.y(p),
      Self::OutDecic => polynomial::decic::OutDecic.y(p),
      Self::OutHectic => polynomial::hectic::OutHectic.y(p),
      Self::InOutQuadratic => polynomial::quadratic::InOutQuadratic.y(p),
      Self::InOutCubic => polynomial::cubic::InOutCubic.y(p),
      Self::InOutQuartic => polynomial::quartic::InOutQuartic.y(p),
      Self::InOutQuintic => polynomial::quintic::InOutQuintic.y(p),
      Self::InOutSextic => polynomial::sextic::InOutSextic.y(p),
      Self::InOutSeptic => polynomial::septic::InOutSeptic.y(p),
      Self::InOutOctic => polynomial::octic::InOutOctic.y(p),
      Self::InOutNonic => polynomial::nonic::InOutNonic.y(p),
      Self::InOutDecic => polynomial::decic::InOutDecic.y(p),
      Self::InOutHectic => polynomial::hectic::InOutHectic.y(p),
      Self::InSine => trigonometric::sine::InSine.y(p),
      Self::InCircle => trigonometric::circle::InCircle.y(p),
      Self::OutSine => trigonometric::sine::OutSine.y(p),
      Self::OutCircle => trigonometric::circle::OutCircle.y(p),
      Self::InOutSine => trigonometric::sine::InOutSine.y(p),
      Self::InOutCircle => trigonometric::circle::InOutCircle.y(p),
      Self::InExpo2 => exponential::expo2::InExpo2.y(p),
      Self::InExpoE => exponential::expoe::InExpoE.y(p),
      Self::OutExpo2 => exponential::expo2::OutExpo2.y(p),
      Self::OutExpoE => exponential::expoe::InExpoE.y(p),
      Self::InOutExpo2 => exponential::expo2::InOutExpo2.y(p),
      Self::InOutExpoE => exponential::expoe::InOutExpoE.y(p),
      Self::InLog10 => logarithmic::log10::InLog10.y(p),
      Self::OutLog10 => logarithmic::log10::OutLog10.y(p),
      Self::InOutLog10 => logarithmic::log10::InOutLog10.y(p),
      Self::InSqrt => root::sqrt::InSqrt.y(p),
      Self::OutSqrt => root::sqrt::OutSqrt.y(p),
      Self::InOutSqrt => root::sqrt::InOutSqrt.y(p),
      Self::InElastic => oscillatory::elastic::InElastic.y(p),
      Self::InBounce => oscillatory::bounce::InBounce.y(p),
      Self::OutElastic => oscillatory::elastic::OutElastic.y(p),
      Self::OutBounce => oscillatory::bounce::OutBounce.y(p),
      Self::InOutElastic => oscillatory::elastic::InOutElastic.y(p),
      Self::InOutBounce => oscillatory::bounce::InOutBounce.y(p),
      Self::Spring => oscillatory::spring::Spring.y(p),
      Self::InBack => backtracking::back::InBack.y(p),
      Self::OutBack => backtracking::back::OutBack.y(p),
      Self::InOutBack => backtracking::back::InOutBack.y(p),
      Self::CubicBezier(bezier) => bezier.y(p),
      Self::Interpolation(interpolation) => interpolation.y(p),
      // shoud stay at the last place. New ones must be place above it.
      // Self::Custom(curve) => curve.y(p),
    }
  }
}

/// ### The Ease Function.
///
/// Interpolates between `start` and `end` using the given easing curve.
///
/// Zero-cost for static easing types (monomorphized at compile time).
/// Also works with trait objects thanks to the blanket impl for `&T`.
///
/// #### examples.
///
/// ```rust
/// use eazy::{ease, Curve, Easing};
/// use eazy::polynomial::quadratic::InQuadratic;
///
/// // Static type (zero-cost, inlined).
/// let value = ease(InQuadratic, 0.5, 0.0, 100.0);
///
/// // Easing enum.
/// let value = ease(Easing::InQuadratic, 0.5, 0.0, 100.0);
///
/// // Reference.
/// let easing = Easing::OutBounce;
/// let value = ease(&easing, 0.5, 0.0, 100.0);
///
/// // Trait object.
/// let curve: &dyn Curve = &Easing::InElastic;
/// let value = ease(curve, 0.5, 0.0, 100.0);
/// ```
#[inline(always)]
pub fn ease(curve: impl Curve, time: f32, start: f32, end: f32) -> f32 {
  start + (end - start) * curve.y(time)
}
