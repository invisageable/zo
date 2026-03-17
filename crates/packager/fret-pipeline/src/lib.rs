//! Build pipeline for fret projects. Orchestrates stages from
//! config parsing through compilation via zo-compiler.
//! Direct library integration — no subprocesses.

pub mod pipeline;
pub mod stage;

pub use pipeline::{Pipeline, PipelineError};
