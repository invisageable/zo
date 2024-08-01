use zo_interner::interner::Interner;
use zo_reporter::reporter::Reporter;
use zo_session::session::Session;

/// The representation of the tokeinzer.
struct Tokenizer<'bytes> {
  interner: &'bytes mut Interner,
  reporter: &'bytes Reporter,
  /// The source code as bytes.
  source: &'bytes [u8],
}

impl<'bytes> Tokenizer<'bytes> {
  /// Creates a new tokenizer instance.
  #[inline]
  pub fn new(
    interner: &'bytes mut Interner,
    reporter: &'bytes Reporter,
    source: &'bytes [u8],
  ) -> Self {
    Self {
      interner,
      reporter,
      source,
    }
  }

  /// Transform the source code into an array of tokens.
  fn tokenize(self) -> Vec<u8> {
    todo!()
  }
}

/// Transform the source code into an array of tokens.
///
/// #### examples.
///
/// ```
/// use zo_tokenizer::tokenizer;
/// use zo_session::session::Session;
///
/// let mut session = Session::default();
/// let tokens = tokenizer::tokenize(&mut session, b"");
///
/// assert_eq!(tokens, vec![]);
/// ```
pub fn tokenize(session: &mut Session, source: &[u8]) -> Vec<u8> {
  Tokenizer::new(&mut session.interner, &session.reporter, source).tokenize()
}
