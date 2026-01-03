pub mod cubic_bezier;

use crate::easing::Curve;

/// The Unit Bezier.
#[derive(Copy, Clone, Debug, Default)]
pub enum Bezier {
  /// CSS `ease` timing function.
  /// Equivalent to `cubic-bezier(0.25, 0.1, 0.25, 1)`.
  #[default]
  Ease,
  /// CSS `ease-in` timing function.
  /// Equivalent to `cubic-bezier(0.42, 0, 1, 1)`.
  InEase,
  /// CSS `ease-out` timing function.
  /// Equivalent to `cubic-bezier(0, 0, 0.58, 1)`.
  OutEase,
  /// CSS `ease-in-out` timing function.
  /// Equivalent to `cubic-bezier(0.42, 0, 0.58, 1)`.
  InOutEase,
  /// Specifies the unit Bézier control points — `p1` and `p2`.
  ///
  /// i.e Curve(p1x, p1y, p2x, p2y).
  Curve(f32, f32, f32, f32),
}

impl Curve for Bezier {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    match self {
      Self::Ease => cubic_bezier::CubicBezier::ease().y(p),
      Self::InEase => cubic_bezier::CubicBezier::in_ease().y(p),
      Self::OutEase => cubic_bezier::CubicBezier::out_ease().y(p),
      Self::InOutEase => cubic_bezier::CubicBezier::in_out_ease().y(p),
      Self::Curve(p1x, p1y, p2x, p2y) => {
        cubic_bezier::CubicBezier::curve(*p1x, *p1y, *p2x, *p2y).y(p)
      }
    }
  }
}
