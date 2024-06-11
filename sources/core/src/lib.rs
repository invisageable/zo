pub mod case;
pub mod color;
pub mod fmt;
pub mod interner;
pub mod mpsc;
pub mod profiler;
pub mod reporter;
pub mod source;
pub mod span;
pub mod system;
pub mod timer;
pub mod writer;

pub const EXIT_SUCCESS: i32 = 0i32;
pub const EXIT_FAILURE: i32 = 1i32;

pub type Result<R> = anyhow::Result<R, reporter::report::ReportError>;
