pub mod case;
pub mod color;
pub mod dsa;
pub mod fmt;
pub mod fs;
pub mod interner;
pub mod reporter;
pub mod span;
pub mod system;
pub mod timer;

#[derive(Debug)]
pub struct Report;

#[derive(Debug)]
pub enum ReportError {}

pub trait Error: Sized {
  fn report(&self) -> Report;
}

pub type Result<R> = anyhow::Result<R, ReportError>;
