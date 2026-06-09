use crate::args;
use crate::cmd;
use crate::cmd::Handle;

use zo_compiler::{Compiler, DiagnosticsConfig};
use zo_error::Error;

use std::process::Command;

#[derive(clap::Args, Debug)]
pub(crate) struct Test {
  #[command(flatten)]
  pub(crate) args: args::Args,
}

impl Test {
  fn test(&self) -> Result<(), Error> {
    let source_files: Vec<_> = self
      .args
      .files
      .iter()
      .map(|path| (path, cmd::read_source(path)))
      .collect();

    let first_path = &source_files[0].0;
    let search_paths = cmd::search_paths(first_path);
    let mut compiler = Compiler::with_search_paths(search_paths);

    compiler.configure_diagnostics(DiagnosticsConfig {
      format: self.args.format.into(),
      snippet_context: self.args.snippet_context,
      explain_decisions: self.args.explain_decisions,
      use_colors: self.args.use_colors(),
    });

    compiler.set_test_mode(true);

    let temp_dir =
      std::env::temp_dir().join(format!("zo_test_{}", std::process::id()));

    std::fs::create_dir_all(&temp_dir).ok();

    let binary_path = temp_dir.join("test_binary");

    compiler.compile_test(
      &source_files,
      self.args.target.into(),
      &binary_path,
    )?;

    let source_dir = first_path
      .parent()
      .map(|parent| parent.to_path_buf())
      .unwrap_or_else(|| std::env::current_dir().unwrap());

    let status = Command::new(&binary_path)
      .current_dir(&source_dir)
      .stdin(std::process::Stdio::inherit())
      .stdout(std::process::Stdio::inherit())
      .stderr(std::process::Stdio::inherit())
      .status();

    let _ = std::fs::remove_dir_all(&temp_dir);

    match status {
      Ok(exit) if exit.success() => Ok(()),
      Ok(_) => Err(Error::new(
        zo_error::ErrorKind::InternalCompilerError,
        zo_span::Span::ZERO,
      )),
      Err(reason) => {
        eprintln!("failed to run test binary: {reason}");
        Err(Error::new(
          zo_error::ErrorKind::InternalCompilerError,
          zo_span::Span::ZERO,
        ))
      }
    }
  }
}

impl Handle for Test {
  fn handle(&self) {
    cmd::handle_with_watch(&self.args, || self.test());
  }
}
