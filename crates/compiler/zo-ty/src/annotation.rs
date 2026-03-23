use crate::ty::TyId;

/// Annotation maps a HIR node to its type
#[derive(Clone, Copy, Debug)]
pub struct Annotation {
  /// The HIR node index
  pub node_idx: usize,
  /// The type ID for this node
  pub ty_id: TyId,
}

impl Annotation {
  /// Creates a new [`Annotation`] instance.
  pub const fn new(node_idx: usize, ty_id: TyId) -> Self {
    Self { node_idx, ty_id }
  }
}
