//! Executes the compilation plan. zo-compiler produces
//! complete executables directly — no separate link step.

use crate::stage::CompileStage;
use fret_types::{BuildContext, Stage, StageError};

/// Stage that executes the compilation plan.
pub struct ExecutePlan;

impl Stage for ExecutePlan {
  fn execute(&self, ctx: &mut BuildContext) -> Result<(), StageError> {
    let compile_stage = CompileStage;
    compile_stage.execute(ctx)?;

    Ok(())
  }

  fn name(&self) -> &'static str {
    "ExecutePlan"
  }
}
