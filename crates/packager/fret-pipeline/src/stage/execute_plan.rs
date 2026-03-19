//! Executes the compilation plan. zo-compiler produces
//! complete executables directly — no separate link step.

use crate::stage::CompileStage;

use fret_types::{BuildContext, Stage, StageError};

/// Stage that executes the compilation plan.
pub struct ExecutePlan;

impl Stage for ExecutePlan {
  fn name(&self) -> &'static str {
    "ExecutePlan"
  }

  fn execute(&self, ctx: &mut BuildContext) -> Result<(), StageError> {
    CompileStage.execute(ctx)
  }
}
