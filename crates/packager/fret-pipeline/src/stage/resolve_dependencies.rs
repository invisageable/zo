//! Dependency resolution stage.
// TODO: implement dependency resolution when fret supports
// external packages. Currently a no-op in Simple Mode.

use fret_types::{BuildContext, Stage, StageError};

/// Stage that resolves project dependencies.
/// Currently a no-op for Simple Mode.
pub struct ResolveDependencies;

impl Stage for ResolveDependencies {
  fn name(&self) -> &'static str {
    "ResolveDependencies"
  }

  fn execute(&self, _ctx: &mut BuildContext) -> Result<(), StageError> {
    Ok(())
  }
}
