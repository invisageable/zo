mod args;
mod cmd;
mod constants;
mod driver;
mod watch;

pub(crate) use driver::Driver;

use clap::Parser;

pub fn run() {
  Driver::parse().run();
}
