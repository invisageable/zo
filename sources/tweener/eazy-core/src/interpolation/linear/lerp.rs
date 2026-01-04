//! ## The Lerp Interpolating Linear Curve.

// --- START of derived from `interpolation`.
// origin: MIT https://github.com/PistonDevelopers/interpolation/blob/master/src/lerp.rs
//
// Copyright (c) 2015 PistonDevelopers, invisageable
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:

/// ### The Linear Interpolation.
///
/// Interpolates linearly a value between two floats with a linear easing.
/// Useful to tweak a value from point A to point B. The progress value can be
/// whatever you want — time, percentage, etc. in a range of `[0,1]`.
///
/// #### params.
///
/// |     |                            |
/// |:----|:---------------------------|
/// | `p` | The progress.              |
/// | `a` | The `start` initial value. |
/// | `b` | The `end` final value.     |
///
/// #### returns.
///
/// `f32` — The interpolated result between the two float values.
///
/// #### examples.
///
/// ```
/// use eazy::interpolation::lerp::lerp;
///
/// let start   = 2.0;
/// let end     = 8.0;
/// let p       = lerp(0.1, start, end);
///
/// assert_eq!(p, 2.6);
/// ```
#[inline(always)]
pub fn lerp<T: Lerp>(p: T::Scalar, a: T, b: T) -> T {
  a.lerp(&b, &p)
}

/// Describes a type that can linearly interpolate between two points.
pub trait Lerp {
  /// The scaling type for linear interpolation.
  type Scalar;

  /// Given `self` and another point `other`, return a point on a line running
  /// between the two that is `scalar` fraction of the distance between the
  /// two points.
  fn lerp(&self, other: &Self, scalar: &Self::Scalar) -> Self;
}

/// Implementation of `Lerp` for floats.
macro_rules! impl_lerp_for_float {
  ($($float: ident),*) => {
    $(
      impl Lerp for $float {
        type Scalar = $float;

        #[inline(always)]
        fn lerp(&self, other: &$float, scalar: &$float) -> $float {
          self + (other - self) * scalar
        }
      }
    )*
  };
}

impl_lerp_for_float!(f32, f64);

/// Transitive impl of `Lerp` for arrays, given a length and index list
macro_rules! impl_lerp_for_array {
  ($len:expr; $($i:expr),*) => {
    impl<T> Lerp for [T; $len] where T: Lerp {
      type Scalar = T::Scalar;

      #[inline(always)]
      fn lerp(&self, other: &Self, scalar: &Self::Scalar) -> Self {
        [
          $(self[$i].lerp(&other[$i], scalar)),*
        ]
      }
    }
  }
}

impl_lerp_for_array!(1; 0);
impl_lerp_for_array!(2; 0, 1);
impl_lerp_for_array!(3; 0, 1, 2);
impl_lerp_for_array!(4; 0, 1, 2, 3);
impl_lerp_for_array!(5; 0, 1, 2, 3, 4);

// --- END of derived from `interpolation`.
