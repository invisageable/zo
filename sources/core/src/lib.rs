pub mod dsa;
pub mod reporter;

#[derive(Debug)]
pub struct Report;

#[derive(Debug)]
pub enum ReportError {}

pub trait Error: Sized {
  fn report(&self) -> Report;
}

pub type Result<R> = anyhow::Result<R, ReportError>;
