//! Compilation stage — integrates with zo-compiler as a
//! library for direct, zero-subprocess compilation.

use fret_types::{BuildContext, Stage, StageError};

use zo_codegen_backend::Target as ZoTarget;
use zo_compiler::orchestrator::Orchestrator;

use hashbrown::HashMap;

use std::fs;

/// Stage that compiles all collected source files using
/// zo-compiler.
pub struct CompileStage;

impl Stage for CompileStage {
  fn execute(&self, ctx: &mut BuildContext) -> Result<(), StageError> {
    if ctx.source_files.is_empty() {
      return Err(StageError::Compilation(
        "No source files to compile".to_string(),
      ));
    }

    let mut source_map = HashMap::with_capacity(ctx.source_files.len());

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

    for (path, content) in file_contents? {
      source_map.insert(path, content);
    }

    let zo_target = convert_target(ctx.target);

    let mut orchestrator = Orchestrator::new();
    let result =
      orchestrator.compile_batch(source_map, zo_target, Some(&ctx.output_dir));

    if !result.is_success() {
      let error_msg = result
        .errors()
        .iter()
        .map(|e| format!("{:?}", e))
        .collect::<Vec<_>>()
        .join("\n");
      return Err(StageError::Compilation(error_msg));
    }

    // zo-compiler names output binaries after the source
    // filename (e.g. "main"), but we want the project's
    // configured binary_name from fret.oz instead.
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
fn convert_target(target: fret_types::Target) -> ZoTarget {
  match target {
    fret_types::Target::X86_64Linux => ZoTarget::X8664UnknownLinuxGnu,
    fret_types::Target::X86_64MacOS => ZoTarget::X8664AppleDarwin,
    fret_types::Target::X86_64Windows => ZoTarget::X8664PcWindowsMsvc,
    fret_types::Target::Aarch64Linux => ZoTarget::Arm64UnknownLinuxGnu,
    fret_types::Target::Aarch64MacOS => ZoTarget::Arm64AppleDarwin,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_target_conversion() {
    assert_eq!(
      convert_target(fret_types::Target::X86_64Linux),
      ZoTarget::X8664UnknownLinuxGnu
    );
    assert_eq!(
      convert_target(fret_types::Target::X86_64MacOS),
      ZoTarget::X8664AppleDarwin
    );
    assert_eq!(
      convert_target(fret_types::Target::X86_64Windows),
      ZoTarget::X8664PcWindowsMsvc
    );
    assert_eq!(
      convert_target(fret_types::Target::Aarch64Linux),
      ZoTarget::Arm64UnknownLinuxGnu
    );
    assert_eq!(
      convert_target(fret_types::Target::Aarch64MacOS),
      ZoTarget::Arm64AppleDarwin
    );
  }
}
