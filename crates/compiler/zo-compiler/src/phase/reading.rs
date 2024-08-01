use super::{On, Process};

use zo_reader::reader;
use zo_reporter::Result;
use zo_session::session::Session;

/// The reading phase.
#[derive(Clone, Copy, Debug)]
pub struct Reading;
impl Process for Reading {
  fn process(&self, session: &mut Session, on: On) -> Result<On> {
    if let On::Path(pathname) = &on {
      println!("phase:{self} — {pathname:?}");
      return reader::read(session, pathname).and_then(On::bytes);
    }

    panic!()
  }
}

impl std::fmt::Display for Reading {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "reading")
  }
}
