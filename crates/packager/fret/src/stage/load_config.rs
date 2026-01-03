//! Configuration loading stage for fret.
//!
//! This stage finds and parses the fret.oz configuration file,
//! populating the BuildContext with project metadata.

use crate::parser::parse_config;
use crate::types::{BuildContext, Stage, StageError};
use std::fs;

/// Stage that loads and parses the fret.oz configuration file.
pub struct LoadConfig;

impl Stage for LoadConfig {
  fn execute(&self, ctx: &mut BuildContext) -> Result<(), StageError> {
    // Fast path: check for fret.oz in project root
    let config_path = ctx.project_root.join("fret.oz");

    if !config_path.exists() {
      return Err(StageError::ConfigParse(format!(
        "fret.oz not found in {}",
        ctx.project_root.display()
      )));
    }

    // Read configuration file - single allocation
    let config_content = fs::read_to_string(&config_path)?;

    // Parse configuration - parse_config takes the content string
    let config = parse_config(&config_content)?;

    // Update build context with parsed configuration
    ctx.config = config;

    // Set derived values
    ctx.output_dir =
      ctx
        .project_root
        .join("build")
        .join(if ctx.config.debug_symbols {
          "debug"
        } else {
          "release"
        });

    ctx.compiler_flags.opt_level = ctx.config.optimization_level;
    ctx.compiler_flags.debug_info = ctx.config.debug_symbols;

    ctx.build_mode = if ctx.config.debug_symbols {
      crate::types::BuildMode::Debug
    } else {
      crate::types::BuildMode::Release
    };

    // Ensure output directory exists
    fs::create_dir_all(&ctx.output_dir)?;

    Ok(())
  }

  fn name(&self) -> &'static str {
    "LoadConfig"
  }
}
