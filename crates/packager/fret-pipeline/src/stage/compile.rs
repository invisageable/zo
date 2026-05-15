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
    let file_contents = ctx
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
      .collect::<Result<Vec<_>, _>>();

    for (path, content) in file_contents? {
      source_map.insert(path, content);
    }

    let zo_target = convert_target(ctx.target);

    // The entry point produces the project binary at
    // `<output_dir>/<binary_name>`; other source files in the
    // batch fall back to `<output_dir>/<stem>`. Resolving
    // `entry_point` against `project_root` mirrors the same
    // join the source-collection stage performs, so the path
    // matches a key in `source_map`.
    let desired_binary_name = format!(
      "{}{}",
      ctx.config.binary_name,
      ctx.target.output_extension()
    );
    let binary_path = ctx.output_dir.join(&desired_binary_name);

    let entry_source = if ctx.config.entry_point.is_absolute() {
      ctx.config.entry_point.clone()
    } else {
      ctx.project_root.join(&ctx.config.entry_point)
    };

    let mut orchestrator = Orchestrator::new();
    let result = orchestrator.compile_batch(
      source_map,
      zo_target,
      Some(&ctx.output_dir),
      Some((&entry_source, &binary_path)),
    );

    if !result.is_success() {
      let error_msg = result
        .errors()
        .iter()
        .map(|e| format!("{e:?}"))
        .collect::<Vec<_>>()
        .join("\n");

      return Err(StageError::Compilation(error_msg));
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
