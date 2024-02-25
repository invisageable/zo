use serde_derive::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Span {
  pub lo: usize,
  pub hi: usize,
}
