//! # Math Abstraction Layer.
//!
//! Uses hardware FPU instructions via `std` by default.
//! Falls back to software `libm` for `no_std` environments.

#[inline(always)]
pub fn sinf(x: f32) -> f32 {
  #[cfg(feature = "std")]
  {
    x.sin()
  }
  #[cfg(not(feature = "std"))]
  {
    libm::sinf(x)
  }
}

#[inline(always)]
pub fn cosf(x: f32) -> f32 {
  #[cfg(feature = "std")]
  {
    x.cos()
  }
  #[cfg(not(feature = "std"))]
  {
    libm::cosf(x)
  }
}

#[inline(always)]
pub fn sqrtf(x: f32) -> f32 {
  #[cfg(feature = "std")]
  {
    x.sqrt()
  }
  #[cfg(not(feature = "std"))]
  {
    libm::sqrtf(x)
  }
}

#[inline(always)]
pub fn exp2f(x: f32) -> f32 {
  #[cfg(feature = "std")]
  {
    x.exp2()
  }
  #[cfg(not(feature = "std"))]
  {
    libm::exp2f(x)
  }
}

#[inline(always)]
pub fn expf(x: f32) -> f32 {
  #[cfg(feature = "std")]
  {
    x.exp()
  }
  #[cfg(not(feature = "std"))]
  {
    libm::expf(x)
  }
}

#[inline(always)]
pub fn log10f(x: f32) -> f32 {
  #[cfg(feature = "std")]
  {
    x.log10()
  }
  #[cfg(not(feature = "std"))]
  {
    libm::log10f(x)
  }
}
