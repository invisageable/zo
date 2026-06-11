//! Cross-module generic body splice — structural round-trip
//! check. PLAN_CROSS_MODULE_GENERICS phase 3 lock-in.
//!
//! Builds a tiny "origin" tree by hand, snapshots a subrange
//! as an `ExportedGenericBody`, splices it into a separate
//! "host" tree, and verifies that every spliced node's
//! parent-child edge resolves inside the spliced range and
//! that literal payloads land at valid slots in the host's
//! `LiteralStore`. The test exists because phase 4 leans on
//! this path — a missed `NodeId`-bearing field would
//! silently corrupt re-execution without crashing the
//! pipeline.

use crate::exports::{
  ExportedGenericBody, ExportedLiteral, ExportedTreeSlice, ModuleHarvest,
  splice_generic_bodies,
};

use zo_interner::Interner;
use zo_span::Span;
use zo_token::{LiteralStore, Token};
use zo_tree::{NodeValue, Tree};

/// Layout for the synthetic origin body — postorder, shape
/// of `fun first(self) -> $T { return self[0]; }`:
///
/// ```text
/// 0: Fun
/// 1: Ident("first")
/// 2: Int(0)                    <- literal
/// 3: Return  (children=[2])
/// ```
///
/// Only the structural shape matters — we just need at
/// least one literal-bearing node and one parent-with-children
/// node to exercise both rebase paths.
fn build_origin_body(
  interner: &mut Interner,
  literals: &mut LiteralStore,
) -> (Tree, ExportedGenericBody) {
  let mut tree = Tree::new();
  let span = Span::ZERO;

  // Pre-splice host noise: put one unrelated node up front
  // so the origin range starts at index 1, not 0. Forces
  // the rebase to handle a non-zero `origin_start`.
  let _filler = tree.push_node(Token::Ident, span);

  let origin_start = tree.nodes.len() as u32;

  let fn_sym = interner.intern("first");
  let n_fun = tree.push_node(Token::Fun, span);
  let _n_name =
    tree.push_node_with_value(Token::Ident, span, NodeValue::Symbol(fn_sym));

  // Literal `0` lives at index `lit_idx` in the origin's
  // `LiteralStore` — the splice has to remap it to the
  // host's store.
  let lit_idx = literals.push_int(0);
  let n_int =
    tree.push_node_with_value(Token::Int, span, NodeValue::Literal(lit_idx));

  let n_return = tree.push_node(Token::Return, span);

  // Mark `Return`'s child as the `Int` node — exercises
  // `child_start` rebase.
  tree.set_children(n_return, n_int, 1);

  // And mark `Fun`'s children as the whole [name, int, return]
  // range — also rebased.
  tree.set_children(n_fun, n_fun + 1, 3);

  let origin_end = tree.nodes.len() as u32;

  // Clone the subrange into the export payload.
  let body_nodes =
    tree.nodes[origin_start as usize..origin_end as usize].to_vec();
  let body_spans =
    tree.spans[origin_start as usize..origin_end as usize].to_vec();

  let mut body_values = Vec::new();

  for idx in origin_start..origin_end {
    if let Some(value) = tree.value(idx) {
      body_values.push((idx, value));
    }
  }

  let body = ExportedGenericBody {
    name: interner.intern("arr_$::first"),
    slice: ExportedTreeSlice {
      nodes: body_nodes,
      spans: body_spans,
      node_values: body_values,
      literal_payloads: vec![(n_int, ExportedLiteral::Int(0))],
      origin_start,
    },
    type_params: vec![interner.intern("T")],
    apply_context: interner.intern("arr_$"),
  };

  (tree, body)
}

/// Build a host tree containing a few unrelated nodes — the
/// splice must land at its tail without touching the existing
/// prefix.
fn build_host_tree() -> (Tree, LiteralStore) {
  let mut tree = Tree::new();
  let span = Span::ZERO;
  let literals = LiteralStore::new();

  // Three filler nodes so the splice offset is non-trivial.
  for _ in 0..3 {
    let _ = tree.push_node(Token::Ident, span);
  }

  (tree, literals)
}

#[test]
fn splice_round_trip_keeps_parent_child_edges_in_range() {
  let mut interner = Interner::new();
  let mut origin_literals = LiteralStore::new();

  let (_origin, body) = build_origin_body(&mut interner, &mut origin_literals);
  let (mut host, mut host_literals) = build_host_tree();

  let host_pre_splice_len = host.nodes.len();
  let pre_int_literals = host_literals.int_literals.len();

  let spliced =
    splice_generic_bodies(&mut host, &mut host_literals, vec![body]);

  assert_eq!(spliced.len(), 1, "expected one splice meta entry");

  let entry = &spliced[0];
  let (start, end) = entry.range;
  let start = start as usize;
  let end = end as usize;

  assert_eq!(
    start, host_pre_splice_len,
    "splice should land at the host's pre-splice tail"
  );
  assert!(end > start, "spliced range must be non-empty");
  assert_eq!(
    host.nodes.len(),
    end,
    "host tree should grow to the announced end"
  );
  assert_eq!(
    host.nodes.len(),
    host.spans.len(),
    "nodes/spans must stay parallel after splice"
  );

  // Every spliced parent's `child_start` should land
  // strictly inside the spliced range — if any survived
  // from the origin tree (no rebase), they'd point at the
  // host's filler nodes (0..3).
  for idx in start..end {
    let node = &host.nodes[idx];

    if node.child_count > 0 {
      let cs = node.child_start as usize;
      let ce = cs + node.child_count as usize;

      assert!(
        cs >= start && ce <= end,
        "spliced node at {} has child range {}..{} outside spliced {}..{}",
        idx,
        cs,
        ce,
        start,
        end
      );
    }
  }

  // Literal payload must have been pushed into the host's
  // `LiteralStore` and the spliced `NodeValue::Literal(i)`
  // must point at the new slot, not the origin slot.
  let post_int_literals = host_literals.int_literals.len();

  assert_eq!(
    post_int_literals,
    pre_int_literals + 1,
    "splice should push exactly one int literal into the host store"
  );

  let mut found_remapped_lit = false;

  for idx in start..end {
    if let Some(NodeValue::Literal(new_idx)) = host.value(idx as u32) {
      assert!(
        (new_idx as usize) < post_int_literals,
        "spliced literal index {} exceeds host store length {}",
        new_idx,
        post_int_literals
      );
      found_remapped_lit = true;
    }
  }

  assert!(
    found_remapped_lit,
    "expected at least one remapped `NodeValue::Literal` in the spliced range"
  );

  assert!(
    host.value_map_is_sorted(),
    "splice must preserve the value_map sort invariant"
  );
}

#[test]
fn splice_two_bodies_into_one_host_keeps_lookups_distinct() {
  let mut interner = Interner::new();
  let mut origin_literals = LiteralStore::new();

  let (_origin, body_a) =
    build_origin_body(&mut interner, &mut origin_literals);

  // Build a second body with a different mangled name so
  // both splices land independently.
  let mut body_b = body_a.clone();

  body_b.name = interner.intern("arr_$::last");

  let (mut host, mut host_literals) = build_host_tree();
  let spliced = splice_generic_bodies(
    &mut host,
    &mut host_literals,
    vec![body_a.clone(), body_b],
  );

  assert_eq!(spliced.len(), 2, "expected both splices to land");

  let (a_start, a_end) = spliced[0].range;
  let (b_start, b_end) = spliced[1].range;

  assert!(
    a_end as usize <= b_start as usize,
    "second splice must land strictly after the first ({}..{} vs {}..{})",
    a_start,
    a_end,
    b_start,
    b_end
  );

  // Both bodies push an int literal, so the host store
  // should contain two new entries.
  assert_eq!(
    host_literals.int_literals.len(),
    2,
    "two bodies should contribute two literal payloads"
  );

  // And the host's value_map must remain monotonic across
  // both insertions.
  assert!(
    host.value_map_is_sorted(),
    "value_map must stay sorted across multiple splices ({}..{} then {}..{})",
    a_start,
    a_end,
    b_start,
    b_end,
  );
}

#[test]
fn extract_exports_round_trips_abstract_impls() {
  use crate::exports::{AbstractImpl, extract_exports};
  use std::path::PathBuf;
  use zo_sir::Sir;
  use zo_value::Pubness;

  let mut interner = Interner::new();
  let eq_sym = interner.intern("Eq");
  let point_sym = interner.intern("Point");
  let point_eq_sym = interner.intern("Point::eq");

  let mut src_impls = rustc_hash::FxHashMap::default();
  src_impls.insert(
    (eq_sym, point_sym),
    AbstractImpl {
      methods: vec![point_eq_sym],
      defined_at: Span::ZERO,
      defining_module: PathBuf::from("/tmp/shapes.zo"),
      pubness: Pubness::Yes,
      vtable_sym: eq_sym,
    },
  );

  let private_target = interner.intern("Hidden");
  src_impls.insert(
    (eq_sym, private_target),
    AbstractImpl {
      methods: vec![],
      defined_at: Span::ZERO,
      defining_module: PathBuf::from("/tmp/shapes.zo"),
      pubness: Pubness::No,
      vtable_sym: eq_sym,
    },
  );

  let exports = extract_exports(
    ModuleHarvest {
      sir: Sir::new(),
      funs: &[],
      generic_bodies: Vec::new(),
      component_bodies: Vec::new(),
      abstract_impls: src_impls,
    },
    None,
    &interner,
  );

  assert_eq!(
    exports.abstract_impls.len(),
    1,
    "Pubness::No impls must be filtered out",
  );
  assert!(
    exports.abstract_impls.contains_key(&(eq_sym, point_sym)),
    "the public `(Eq, Point)` entry must survive the export",
  );

  let entry = exports.abstract_impls.get(&(eq_sym, point_sym)).unwrap();
  assert_eq!(entry.methods, vec![point_eq_sym]);
  assert_eq!(entry.defining_module, PathBuf::from("/tmp/shapes.zo"));
}

#[test]
fn extract_exports_filters_abstract_impls_on_selective_load() {
  use crate::exports::{AbstractImpl, extract_exports};
  use std::path::PathBuf;
  use zo_sir::Sir;
  use zo_value::Pubness;

  let mut interner = Interner::new();
  let eq_sym = interner.intern("Eq");
  let point_sym = interner.intern("Point");
  let other_sym = interner.intern("Other");

  let entry_for = |path: &str| AbstractImpl {
    methods: Vec::new(),
    defined_at: Span::ZERO,
    defining_module: PathBuf::from(path),
    pubness: Pubness::Yes,
    vtable_sym: eq_sym,
  };

  let mut src_impls = rustc_hash::FxHashMap::default();
  src_impls.insert((eq_sym, point_sym), entry_for("/tmp/a.zo"));
  src_impls.insert((eq_sym, other_sym), entry_for("/tmp/b.zo"));

  let exports = extract_exports(
    ModuleHarvest {
      sir: Sir::new(),
      funs: &[],
      generic_bodies: Vec::new(),
      component_bodies: Vec::new(),
      abstract_impls: src_impls,
    },
    Some("Point"),
    &interner,
  );

  assert_eq!(
    exports.abstract_impls.len(),
    1,
    "selective `load M::(Point);` keeps only `(_, Point)` impls",
  );
  assert!(exports.abstract_impls.contains_key(&(eq_sym, point_sym)),);
}
