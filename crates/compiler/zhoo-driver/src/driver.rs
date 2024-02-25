use super::cmd;

use clap::Parser;

pub fn main() {
  cmd::Cmd::parse().run();
}
