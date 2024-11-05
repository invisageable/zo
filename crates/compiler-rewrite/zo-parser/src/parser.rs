use zor_interner::interner::Interner;
use zor_tokenizer::tokenizer::Tokenizer;

/// The representation of a parser.
pub struct Parser<'source> {
  /// See [`Tokenizer`].
  tokenizer: Tokenizer<'source>,
  // /// See [`Interner`].
  // interner: &'source mut Interner,
}

impl<'source> Parser<'source> {
  /// Creates a new parser.
  pub fn new(
    source: &'source str,
    skip_whitespace: bool,
    interner: &'source mut Interner,
  ) -> Self {
    Self {
      tokenizer: Tokenizer::new(source, skip_whitespace, interner),
      // interner,
    }
  }
}

#[cfg(test)]
mod tests {}
