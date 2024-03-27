use super::cmd;

use clap::Parser;

pub fn drive() {
  cmd::Cmd::parse().run();
}
