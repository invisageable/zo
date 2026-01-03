//! Compilation stage for fret.
//!
//! This stage integrates directly with zo-compiler as a library,
//! achieving maximum performance through direct integration.

use crate::types::{BuildContext, Stage, StageError};

use zo_codegen_backend::Target as ZoTarget;
use zo_compiler::orchestrator::Orchestrator;

use hashbrown::HashMap;

use std::fs;

/// Stage that compiles all collected source files using zo-compiler.
pub struct CompileStage;

impl Stage for CompileStage {
  fn execute(&self, ctx: &mut BuildContext) -> Result<(), StageError> {
    if ctx.source_files.is_empty() {
      return Err(StageError::Compilation(
        "No source files to compile".to_string(),
      ));
    }

    // Prepare source files map - single allocation.
    let mut source_map = HashMap::with_capacity(ctx.source_files.len());

    // Read all source files in parallel using rayon.
    use rayon::prelude::*;
    let file_contents: Result<Vec<_>, _> = ctx
      .source_files
      .par_iter()
      .map(|path| {
        fs::read_to_string(path)
          .map(|content| (path.clone(), content))
          .map_err(|e| {
            StageError::Compilation(format!(
              "Failed to read {}: {}",
              path.display(),
              e
            ))
          })
      })
      .collect();

    // Populate the source map.
    for (path, content) in file_contents? {
      source_map.insert(path, content);
    }

    // Map fret Target to zo-compiler Target.
    let zo_target = convert_target(ctx.target);

    // Execute compilation using zo-compiler orchestrator.
    let mut orchestrator = Orchestrator::new();
    let result =
      orchestrator.compile_batch(source_map, zo_target, Some(&ctx.output_dir));

    // Check for compilation errors.
    if !result.is_success() {
      let error_msg = result
        .errors()
        .iter()
        .map(|e| format!("{:?}", e))
        .collect::<Vec<_>>()
        .join("\n");
      return Err(StageError::Compilation(error_msg));
    }

    // Rename the entry point binary to match the configured binary_name.
    // The orchestrator names files based on source filenames, but we want
    // the main binary to use the project's binary_name from fret.oz.
    let entry_point_stem = ctx
      .config
      .entry_point
      .file_stem()
      .and_then(|s| s.to_str())
      .unwrap_or("main");

    let generated_binary = ctx.output_dir.join(entry_point_stem);
    let desired_binary_name = format!(
      "{}{}",
      ctx.config.binary_name,
      ctx.target.output_extension()
    );
    let target_binary = ctx.output_dir.join(&desired_binary_name);

    // Rename if needed and the generated binary exists
    if generated_binary.exists() && generated_binary != target_binary {
      fs::rename(&generated_binary, &target_binary).map_err(|e| {
        StageError::Compilation(format!(
          "Failed to rename {} to {}: {}",
          generated_binary.display(),
          target_binary.display(),
          e
        ))
      })?;
    }

    // Print compilation statistics.
    #[cfg(debug_assertions)]
    {
      eprintln!("Compilation stats:");
      eprintln!("  Files compiled: {}", result.files_compiled());
      eprintln!("  Duration: {:?}", result.duration());
    }

    Ok(())
  }

  fn name(&self) -> &'static str {
    "Compile"
  }
}

/// Converts fret Target to zo_codegen_backend Target.
fn convert_target(target: crate::types::Target) -> ZoTarget {
  match target {
    crate::types::Target::X86_64Linux => ZoTarget::X8664UnknownLinuxGnu,
    crate::types::Target::X86_64MacOS => ZoTarget::X8664AppleDarwin,
    crate::types::Target::X86_64Windows => ZoTarget::X8664PcWindowsMsvc,
    crate::types::Target::Aarch64Linux => ZoTarget::Arm64UnknownLinuxGnu,
    crate::types::Target::Aarch64MacOS => ZoTarget::Arm64AppleDarwin,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_target_conversion() {
    // Test all native targets
    assert_eq!(
      convert_target(crate::types::Target::X86_64Linux),
      ZoTarget::X8664UnknownLinuxGnu
    );
    assert_eq!(
      convert_target(crate::types::Target::X86_64MacOS),
      ZoTarget::X8664AppleDarwin
    );
    assert_eq!(
      convert_target(crate::types::Target::X86_64Windows),
      ZoTarget::X8664PcWindowsMsvc
    );
    assert_eq!(
      convert_target(crate::types::Target::Aarch64Linux),
      ZoTarget::Arm64UnknownLinuxGnu
    );
    assert_eq!(
      convert_target(crate::types::Target::Aarch64MacOS),
      ZoTarget::Arm64AppleDarwin
    );
  }
}
