//! WiP WiP WiP WiP WiP WiP WiP WiP WiP WiP WiP WiP WiP WiP WiP WiP WiP.
//!
//! The formatter is mini-language able to format string with those
//! arguments in a suitable way.
//!
//! The fancier output format follows the Python-like syntax. See Python
//! [docs][0].
//!
//! [0]: https://docs.python.org/3/tutorial/inputoutput.html

use crate::case::is;
use crate::cursor::Cursor;

/// The representation of a formatter;
#[derive(Debug)]
pub struct Formatter {
  /// A string buffer.
  buf: String,
}
impl Formatter {
  /// Creates a formatter;
  #[inline(always)]
  pub fn new() -> Self {
    Self { buf: String::new() }
  }

  /// Pushes a character to the string buffer;
  #[inline(always)]
  pub fn push(&mut self, ch: char) {
    self.buf.push(ch);
  }
}
impl Default for Formatter {
  fn default() -> Self {
    Self::new()
  }
}
impl std::fmt::Display for Formatter {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.buf.to_string())
  }
}
impl std::fmt::Write for Formatter {
  fn write_fmt(&mut self, args: std::fmt::Arguments<'_>) -> std::fmt::Result {
    write!(self.buf, "{args}")
  }

  fn write_str(&mut self, s: &str) -> std::fmt::Result {
    write!(self.buf, "{s}")
  }
}

/// The representation of a formatter state.
#[derive(Debug)]
pub enum FormatterState {
  /// An idle state.
  Idle,
  /// An in format state.
  InFormat,
  /// A format state.
  Format,
  /// An out format state.
  OutFormat,
  /// A name state.
  Name,
  /// A specification state.
  Spec,
}

/// A simple implementation of a sprintf-like function.
///
/// #### examples.
///
/// ```
/// use swisskit::fmt::format;
///
/// let name = "invisageable";
/// let kind = "personnage";
/// let lhs = format("L'{name:} {kind}.", &[name, kind]).unwrap();
/// let rhs = format("L'invisageable personnage.");
///
/// assert_eq!(lhs, rhs);
/// ```
pub fn format(
  source: impl AsRef<str>,
  args: &[impl ToString + std::fmt::Display],
) -> Result<String, String> {
  use std::fmt::Write;
  let mut formatter = Formatter::new();
  let mut arg_idx = 0usize;
  let mut arg_current = String::new();
  let mut state = FormatterState::Idle;
  let mut cursor = Cursor::new(source.as_ref());

  while let Some(ch) = cursor.peek() {
    match state {
      //# idle-state.
      FormatterState::Idle => match ch {
        '{' => {
          state = FormatterState::InFormat;
          cursor.next();
        }
        _ => {
          formatter.push(ch);
          cursor.next();
        }
      },
      //# in-format-state.
      FormatterState::InFormat => match ch {
        '}' => {
          state = FormatterState::Format;
          formatter.push(ch);
          cursor.next();
        }
        c if c.is_ascii_alphabetic() => {
          state = FormatterState::Name;
          arg_current.push(ch);
          cursor.next();
        }
        _ => {
          formatter.push(ch);
          cursor.next();
        }
      },
      //# format-state.
      FormatterState::Format => match ch {
        _ => {
          formatter.push(ch);
          cursor.next();
        }
      },
      //# out-format-state.
      FormatterState::OutFormat => {
        state = FormatterState::Idle;
        cursor.next();
      }
      //# name-state.
      FormatterState::Name => match ch {
        '}' => match args.get(arg_idx) {
          Some(arg) => {
            write!(&mut formatter, "{}", arg).unwrap();
            arg_idx += 1;
            state = FormatterState::OutFormat;
            arg_current.clear();
          }
          None => {
            return Err("not enough arguments provided.".into());
          }
        },
        ':' => {
          state = FormatterState::Spec;
          cursor.next();
        }
        _ => {
          arg_current.push(ch);
          cursor.next();
        }
      },
      //# spec-state.
      FormatterState::Spec => match ch {
        c if is!(space c) => {
          formatter.push(ch);
          cursor.next();
        }
        '}' => match args.get(arg_idx) {
          Some(arg) => {
            write!(&mut formatter, "{arg}").unwrap();
            arg_idx += 1;
            state = FormatterState::OutFormat;
            arg_current.clear();
          }
          None => {
            return Err("not enough arguments provided.".into());
          }
        },
        c => unimplemented!("Ch = {c} at State = {state:?}"),
      },
    }
  }

  if arg_idx != args.len() {
    if arg_idx > args.len() {
      return Err(format!(
        "too many arguments provided — expected {arg_idx} but got {}.",
        args.len()
      ));
    } else {
      return Err(format!(
        "missing arguments. {}",
        args
          .iter()
          .map(|a| a.to_string())
          .collect::<Vec<_>>()
          .join(", ")
      ));
    }
  }

  Ok(formatter.to_string())
}
