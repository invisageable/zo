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
use zhoo_session::settings::Settings;

use zo_core::fmt::sep_newline;
use zo_core::mpsc::channel;
use zo_core::reporter::report::pasteboard::Pasteboard;
use zo_core::{Result, EXIT_FAILURE, EXIT_SUCCESS};

use arboard::Clipboard;
use clap::Parser;
use smol_str::{SmolStr, ToSmolStr};

#[derive(Parser)]
#[clap(about = "Pretty print `tokens`, `ast`, `hir`, etc")]
pub(crate) struct Print {
  #[clap(short, long, default_value = "false")]
  verbose: bool,
  #[clap(short, long)]
  input: SmolStr,
  #[clap(long, default_value = "wasm")]
  backend: SmolStr,
  #[clap(short, long, default_value = "false")]
  profile: bool,
  #[clap(long, default_value = "false")]
  tokens: bool,
  #[clap(long, default_value = "false")]
  ast: bool,
  #[clap(long, default_value = "false")]
  hir: bool,
  #[clap(long, default_value = "false")]
  bytecode: bool,
  #[clap(long, default_value = "false")]
  ir: bool,
}

impl Print {
  #[inline]
  fn print(&self) -> Result<()> {
    self.printing()
  }

  fn printing(&self) -> Result<()> {
    let mut session = Session {
      settings: Settings {
        input: self.input.to_owned(),
        backend: self.backend.to_owned().into(),
        profile: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
          self.profile,
        )),
        verbose: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
          self.verbose,
        )),
      },
      ..Default::default()
    };

    let mut state = State::Idle;
    let mut result = SmolStr::default();

    let (rx_reading, tx_reading) = channel::bounded(channel::CAPACITY);
    let (rx_tokenizing, tx_tokenizing) = channel::bounded(channel::CAPACITY);
    let (rx_parsing, tx_parsing) = channel::bounded(channel::CAPACITY);
    let (rx_analyzing, tx_analyzing) = channel::bounded(channel::CAPACITY);
    let (rx_generating, tx_generating) = channel::bounded(channel::CAPACITY);
    let (rx_building, tx_building) = channel::bounded(channel::CAPACITY);

    let mut compiler =
      Compiler::new().add_phase(Phase::Reading(Reading { rx: rx_reading }));

    if self.tokens || self.ast || self.hir || self.bytecode || self.ir {
      compiler = compiler.add_phase(Phase::Tokenizing(Tokenizing {
        rx: rx_tokenizing,
        tx: tx_reading,
      }));
    }

    if self.ast || self.hir || self.bytecode || self.ir {
      compiler = compiler.add_phase(Phase::Parsing(Parsing {
        rx: rx_parsing,
        tx: tx_tokenizing.to_owned(),
      }));
    }

    if self.hir || self.bytecode || self.ir {
      compiler = compiler.add_phase(Phase::Analyzing(Analyzing {
        rx: rx_analyzing,
        tx: tx_parsing.to_owned(),
      }));
    }

    if self.bytecode || self.ir {
      compiler = compiler.add_phase(Phase::Generating(Generating {
        rx: rx_generating,
        tx: tx_analyzing.to_owned(),
      }));
    }

    if self.ir {
      compiler = compiler.add_phase(Phase::Building(Building {
        rx: rx_building,
        tx: tx_generating.to_owned(),
      }));
    }

    compiler.compile(&mut session)?;

    loop {
      match state {
        State::Idle => {
          state = match true {
            _ if self.tokens => State::Tokens,
            _ if self.ast => State::Ast,
            _ if self.hir => State::Hir,
            _ if self.bytecode => State::Bytecode,
            _ if self.ir => State::Ir,
            _ => State::End,
          };
        }
        State::Tokens => {
          compiler.finish(tx_tokenizing.to_owned()).map(|tokens| {
            if self.tokens {
              state = State::End;
            } else {
              state = State::Ast;
            }

            result = SmolStr::new(sep_newline(&tokens));

            print!("print tokens.");
          })?;
        }
        State::Ast => {
          compiler.finish(tx_parsing.to_owned()).map(|ast| {
            if self.ast {
              state = State::End;
            } else {
              state = State::Hir;
            }

            result = ast.to_smolstr();
          })?;
        }
        State::Hir => {
          compiler.finish(tx_analyzing.to_owned()).map(|hir| {
            if self.hir {
              state = State::End;
            } else {
              state = State::Bytecode;
            }

            result = hir.to_smolstr();
          })?;
        }
        State::Bytecode => {
          compiler.finish(tx_generating.to_owned()).map(|bytecode| {
            if self.bytecode {
              state = State::End;
            } else {
              state = State::Ir;
            }

            result = format!("{bytecode:?}").to_smolstr();
          })?;
        }
        State::Ir => {
          compiler.finish(tx_building.to_owned()).map(|output| {
            state = State::End;

            result = format!("{output:?}").to_smolstr();
          })?;
        }
        State::End => break,
      }
    }

    println!("{result}");

    // Clipboard::new()
    //   .map_err(Pasteboard::not_supported)?
    //   .set_text(result.as_str())
    //   .map_err(Pasteboard::unknown)?;

    Clipboard::new()
      .and_then(|mut clipboard| clipboard.set_text(result.as_str()))
      .map_err(Pasteboard::unknown)?;

    println!("\n👉 added to the clipboard.\n",);

    Ok(())
  }
}

impl Handle for Print {
  #[inline]
  fn handle(&self) {
    match self.print() {
      Ok(_) => std::process::exit(EXIT_SUCCESS),
      Err(_) => std::process::exit(EXIT_FAILURE),
    }
  }
}

pub(crate) enum State {
  Idle,
  Tokens,
  Ast,
  Hir,
  Bytecode,
  Ir,
  End,
}
