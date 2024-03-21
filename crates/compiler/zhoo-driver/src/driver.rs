use super::cmd;

use clap::Parser;

#[inline]
pub fn drive() {
  cmd::Cmd::parse().run();
}
