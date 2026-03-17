use crate::cmd::Handle;
use crate::constants::{EXIT_CODE_ERROR, EXIT_CODE_SUCCESS};

use std::fs;
use std::path::PathBuf;

#[derive(clap::Args, Debug)]
pub(crate) struct Init {
  /// Name of the project to create.
  pub(crate) name: String,
}

impl Init {
  /// Inits a `zo` project.
  fn init(&self) -> Result<PathBuf, String> {
    let project_dir = std::env::current_dir()
      .map_err(|e| format!("{e}"))?
      .join(&self.name);

    if project_dir.exists() {
      return Err(format!("Directory '{}' already exists", self.name));
    }

    // Create project structure
    let src_dir = project_dir.join("src");

    fs::create_dir_all(&src_dir).map_err(|e| format!("{e}"))?;

    // Write fret.oz
    let config = format!(
      "@pack = (\n\
       \x20 name: \"{name}\",\n\
       \x20 version: \"0.1.0\",\n\
       \x20 authors: [],\n\
       )\n",
      name = self.name
    );

    fs::write(project_dir.join("fret.oz"), config)
      .map_err(|e| format!("{e}"))?;

    // Write main.zo
    let main_zo = "\
      fun main() {\n\
      \x20 showln(\"hello!\");\n\
      }\n";

    fs::write(src_dir.join("main.zo"), main_zo).map_err(|e| format!("{e}"))?;

    Ok(project_dir)
  }
}

impl Handle for Init {
  fn handle(&self) {
    match self.init() {
      Ok(path) => {
        println!("Created project '{}' at {}", self.name, path.display());
        std::process::exit(EXIT_CODE_SUCCESS);
      }
      Err(e) => {
        eprintln!("Error: {e}");
        std::process::exit(EXIT_CODE_ERROR);
      }
    }
  }
}
