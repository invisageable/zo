pub mod eval;
pub mod internal;
pub mod lexical;
pub mod semantic;
pub mod syntax;

pub use super::report::Report;

/// The representation of a diagnostic.
///
/// `zo` provides nice-looking and more precised error messages using `ariadne`
/// crate. An error must implements this trait to be handle by the reporter.
pub trait Diagnostic<'a> {
  fn report(&self) -> Report<'a>;
}

/// The representation of a compiler's error.
///
/// [`Error`] are group in the following way:
///
/// * Io.
/// * Lexical.
/// * Syntax.
/// * Semantic.
/// * Eval.
///
/// note — By modifying this enum, you should update the [get the line from
/// Reporter::add_error]
#[derive(Debug)]
pub enum Error {
  /// A wrapper related to [`std::io::Error`].
  Internal(internal::Internal),
  /// An error used by the tokenizer during the lexical analysis.
  Lexical(lexical::Lexical),
  /// An error used by the parser during the syntax analysis.
  Syntax(syntax::Syntax),
  /// An error used by the analyzer during the semantic analysis.
  Semantic(semantic::Semantic),
  /// An error used by the interpreter during the evaluation.
  Eval(eval::Eval),
}

impl Error {
  /// Gets the code of an error.
  ///
  /// #### notes.
  ///
  /// This is require by `ariadne` crate to display the current code report.
  #[inline]
  fn as_code(&self) -> i32 {
    match self {
      Self::Internal(_) => 0,
      Self::Lexical(_) => 1,
      Self::Syntax(_) => 2,
      Self::Semantic(_) => 3,
      Self::Eval(_) => 5,
    }
  }
}

impl std::fmt::Display for Error {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{:04}", self.as_code())
  }
}
