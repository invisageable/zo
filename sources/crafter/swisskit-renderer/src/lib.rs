//! Swisskit renderer - Generic rendering infrastructure for compilers and
//! tools.

#[cfg(not(target_arch = "wasm32"))]
pub mod html;
pub mod markdown;
#[cfg(not(target_arch = "wasm32"))]
pub mod pdf;
pub mod xls;
