use zo_ast::ast::Ast;
use zo_builder::builder::Output;
use zo_reporter::Result;
use zo_tokenizer::token::Token;
use zo_value::value::Value;

/// The representation of compiler's event.
#[derive(Debug)]
pub enum Event {
  Path(std::path::PathBuf),
  Bytes(Vec<u8>),
  Tokens(Vec<Token>),
  Ast(Ast),
  Bytecode(Box<[u8]>),
  Output(Output),
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
  pub const fn bytes(bytes: Vec<u8>) -> Result<Self> {
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
