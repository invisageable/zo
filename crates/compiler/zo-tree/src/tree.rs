use zo_interner::Symbol;
use zo_span::Span;
use zo_token::Token;

use serde::Serialize;

/// Data-oriented parse tree
/// Optimized for linear traversal and cache efficiency
#[derive(Debug, Serialize)]
pub struct Tree {
  /// Primary array - 1:1 with tokens in postorder
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

/// Captured lengths of every internal vector of a `Tree`.
/// Paired with `LiteralStoreBaseline` so a cached parsed
/// tree shared across multiple analyze invocations can
/// be rewound to its post-parse state between cross-
/// module generic splices.
#[derive(Clone, Copy, Debug)]
pub struct TreeBaseline {
  pub nodes_len: usize,
  pub spans_len: usize,
  pub values_len: usize,
  pub value_map_len: usize,
  pub child_indices_len: usize,
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

  /// Snapshot of every parallel-vector length. Captured at
  /// parse time so a cached tree can be rewound to its
  /// post-parse state between cross-module splices.
  pub fn baseline(&self) -> TreeBaseline {
    TreeBaseline {
      nodes_len: self.nodes.len(),
      spans_len: self.spans.len(),
      values_len: self.values.len(),
      value_map_len: self.value_map.len(),
      child_indices_len: self.child_indices.len(),
    }
  }

  /// Rewinds every parallel vector to a saved baseline.
  /// Splice always appends, so a tail-truncate is enough —
  /// no per-entry filtering. Used by the compiler driver
  /// to keep `parse_cache` idempotent between analyze
  /// invocations: without it, the second analyze of a
  /// cascaded preload module accumulates a tail-second-
  /// splice past the `splice_boundary` cap and the main
  /// pass walks into the second splice's `$T` tokens.
  pub fn truncate_to(&mut self, baseline: TreeBaseline) {
    self.nodes.truncate(baseline.nodes_len);
    self.spans.truncate(baseline.spans_len);
    self.values.truncate(baseline.values_len);
    self.value_map.truncate(baseline.value_map_len);
    self.child_indices.truncate(baseline.child_indices_len);
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

  /// Attach a value to a node already in `nodes`, keeping
  /// the `value_map` binary-search invariant by appending —
  /// callers MUST push with monotonically non-decreasing
  /// `node_index`, which the cross-module body splice does
  /// because it always splices at the tail of the importer's
  /// tree.
  ///
  /// @note — does NOT flip `FLAG_HAS_VALUE`. The caller is
  /// expected to have pushed the node with the flag already
  /// set (splice clones the original `NodeHeader` whole, so
  /// the flag rides along).
  pub fn attach_value_tail(&mut self, node_index: u32, value: NodeValue) {
    let value_idx = self.values.len() as u16;

    self.values.push(value);
    self.value_map.push((node_index as u16, value_idx));
  }

  /// `true` when `value_map` is sorted by `node_index`. The
  /// `value()` lookup binary-searches this array, so any
  /// out-of-order pair silently misroutes the lookup.
  /// Debug-only check site for the cross-module body
  /// splice — release stays branchless.
  pub fn value_map_is_sorted(&self) -> bool {
    self.value_map.windows(2).all(|w| w[0].0 <= w[1].0)
  }

  /// Gets value for a node.
  pub fn value(&self, node_index: u32) -> Option<NodeValue> {
    let node = &self.nodes[node_index as usize];

    if !node.has_value() {
      return None;
    }

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

  /// `true` when `nodes[idx]` is immediately preceded by a
  /// `Token::Pub` modifier. Shared by every site that emits
  /// `Pubness` for a top-level item (`execute_fun`,
  /// `execute_struct`, `execute_pack`, the lib.zo manifest
  /// scan in zo-compiler).
  #[inline]
  pub fn is_pub_at(&self, idx: usize) -> bool {
    idx > 0
      && self
        .nodes
        .get(idx - 1)
        .is_some_and(|n| n.token == Token::Pub)
  }

  /// Symbol of the first `Token::Ident` child of `node_idx`,
  /// or `None` when the first ident child carries no symbol
  /// value. Used by introducer scans that look up the
  /// declared name of an item — `pack X;`, `fun X(...)`,
  /// etc. Returns the symbol attached to the FIRST ident
  /// encountered, which matches zo's grammar where the
  /// declared name always appears as the leading ident
  /// child of an introducer.
  ///
  /// `node_idx` is trusted — callers pass indices obtained
  /// from `nodes_with_token` or the parser's own bookkeeping,
  /// matching the contract of every other `Tree` accessor.
  pub fn first_ident_child_symbol(&self, node_idx: usize) -> Option<Symbol> {
    let node = &self.nodes[node_idx];

    for child_idx in node.children_range() {
      let child = &self.nodes[child_idx];
      if child.token == Token::Ident {
        let Some(NodeValue::Symbol(sym)) = self.value(child_idx as u32) else {
          return None;
        };
        return Some(sym);
      }
    }

    None
  }

  /// Iterate `(index, node)` pairs for every node whose
  /// token kind equals `tok`. Replaces the hand-rolled
  /// `for (i, node) in tree.nodes.iter().enumerate()` +
  /// `if node.token != X { continue; }` pattern in the
  /// compiler's introducer scans (`scan_loads`,
  /// `scan_packs`).
  pub fn nodes_with_token(
    &self,
    tok: Token,
  ) -> impl Iterator<Item = (usize, &NodeHeader)> + '_ {
    self
      .nodes
      .iter()
      .enumerate()
      .filter(move |(_, n)| n.token == tok)
  }

  /// `true` when the first non-`Token::Pub` node at the top
  /// of the tree has token kind `tok`. Used by the
  /// executor's implicit-pack synthesis to detect whether
  /// the file already opens with an explicit `pack X;` (in
  /// which case synthesis is suppressed).
  pub fn top_level_starts_with(&self, tok: Token) -> bool {
    for node in self.nodes.iter() {
      match node.token {
        Token::Pub => continue,
        t => return t == tok,
      }
    }

    false
  }

  /// Get span for a node
  #[inline(always)]
  pub fn span(&self, node_index: u32) -> Span {
    self.spans[node_index as usize]
  }

  /// Zero-length span pointing at the end of the source.
  /// Used by file-level diagnostics that have no specific
  /// node to anchor on (e.g. "this file declares no main")
  /// — the caret falls past the last token, matching where
  /// a user would type the fix.
  #[inline]
  pub fn eof_span(&self) -> Span {
    let end = self.spans.last().map(|s| s.end()).unwrap_or(0);
    Span::new(end, 0)
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
