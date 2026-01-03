//! zo-template-optimizer - Compile-time optimization for UI templates
//!
//! This crate implements the template optimization strategy inspired by Malina.js:
//! - Analyze static vs dynamic parts
//! - Merge adjacent commands
//! - Flatten unnecessary nesting
//! - Mark static regions for caching
//!
//! Phase 1: Analysis (current implementation)
//! Phase 2: Optimization passes (TODO)
//! Phase 3: Code generation hints (TODO)

use zo_ui_protocol::UiCommand;

mod dependency;
mod optimizer;

pub use dependency::{DependencyGraph, DependencyNode};
pub use optimizer::{CommandClassification, TemplateOptimizer};

/// Analyze template commands and classify them
pub fn analyze_commands(commands: &[UiCommand]) -> Vec<CommandClassification> {
  let optimizer = TemplateOptimizer::new();
  optimizer.analyze(commands)
}

/// Optimize template commands using compile-time analysis
pub fn optimize_commands(commands: Vec<UiCommand>) -> Vec<UiCommand> {
  let optimizer = TemplateOptimizer::new();
  optimizer.optimize(commands)
}
