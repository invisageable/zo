mod compiler;
mod constants;
pub mod orchestrator;
mod stage;

#[cfg(test)]
mod tests;

pub use compiler::{Compiler, DiagnosticsConfig, default_core_search_paths};
pub use stage::Stage;
