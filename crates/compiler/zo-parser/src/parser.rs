use zo_ast::ast::Ast;
use zo_interner::interner::Interner;
use zo_reporter::reporter::Reporter;
use zo_reporter::Result;
use zo_session::session::Session;
use zo_tokenizer::token::Token;

/// The representation of a parser.
struct Parser<'tokens> {
  tokens: &'tokens [Token],
  interner: &'tokens mut Interner,
  reporter: &'tokens Reporter,
}

impl<'tokens> Parser<'tokens> {
  /// Creates a new parser instance from tokens, interner and reporter.
  #[inline]
  fn new(
    tokens: &'tokens [Token],
    interner: &'tokens mut Interner,
    reporter: &'tokens Reporter,
  ) -> Self {
    Self {
      tokens,
      interner,
      reporter,
    }
  }

  /// Transform an collection of tokens into an abstract syntax tree.
  ///
  /// #### result.
  ///
  /// The resulting is an AST.
  fn parse(&mut self) -> Result<Ast> {
    let mut ast = Ast::new();

    self.reporter.abort_if_has_errors();

    Ok(ast)
  }
}

/// A wrapper of [`Parser::new`] and [`Parser::parse`].
///
/// ```ignore
/// use zo_parser::parser;
/// use zo_session::session::Session;
/// use zo_tokenizer::tokenizer;
///
/// let mut session = Session::default();
/// let tokens = tokenizer::tokenize(&mut session, b"4 + 2");
///
/// parser::parse(&mut session, &tokens);
/// ```
pub fn parse(session: &mut Session, tokens: &[Token]) -> Result<Ast> {
  Parser::new(tokens, &mut session.interner, &session.reporter).parse()
}
