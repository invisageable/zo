use zo_ast::ast::Ast;
use zo_builder::output::Output;
use zo_reporter::Result;
use zo_tokenizer::token::Token;
use zo_value::value::Value;

use swisskit::fmt::sep_comma;

use smol_str::{SmolStr, ToSmolStr};

/// The representation of compiler's event.
#[derive(Debug)]
pub enum Event {
  /// A path event — used during the `reading` phase.
  Path(std::path::PathBuf),
  /// A bytes event — used during the `tokenizer` phase.
  Bytes(std::collections::HashMap<String, String>),
  /// A token event — used during the `parsing` phase.
  Tokens(Vec<Token>),
  /// An AST event — used during the `analyzing` and `generating` phase.
  Ast(Ast),
  /// A bytecode event — used during the `building` phase.
  Bytecode(Box<[u8]>),
  /// An output event — used to display the `building` phase result.
  Output(Output),
  /// A value event — used during the `interpreting` phase.
  Value(Value),
}

impl Event {
  /// Creates a new path event.
  #[inline]
  pub const fn path(path: std::path::PathBuf) -> Result<Self> {
    Ok(Event::Path(path))
  }

  /// Creates a new bytes event.
  #[inline]
  pub const fn bytes(
    bytes: std::collections::HashMap<String, String>,
  ) -> Result<Self> {
    Ok(Event::Bytes(bytes))
  }

  /// Creates a new tokens event.
  #[inline]
  pub const fn tokens(tokens: Vec<Token>) -> Result<Self> {
    Ok(Event::Tokens(tokens))
  }

  /// Creates a new ast event.
  #[inline]
  pub const fn ast(ast: Ast) -> Result<Self> {
    Ok(Event::Ast(ast))
  }

  /// Creates a new bytecode event.
  #[inline]
  pub fn bytecode(bytecode: Box<[u8]>) -> Result<Self> {
    Ok(Event::Bytecode(bytecode))
  }

  /// Creates a new bytecode event.
  #[inline]
  pub const fn output(output: Output) -> Result<Self> {
    Ok(Event::Output(output))
  }

  /// Creates a new value event.
  #[inline]
  pub const fn value(value: Value) -> Result<Self> {
    Ok(Event::Value(value))
  }
}

impl From<Event> for SmolStr {
  #[inline]
  fn from(event: Event) -> Self {
    event.to_smolstr()
  }
}

impl std::fmt::Display for Event {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Path(pathname) => write!(f, "{}", pathname.display()),
      Self::Bytes(bytes) => write!(f, "{:?}", bytes),
      Self::Tokens(tokens) => write!(f, "{}", sep_comma(tokens)),
      Self::Ast(ast) => write!(f, "{ast}"),
      Self::Bytecode(bytecode) => write!(f, "{}", sep_comma(bytecode)),
      Self::Output(output) => write!(f, "{output}"),
      Self::Value(value) => write!(f, "{value}"),
    }
  }
}
