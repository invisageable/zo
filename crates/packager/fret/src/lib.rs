//! fret - The package manager for the zo programming language
//!
//! This crate provides the core functionality for building zo projects,
//! managing dependencies, and integrating with zo-compiler-orchestrator.

pub mod lexer;
pub mod parser;
pub mod pipeline;
pub mod stage;
pub mod token;
pub mod types;

pub use parser::parse_config;
pub use pipeline::{Pipeline, PipelineError};
pub use types::{
  BuildContext, BuildMode, ProjectConfig, Stage, StageError, Target, Version,
};
