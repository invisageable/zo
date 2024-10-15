use super::cmd;

use clap::Parser;

/// The entry point of the driver.
#[inline]
pub fn main() {
  cmd::Cmd::parse().run();
}
