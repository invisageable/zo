mod program;
mod style;
mod template;

pub use program::{Expo, Num, Program};
pub use style::Style;
pub use template::{Quoted, Template};

/// The representation of a tokenizer state.
#[derive(Debug)]
pub enum TokenizerState {
  /// A program state.
  Program(Program),
  /// A style state.
  Style(Style),
  /// A template state.
  Template(Template),
}
