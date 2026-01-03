//! The fret build pipeline - Orchestrating the fastest builds in the west.
//!
//! This module implements the complete build pipeline for fret projects.
//! Following the "Compile Fast, Analyze Deep" philosophy, we optimize for:
//! - Zero-allocation where possible
//! - Data-oriented design
//! - Maximum parallelism
//! - Direct library integration (no subprocesses)

use crate::stage::{
  CollectSources, ExecutePlan, GeneratePlan, LoadConfig, ResolveDependencies,
};
use crate::types::{BuildContext, Stage, StageError, Target};

use std::path::PathBuf;
use std::time::Instant;

/// The main build pipeline that executes all stages in order.
/// This is the entry point for all fret builds.
pub struct Pipeline {
  /// Stages to execute in order
  stages: Vec<Box<dyn Stage>>,
}
impl Pipeline {
  /// Create a new pipeline for Simple Mode builds.
  /// This mode focuses on maximum compilation speed for single projects.
  pub fn simple_mode() -> Self {
    let stages: Vec<Box<dyn Stage>> = vec![
      Box::new(LoadConfig),
      Box::new(CollectSources),
      Box::new(ResolveDependencies), // No-op in simple mode
      Box::new(GeneratePlan),
      Box::new(ExecutePlan),
    ];

    Self { stages }
  }

  /// Execute the entire pipeline, transforming raw project path into compiled
  /// binary. Returns the output binary path on success.
  pub fn execute(
    &self,
    project_path: PathBuf,
  ) -> Result<PathBuf, PipelineError> {
    self.execute_with_target(project_path, None)
  }

  /// Execute the entire pipeline with an optional target override.
  /// Returns the output binary path on success.
  pub fn execute_with_target(
    &self,
    project_path: PathBuf,
    target: Option<Target>,
  ) -> Result<PathBuf, PipelineError> {
    let start_time = Instant::now();

    // Read and parse the configuration file
    let config_path = project_path.join("fret.oz");
    let config_content =
      std::fs::read_to_string(&config_path).map_err(|e| {
        PipelineError::ConfigParse(format!("Failed to read fret.oz: {}", e))
      })?;

    let config = crate::parser::parse_config(&config_content)
      .map_err(|e| PipelineError::ConfigParse(e.to_string()))?;

    let mut ctx = BuildContext::new(config, project_path);

    // Override target if specified
    if let Some(t) = target {
      ctx.target = t;
      ctx.compiler_flags.target = t;
    }

    // Execute each stage in sequence
    for stage in &self.stages {
      let stage_start = Instant::now();

      if let Err(e) = stage.execute(&mut ctx) {
        return Err(PipelineError::Stage {
          stage: stage.name().to_string(),
          error: e,
        });
      }

      let stage_time = stage_start.elapsed();

      // Log stage completion in debug builds
      #[cfg(debug_assertions)]
      eprintln!("[fret] {} completed in {:?}", stage.name(), stage_time);
    }

    // Calculate final binary path based on target
    let binary_name = format!(
      "{}{}",
      ctx.config.binary_name,
      ctx.target.output_extension()
    );

    let binary_path = ctx.output_dir.join(&binary_name);

    let total_time = start_time.elapsed();

    // Print build summary
    println!(
      "Build completed in {:.2}s - {} files compiled",
      total_time.as_secs_f64(),
      ctx.source_files.len()
    );

    Ok(binary_path)
  }
}

/// Errors that can occur during pipeline execution
#[derive(Debug)]
pub enum PipelineError {
  /// Configuration parsing failed
  ConfigParse(String),

  /// A pipeline stage failed
  Stage { stage: String, error: StageError },
}
impl std::fmt::Display for PipelineError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      PipelineError::ConfigParse(e) => {
        write!(f, "Failed to parse fret.oz: {}", e)
      }
      PipelineError::Stage { stage, error } => {
        write!(f, "Stage '{}' failed: {}", stage, error)
      }
    }
  }
}
impl std::error::Error for PipelineError {}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_simple_pipeline_creation() {
    let pipeline = Pipeline::simple_mode();
    assert_eq!(pipeline.stages.len(), 5);
  }

  #[test]
  fn test_pipeline_error_display() {
    let err = PipelineError::ConfigParse("test error".to_string());
    assert_eq!(err.to_string(), "Failed to parse fret.oz: test error");

    let err = PipelineError::Stage {
      stage: "TestStage".to_string(),
      error: StageError::Compilation("compilation failed".to_string()),
    };
    assert_eq!(
      err.to_string(),
      "Stage 'TestStage' failed: Compilation error: compilation failed"
    );
  }
}
