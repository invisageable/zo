use crate::args;
use crate::cmd;
use crate::cmd::Handle;

use zo_compiler::{Compiler, DiagnosticsConfig, Stage};
use zo_error::Error;

#[derive(clap::Args, Debug)]
pub(crate) struct Build {
  #[command(flatten)]
  pub(crate) args: args::Args,
}

impl Build {
  fn build(&self) -> Result<(), Error> {
    let source_files: Vec<_> = self
      .args
      .files
      .iter()
      .map(|p| (p, cmd::read_source(p)))
      .collect();

    let first_path = &source_files[0].0;
    let search_paths = cmd::search_paths(first_path);
    let mut compiler = Compiler::with_search_paths(search_paths);
    compiler.configure_diagnostics(DiagnosticsConfig {
      json: self.args.format == args::Format::Json,
      snippet_context: self.args.snippet_context,
      explain_decisions: self.args.explain_decisions,
    });

    let stages = self
      .args
      .emit
      .iter()
      .map(|stage| match stage {
        args::Stage::Tokens => Stage::Tokens,
        args::Stage::Tree => Stage::Tree,
        args::Stage::Sir => Stage::Sir,
        args::Stage::Asm => Stage::Asm,
        args::Stage::All => Stage::All,
      })
      .collect::<Vec<_>>();

    compiler.compile(
      &source_files,
      self.args.target.into(),
      &stages,
      &self.args.output,
      self.args.out_dir.as_deref(),
    )
  }
}

impl Handle for Build {
  fn handle(&self) {
    cmd::handle_with_watch(&self.args, || self.build());
  }
}
