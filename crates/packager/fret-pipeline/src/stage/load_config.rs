//! Reads and parses fret.oz, populating [`BuildContext`].

use fret_parser::parse_config;
use fret_types::{BuildContext, Stage, StageError};

use std::fs;

/// Stage that loads and parses the fret.oz configuration file.
pub struct LoadConfig;

impl Stage for LoadConfig {
  fn name(&self) -> &'static str {
    "LoadConfig"
  }

  fn execute(&self, ctx: &mut BuildContext) -> Result<(), StageError> {
    let config_path = ctx.project_root.join("fret.oz");

    if !config_path.exists() {
      return Err(StageError::ConfigParse(format!(
        "fret.oz not found in {}",
        ctx.project_root.display()
      )));
    }

    let config_content = fs::read_to_string(&config_path)?;
    let config = parse_config(&config_content)?;

    ctx.config = config;

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
      fret_types::BuildMode::Debug
    } else {
      fret_types::BuildMode::Release
    };

    fs::create_dir_all(&ctx.output_dir)?;

    Ok(())
  }
}
