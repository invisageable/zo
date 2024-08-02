use zo_ast::ast::Ast;
use zo_reporter::Result;
use zo_tokenizer::token::Token;

/// The representation of compiler's event.
#[derive(Debug)]
pub enum Event {
  Path(std::path::PathBuf),
  Bytes(Vec<u8>),
  Tokens(Vec<Token>),
  Ast(Ast),
  // Bytecode(Vec<u8>),
  // Value(Value),
  // Output(Output),
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
}
