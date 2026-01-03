//! Compilation plan generation stage for fret.
//!
//! This stage creates an optimal compilation plan based on the collected
//! sources. In Simple Mode, this is straightforward - compile everything then
//! link. Future versions will support incremental compilation and dependency
//! graphs.

use crate::types::{BuildContext, Stage, StageError};

/// Stage that generates a compilation plan.
/// The plan determines the order and parallelization of compilation tasks.
pub struct GeneratePlan;

impl Stage for GeneratePlan {
  fn execute(&self, ctx: &mut BuildContext) -> Result<(), StageError> {
    // In Simple Mode, the plan is trivial:
    // 1. Compile all source files in parallel
    // 2. Link all object files together

    // Validate we have sources to compile
    if ctx.source_files.is_empty() {
      return Err(StageError::Compilation(
        "No source files found in compilation plan".to_string(),
      ));
    }

    // Ensure output directory exists
    std::fs::create_dir_all(&ctx.output_dir)?;

    // In the future, this stage will:
    // - Build a dependency graph
    // - Identify compilation units
    // - Determine optimal compilation order
    // - Plan incremental recompilation

    // For now, we just validate the configuration
    if ctx.config.binary_name.is_empty() {
      return Err(StageError::ConfigParse(
        "Binary name cannot be empty".to_string(),
      ));
    }

    #[cfg(debug_assertions)]
    eprintln!(
      "Generated compilation plan for {} source files",
      ctx.source_files.len()
    );

    Ok(())
  }

  fn name(&self) -> &'static str {
    "GeneratePlan"
  }
}
