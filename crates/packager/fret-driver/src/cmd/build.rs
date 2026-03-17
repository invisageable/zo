use crate::cmd::Handle;
use crate::constants::{EXIT_CODE_ERROR, EXIT_CODE_SUCCESS};

use fret_pipeline::Pipeline;

use std::path::PathBuf;

#[derive(clap::Args, Debug)]
pub(crate) struct Build {
  /// Path to the project directory (default: current dir).
  #[arg(default_value = ".")]
  pub(crate) path: PathBuf,
}

impl Build {
  /// Builds a `zo` project based on `fret.oz`.
  fn build(&self) -> Result<PathBuf, String> {
    let project_path = if self.path.is_absolute() {
      self.path.clone()
    } else {
      std::env::current_dir()
        .map_err(|e| format!("{e}"))?
        .join(&self.path)
    };

    if !project_path.join("fret.oz").exists() {
      return Err(format!("No fret.oz found in {}", project_path.display()));
    }

    let pipeline = Pipeline::simple_mode();

    pipeline.execute(project_path).map_err(|e| format!("{e}"))
  }
}

impl Handle for Build {
  fn handle(&self) {
    match self.build() {
      Ok(binary_path) => {
        println!("Binary: {}", binary_path.display());
        std::process::exit(EXIT_CODE_SUCCESS);
      }
      Err(e) => {
        eprintln!("Error: {e}");
        std::process::exit(EXIT_CODE_ERROR);
      }
    }
  }
}
