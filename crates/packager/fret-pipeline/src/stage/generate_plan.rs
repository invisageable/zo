//! Validates configuration and prepares the compilation plan.

use fret_types::{BuildContext, Stage, StageError};

/// Stage that generates a compilation plan.
pub struct GeneratePlan;

impl Stage for GeneratePlan {
  fn name(&self) -> &'static str {
    "GeneratePlan"
  }

  fn execute(&self, ctx: &mut BuildContext) -> Result<(), StageError> {
    if ctx.source_files.is_empty() {
      return Err(StageError::Compilation(
        "No source files found in compilation plan".to_string(),
      ));
    }

    std::fs::create_dir_all(&ctx.output_dir)?;

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
}
