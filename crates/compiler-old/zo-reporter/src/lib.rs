mod color;
pub mod error;
pub mod report;
pub mod reporter;

/// The result type of the whole compiler.
pub type Result<T> = anyhow::Result<T, error::Error>;
