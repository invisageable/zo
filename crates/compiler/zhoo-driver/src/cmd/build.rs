use crate::cmd::Handle;

use zhoo_compiler::compiler::Compiler;
use zhoo_compiler::phase::analyzing::Analyzing;
use zhoo_compiler::phase::building::Building;
use zhoo_compiler::phase::generating::Generating;
use zhoo_compiler::phase::parsing::Parsing;
use zhoo_compiler::phase::reading::Reading;
use zhoo_compiler::phase::tokenizing::Tokenizing;
use zhoo_compiler::phase::Phase;
use zhoo_session::session::Session;

use zo_core::Result;

use clap::Parser;

#[derive(Parser)]
pub(crate) struct Build {
  #[clap(short, long, default_value = "false")]
  pub verbose: bool,
  #[clap(short, long)]
  pub input: smol_str::SmolStr,
  #[clap(short, long, default_value = "wasm")]
  pub target: smol_str::SmolStr,
  #[clap(short, long, default_value = "false")]
  pub release: bool,
  #[clap(short, long, default_value = "false")]
  pub profile: bool,
}

impl Build {
  #[inline]
  pub fn compile(&self) -> Result<()> {
    self.compiling()
  }

  #[inline]
  fn compiling(&self) -> Result<()> {
    let mut session = Session {};

    let compiler = Compiler::new()
      .add_phase(Phase::Reading(Reading {}))
      .add_phase(Phase::Tokenizing(Tokenizing {}))
      .add_phase(Phase::Parsing(Parsing {}))
      .add_phase(Phase::Analyzing(Analyzing {}))
      .add_phase(Phase::Generating(Generating {}))
      .add_phase(Phase::Building(Building {}));

    compiler.compile(&mut session)?;

    compiler.finish()
  }
}

impl Handle for Build {
  #[inline]
  fn handle(&self) {
    match self.compile() {
      Ok(_) => std::process::exit(0),
      Err(_) => std::process::exit(1),
    }
  }
}
