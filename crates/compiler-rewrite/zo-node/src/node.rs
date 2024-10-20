/// The representation of a node.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct Node(pub std::num::NonZeroU32);

impl Node {
  /// Creates a new dummy node.
  pub const DUMMY: Self = Self(std::num::NonZeroU32::MAX);
}
