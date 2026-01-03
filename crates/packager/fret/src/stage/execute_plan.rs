//! Plan execution stage for fret.
//!
//! This stage executes the compilation plan generated in the previous stage.
//! The zo-compiler generates complete executables directly, so no separate
//! linking stage is needed.

use crate::stage::CompileStage;
use crate::types::{BuildContext, Stage, StageError};

/// Stage that executes the compilation plan.
///
/// The zo-compiler generates complete executables in one pass, so this stage
/// only needs to invoke compilation. No separate linking step is required.
pub struct ExecutePlan;

impl Stage for ExecutePlan {
  fn execute(&self, ctx: &mut BuildContext) -> Result<(), StageError> {
    // Execute compilation - zo-compiler generates complete executables.
    let compile_stage = CompileStage;
    compile_stage.execute(ctx)?;

    Ok(())
  }

  fn name(&self) -> &'static str {
    "ExecutePlan"
  }
}
