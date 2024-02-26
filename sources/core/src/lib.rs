//! ...

pub mod case;
pub mod color;
pub mod dsa;
pub mod fmt;
pub mod fs;
pub mod interner;
pub mod mpsc;
pub mod profiler;
pub mod reporter;
pub mod source;
pub mod span;
pub mod system;
pub mod timer;
pub mod writer;

pub trait Error: Sized {
  fn report(&self) -> reporter::report::Report;
}

pub type Result<R> = anyhow::Result<R, reporter::report::ReportError>;

pub const EXIT_SUCCESS: i32 = 0i32;
pub const EXIT_FAILURE: i32 = 1i32;
