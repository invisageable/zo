//! Pipeline stages for the fret build system.
//!
//! Each stage implements a specific transformation of the BuildContext.
//! Stages are designed to be:
//! - Stateless (all state in BuildContext)
//! - Fast (zero-allocation where possible)
//! - Independent (can be parallelized in future)

mod collect_sources;
mod compile;
mod load_config;

// Re-export all stages
pub use collect_sources::CollectSources;
pub use compile::CompileStage;
pub use load_config::LoadConfig;

// Additional stages for the pipeline
mod execute_plan;
mod generate_plan;
mod resolve_dependencies;

pub use execute_plan::ExecutePlan;
pub use generate_plan::GeneratePlan;
pub use resolve_dependencies::ResolveDependencies;
