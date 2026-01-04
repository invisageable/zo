use zo_tree::Tree;

/// Represents a printable node of tree.
pub struct Printee<'p> {
  pub(crate) tree: &'p Tree,
  pub(crate) node_idx: usize,
  pub(crate) source: &'p str,
  pub(crate) depth: usize,
  pub(crate) is_last: bool,
  pub(crate) parent_continues: Vec<bool>,
  pub(crate) printed: Vec<bool>,
}
