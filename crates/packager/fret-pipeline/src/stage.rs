//! Pipeline stages. Each stage transforms [`BuildContext`] in
//! place. Stateless — all state lives in the context.

mod collect_sources;
mod compile;
mod load_config;

pub use collect_sources::CollectSources;
pub use compile::CompileStage;
pub use load_config::LoadConfig;

mod execute_plan;
mod generate_plan;
mod resolve_dependencies;

pub use execute_plan::ExecutePlan;
pub use generate_plan::GeneratePlan;
pub use resolve_dependencies::ResolveDependencies;
