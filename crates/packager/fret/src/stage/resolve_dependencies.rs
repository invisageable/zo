//! Dependency resolution stage for fret.
//!
//! This stage is a no-op in Simple Mode but will handle
//! dependency resolution in future versions.

use crate::types::{BuildContext, Stage, StageError};

/// Stage that resolves project dependencies.
/// Currently a no-op for Simple Mode.
pub struct ResolveDependencies;

impl Stage for ResolveDependencies {
  fn execute(&self, _ctx: &mut BuildContext) -> Result<(), StageError> {
    // No-op in Simple Mode
    // Future: Will resolve dependencies from fret.oz
    Ok(())
  }

  fn name(&self) -> &'static str {
    "ResolveDependencies"
  }
}
