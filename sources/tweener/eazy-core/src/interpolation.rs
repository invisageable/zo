pub mod linear;
pub mod piecewize;
pub mod polynomial;
pub mod rational;
pub mod trigonometric;

use crate::Curve;

/// ### The [`Interpolation`] Function User Access.
///
/// Wraps all interpolation functions in one place.
///
/// #### examples.
///
/// ```rust
/// use eazy::interpolation::Interpolation;
/// use eazy::Curve;
///
/// let p = Interpolation::InOutSmooth.y(0.5);
///
/// assert_eq!(p, 0.5);
/// ```
#[derive(Debug, Clone)]
pub enum Interpolation {
  // constant.
  None,
  // linear.
  // #[default]
  Linear,
  // polynomial:smoothstep.
  InSmooth,
  OutSmooth,
  InOutSmooth,
  // polynomial:smootherstep.
  InSmoother,
  OutSmoother,
  InOutSmoother,
  // polynomial:quartic (Inigo Quilez).
  Quartic,
  InvQuartic,
  // rational:in.
  InRationalCubic,
  InRationalQuadratic,
  // rational:out.
  OutRationalCubic,
  OutRationalQuadratic,
  // piecewize:in.
  InPiecewizePolynomial,
  InPiecewizeQuadratic,
  // piecewize:out.
  OutPiecewizePolynomial,
  OutPiecewizeQuadratic,
  // trigonometric:sinusoidal (Inigo Quilez).
  Sinusoidal,
  InvSinusoidal,
}

impl Curve for Interpolation {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    match self {
      Self::None => polynomial::none::None.y(p),
      Self::Linear => p, // Linear interpolation is identity
      Self::InSmooth => polynomial::smoothstep::InSmooth.y(p),
      Self::OutSmooth => polynomial::smoothstep::OutSmooth.y(p),
      Self::InOutSmooth => polynomial::smoothstep::InOutSmooth.y(p),
      Self::InSmoother => polynomial::smootherstep::InSmoother.y(p),
      Self::OutSmoother => polynomial::smootherstep::OutSmoother.y(p),
      Self::InOutSmoother => polynomial::smootherstep::InOutSmoother.y(p),
      Self::Quartic => polynomial::quartic::Quartic.y(p),
      Self::InvQuartic => polynomial::quartic::InvQuartic.y(p),
      Self::InRationalCubic => rational::cubic::InRationalCubic.y(p),
      Self::InRationalQuadratic => {
        rational::quadratic::InRationalQuadratic.y(p)
      }
      Self::OutRationalCubic => rational::cubic::OutRationalCubic.y(p),
      Self::OutRationalQuadratic => {
        rational::quadratic::OutRationalQuadratic.y(p)
      }
      Self::InPiecewizePolynomial => {
        piecewize::polynomial::InPiecewizePolynomial.y(p)
      }
      Self::InPiecewizeQuadratic => {
        piecewize::quadratic::InPiecewizeQuadratic.y(p)
      }
      Self::OutPiecewizePolynomial => {
        piecewize::polynomial::OutPiecewizePolynomial.y(p)
      }
      Self::OutPiecewizeQuadratic => {
        piecewize::quadratic::OutPiecewizeQuadratic.y(p)
      }
      Self::Sinusoidal => trigonometric::sinusoidal::Sinusoidal.y(p),
      Self::InvSinusoidal => trigonometric::sinusoidal::InvSinusoidal.y(p),
    }
  }
}
