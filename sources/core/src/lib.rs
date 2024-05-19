pub mod color;
pub mod interner;
pub mod reporter;
pub mod source;
pub mod span;

pub const EXIT_SUCCESS: i32 = 0i32;
pub const EXIT_FAILURE: i32 = 1i32;

pub type Result<R> = anyhow::Result<R, reporter::report::ReportError>;
