use zo_interner::Symbol;
use zo_span::Span;
use zo_token::Token;

use serde::Serialize;

/// Data-oriented parse tree
/// Optimized for linear traversal and cache efficiency
#[derive(Debug, Serialize)]
pub struct Tree {
  /// Primary array - 1:1 with tokens in postorder
  /// Dense packed for cache efficiency
  pub nodes: Vec<NodeHeader>,
  /// Sidecar: Node values (symbols, literals)
  /// Indexed by value_index (sparse, only for nodes with FLAG_HAS_VALUE)
  pub values: Vec<NodeValue>,
  /// Sidecar: Explicit child indices
  /// Used when children aren't contiguous in postorder
  /// Indexed by child_index (sparse, only for FLAG_HAS_EXPLICIT_CHILDREN)
  pub child_indices: Vec<u32>,
  /// Sidecar: Source spans
  /// 1:1 with nodes array
  pub spans: Vec<Span>,
  /// Value index map: node_index -> value_index
  /// Only has entries for nodes with values
  value_map: Vec<(u16, u16)>, // (node_index, value_index) pairs
}
impl Tree {
  pub fn new() -> Self {
    Self {
      nodes: Vec::with_capacity(1024),
      values: Vec::with_capacity(256),
      child_indices: Vec::with_capacity(64),
      spans: Vec::with_capacity(1024),
      value_map: Vec::with_capacity(256),
    }
  }

  /// Add a node to the tree
  pub fn push_node(&mut self, token: Token, span: Span) -> u32 {
    let index = self.nodes.len() as u32;

    let header = NodeHeader {
      token,
      flags: 0,
      child_start: 0,
      child_count: 0,
      _reserved: 0,
    };

    self.nodes.push(header);
    self.spans.push(span);

    index
  }

  /// Add a node with a value
  pub fn push_node_with_value(
    &mut self,
    token: Token,
    span: Span,
    value: NodeValue,
  ) -> u32 {
    let node_index = self.nodes.len() as u32;
    let value_index = self.values.len() as u16;

    let header = NodeHeader {
      token,
      flags: NodeHeader::FLAG_HAS_VALUE,
      child_start: 0,
      child_count: 0,
      _reserved: 0,
    };

    self.nodes.push(header);
    self.spans.push(span);
    self.values.push(value);
    self.value_map.push((node_index as u16, value_index));

    node_index
  }

  /// Set children for a node using postorder range
  pub fn set_children(
    &mut self,
    node_index: u32,
    child_start: u32,
    child_count: u16,
  ) {
    let node = &mut self.nodes[node_index as usize];

    node.child_start = child_start as u16;
    node.child_count = child_count;
  }

  /// Gets value for a node.
  pub fn value(&self, node_index: u32) -> Option<NodeValue> {
    let node = &self.nodes[node_index as usize];

    if !node.has_value() {
      return None;
    }

    // Binary search in value_map for this node
    match self
      .value_map
      .binary_search_by_key(&(node_index as u16), |(idx, _)| *idx)
    {
      Ok(i) => {
        let (_, value_idx) = self.value_map[i];

        Some(self.values[value_idx as usize])
      }
      Err(_) => None,
    }
  }

  /// Iterate children of a node
  pub fn children(
    &self,
    node_index: u32,
  ) -> impl Iterator<Item = (u32, &NodeHeader)> + '_ {
    let node = &self.nodes[node_index as usize];
    let range = node.children_range();

    range.map(move |i| (i as u32, &self.nodes[i]))
  }

  /// Get span for a node
  #[inline(always)]
  pub fn span(&self, node_index: u32) -> Span {
    self.spans[node_index as usize]
  }

  /// Replace a range of nodes with new nodes (for expression reordering)
  /// This is used by the parser for Shunting Yard algorithm
  pub fn replace_range(
    &mut self,
    start: usize,
    end: usize,
    new_nodes: Vec<(Token, Span, Option<NodeValue>)>,
  ) {
    if start >= end || start >= self.nodes.len() {
      return;
    }

    let actual_end = end.min(self.nodes.len());
    let old_count = actual_end - start;
    let new_count = new_nodes.len();

    // Collect values that need to be preserved from outside the range
    let mut preserved_values = Vec::new();
    let mut preserved_map = Vec::new();

    for &(node_idx, value_idx) in &self.value_map {
      let node_pos = node_idx as usize;

      if node_pos < start {
        // Before the range - keep as is
        preserved_map.push((node_idx, preserved_values.len() as u16));
        preserved_values.push(self.values[value_idx as usize]);
      } else if node_pos >= actual_end {
        // After the range - adjust index
        let offset = new_count as i32 - old_count as i32;
        let new_node_idx = (node_pos as i32 + offset) as u16;

        preserved_map.push((new_node_idx, preserved_values.len() as u16));
        preserved_values.push(self.values[value_idx as usize]);
      }
      // Skip nodes in the range - they'll be replaced
    }

    // Replace the node and span ranges
    let mut replacement_nodes = Vec::with_capacity(new_count);
    let mut replacement_spans = Vec::with_capacity(new_count);

    for (token, span, _) in &new_nodes {
      replacement_nodes.push(NodeHeader {
        token: *token,
        flags: 0, // Will be set below for nodes with values
        child_start: 0,
        child_count: 0,
        _reserved: 0,
      });

      replacement_spans.push(*span);
    }

    // Replace nodes in the main arrays
    self.nodes.splice(start..actual_end, replacement_nodes);
    self.spans.splice(start..actual_end, replacement_spans);

    // Add new values and update flags
    for (i, (_, _, value)) in new_nodes.into_iter().enumerate() {
      let node_idx = start + i;

      if let Some(val) = value {
        self.nodes[node_idx].flags |= NodeHeader::FLAG_HAS_VALUE;

        preserved_map.push((node_idx as u16, preserved_values.len() as u16));
        preserved_values.push(val);
      }
    }

    // Replace value arrays
    self.values = preserved_values;
    self.value_map = preserved_map;

    // Sort value map to maintain binary search invariant
    self.value_map.sort_by_key(|(node_idx, _)| *node_idx);
  }
}
impl Default for Tree {
  fn default() -> Self {
    Self::new()
  }
}

/// Compact node header - 8 bytes total
/// Designed for cache efficiency and dense packing
#[derive(Debug, Clone, Copy, Serialize)]
#[repr(C)]
pub struct NodeHeader {
  /// Token kind (1 byte)
  pub token: Token,

  /// Flags for node properties (1 byte)
  /// Bit 0: has_value (symbol or literal)
  /// Bit 1: has_explicit_children (uses child_index sidecar)
  /// Bit 2-7: reserved
  pub flags: u8,

  /// For postorder ranges: index of first child in parse buffer
  /// 0 means no children (leaf node)
  pub child_start: u16,

  /// Number of children in postorder range
  /// 0 means no children (leaf node)
  pub child_count: u16,

  /// Reserved for alignment and future use
  pub _reserved: u16,
}
impl NodeHeader {
  pub const FLAG_HAS_VALUE: u8 = 0x01;
  pub const FLAG_HAS_EXPLICIT_CHILDREN: u8 = 0x02;

  #[inline(always)]
  pub fn has_value(&self) -> bool {
    self.flags & Self::FLAG_HAS_VALUE != 0
  }

  #[inline(always)]
  pub fn has_explicit_children(&self) -> bool {
    self.flags & Self::FLAG_HAS_EXPLICIT_CHILDREN != 0
  }

  #[inline(always)]
  pub fn is_leaf(&self) -> bool {
    self.child_count == 0
  }

  /// Get the postorder range of children
  #[inline(always)]
  pub fn children_range(&self) -> std::ops::Range<usize> {
    let start = self.child_start as usize;
    let end = start + self.child_count as usize;

    start..end
  }
}

/// Node value stored in sidecar array
/// Only allocated for nodes that need values (identifiers, literals)
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum NodeValue {
  Symbol(Symbol), // Identifier symbol from interner (for compatibility)
  Literal(u32),   // Index into literal store
  TextRange(u32, u16), // Deferred: (start, length) in source - not interned yet
}
