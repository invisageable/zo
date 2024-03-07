use zhoo_tokenizer::token::Token;

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub(super) enum Precedence {
  Low, //
  // Equals,      // ...
  Assignement, // `+=`.
  Conditional, // `&&`, `||`.
  Comparison,  // `==`, `<`, `>`, `<=`, `>=`.
  Sum,         // `+`, `-`.
  Exponent,    // `*`, `/`, `%`.
  // Unary,       //
  Calling,  // `foo()`.
  Index,    // `bar[index]`.
  Chaining, // `x.y.z`.
}

impl From<Option<&Token>> for Precedence {
  fn from(maybe_token: Option<&Token>) -> Self {
    maybe_token
      .map(|token| match token.kind {
        kind if kind.is_assignement() => Self::Assignement,
        kind if kind.is_conditional() => Self::Conditional,
        kind if kind.is_comparison() => Self::Comparison,
        kind if kind.is_sum() => Self::Sum,
        kind if kind.is_exponent() => Self::Exponent,
        kind if kind.is_calling() => Self::Calling,
        kind if kind.is_index() => Self::Index,
        kind if kind.is_chaining() => Self::Chaining,
        _ => Self::Low,
      })
      .unwrap()
  }
}
