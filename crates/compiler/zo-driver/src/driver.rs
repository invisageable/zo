use super::cmd;

use clap::Parser;

#[inline]
pub fn main() {
  cmd::Cmd::parse().run();
}
