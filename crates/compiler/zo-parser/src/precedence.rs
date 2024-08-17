use zo_tokenizer::token::Token;

/// The representation of a precedence.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub(crate) enum Precedence {
  /// low level precedence.
  Low,
  /// `+=`.   
  Assignement,
  /// `&&`, `||`.
  Conditional,
  /// `==`, `<`, `>`, `<=`, `>=`.
  Comparison,
  /// `+`, `-`.
  Sum,
  /// `*`, `/`, `%`.
  Exponent,
  /// `foo()`.      
  Calling,
  /// `bar[index]`.
  Index,
  /// `foo.bar`.
  Chaining,
}

impl From<Option<&Token>> for Precedence {
  #[inline]
  fn from(maybe_token: Option<&Token>) -> Self {
    maybe_token
      .map(|token| match &token.kind {
        k if k.is_assignment() => Self::Assignement,
        k if k.is_conditional() => Self::Conditional,
        k if k.is_comparison() => Self::Comparison,
        k if k.is_sum() => Self::Sum,
        k if k.is_exponent() => Self::Exponent,
        k if k.is_calling() => Self::Calling,
        k if k.is_index() => Self::Index,
        k if k.is_chaining() => Self::Chaining,
        _ => Self::Low,
      })
      .unwrap()
  }
}
