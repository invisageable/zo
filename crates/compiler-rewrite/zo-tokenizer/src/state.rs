mod program;
mod style;
mod template;

pub use program::Program;
pub use style::Style;

#[derive(Debug)]
pub enum TokenizerState {
  ProgramData,
  StyleData,
  TemlateData,
}
