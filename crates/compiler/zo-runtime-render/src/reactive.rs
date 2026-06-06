//! Compile-time-driven fine-grained reactivity.
//!
//! zo emits the binding graph at compile time, so the runtime
//! never builds a subscriber graph: a state write marks its
//! slot dirty, the [`BindingGraph`] maps that slot to the exact
//! commands it drives, and only those commands are refreshed
//! and repainted. This module is the single reactive home,
//! shared by the compiled (`aot`) path and the `zo run`
//! (`StateCell`) path.
//!
//! - [`BitSet`] (`DirtySet` / `DirtyCommands`): a bit-packed,
//!   reused set of `u32` indices — no per-event allocation.
//! - [`BindingGraph`]: a CSR from state slot → the commands
//!   that slot drives, built once from the flat binding tables.
//! - [`reconcile_list`] / [`apply_list_bindings`] /
//!   [`apply_computed_bindings`]: the list + computed binding
//!   appliers, shared by the `aot` and `zo run` paths.

use crate::evaluator::HandlerEvaluator;
use crate::render::StateCell;

use zo_interner::Symbol;
use zo_sir::{Insn, ListItemCmd};
use zo_ui_protocol::{Attr, UiCommand};

/// A bit-packed set of `u32` indices, sized once and reused.
///
/// Backs both the slot dirty-set (`DirtySet`) and the command
/// dirty-set (`DirtyCommands`): a write marks an index, the
/// consumer drains or scans, then the set is cleared for the
/// next event. `mark`/`contains`/`clear` are O(1)/O(words); no
/// allocation happens on the steady-state event path.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BitSet {
  /// One bit per index: index `i` lives in `words[i / 64]` at
  /// bit `i % 64`.
  words: Box<[u64]>,
}

/// Slots written since the last drain. Drained after each
/// event dispatch to drive the [`BindingGraph`].
pub type DirtySet = BitSet;

/// Command indices touched by the dirtied slots' bindings.
/// Consumed by the view layer to repaint exactly those
/// commands instead of re-diffing the whole stream.
pub type DirtyCommands = BitSet;

const BITS_PER_WORD: usize = 64;

impl BitSet {
  /// Allocate a set covering `count` indices, rounded up to a
  /// whole 64-bit word.
  pub fn with_capacity(count: usize) -> Self {
    let words = count.div_ceil(BITS_PER_WORD);

    Self {
      words: vec![0u64; words].into_boxed_slice(),
    }
  }

  /// Number of indices this set can hold without growing.
  pub fn capacity(&self) -> usize {
    self.words.len() * BITS_PER_WORD
  }

  /// Grow (never shrink) so the set covers `count` indices.
  /// Mirrors `zo_state_init`'s grow-only resize so re-init
  /// against a larger program keeps the marked bits.
  pub fn ensure(&mut self, count: usize) {
    let needed = count.div_ceil(BITS_PER_WORD);

    if self.words.len() < needed {
      let mut grown = vec![0u64; needed];

      grown[..self.words.len()].copy_from_slice(&self.words);
      self.words = grown.into_boxed_slice();
    }
  }

  /// Mark `index`. Out-of-range is a no-op — defensive, like
  /// the `zo_state_*` helpers: a stale binary against a newer
  /// runtime fails soft, not crash.
  pub fn mark(&mut self, index: u32) {
    let i = index as usize;

    if let Some(word) = self.words.get_mut(i / BITS_PER_WORD) {
      *word |= 1u64 << (i % BITS_PER_WORD);
    }
  }

  /// Whether `index` is set. Out-of-range reads `false`.
  pub fn contains(&self, index: u32) -> bool {
    let i = index as usize;

    self
      .words
      .get(i / BITS_PER_WORD)
      .is_some_and(|word| word & (1u64 << (i % BITS_PER_WORD)) != 0)
  }

  /// No index is set.
  pub fn is_empty(&self) -> bool {
    self.words.iter().all(|&word| word == 0)
  }

  /// Clear every bit, keeping the allocation for reuse.
  pub fn clear(&mut self) {
    self.words.fill(0);
  }

  /// Visit each set index in ascending order. The classic
  /// `bits &= bits - 1` clears the lowest set bit each step,
  /// so the cost is O(set bits), not O(capacity).
  pub fn for_each_set(&self, mut f: impl FnMut(u32)) {
    for (word_idx, &word) in self.words.iter().enumerate() {
      let mut bits = word;

      while bits != 0 {
        let bit = bits.trailing_zeros();

        f((word_idx * BITS_PER_WORD) as u32 + bit);
        bits &= bits - 1;
      }
    }
  }

  /// Append every set index (ascending) to `into`, then clear.
  /// `into` is the caller's reused scratch buffer — no
  /// allocation on the event path.
  pub fn drain_into(&mut self, into: &mut Vec<u32>) {
    for (word_idx, word) in self.words.iter_mut().enumerate() {
      let mut bits = *word;

      while bits != 0 {
        let bit = bits.trailing_zeros();

        into.push((word_idx * BITS_PER_WORD) as u32 + bit);
        bits &= bits - 1;
      }

      *word = 0;
    }
  }

  /// Collect the set indices into a fresh `Vec` (test helper /
  /// non-hot callers).
  pub fn to_vec(&self) -> Vec<u32> {
    let mut out = Vec::new();

    self.for_each_set(|i| out.push(i));
    out
  }
}

/// A reactive target a state slot drives: one UI command, plus
/// the slice within it when the binding is finer-grained than
/// the whole command (an attribute, a list, a computed text).
///
/// Built once from the flat compile-time binding tables. Every
/// variant carries `cmd_idx` so the view layer can repaint the
/// touched command uniformly; the extra fields let the refresh
/// pass apply the right kind of patch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindingRef {
  /// `commands[cmd_idx]` is a `Text` regenerated from the
  /// slot's value.
  Text { cmd_idx: u32 },
  /// `commands[cmd_idx]` is an `Element` whose attribute at
  /// `attr_idx` (index into the bindings' attr table) is
  /// reactive on the slot.
  Attr { cmd_idx: u32, attr_idx: u32 },
  /// `commands[cmd_idx]` is a list anchor; `list_id` indexes
  /// the bindings' list table.
  List { cmd_idx: u32, list_id: u32 },
  /// `commands[cmd_idx]` is a computed `Text`; `computed_id`
  /// indexes the bindings' computed table.
  Computed { cmd_idx: u32, computed_id: u32 },
}

impl BindingRef {
  /// The command index this target patches.
  pub fn cmd_idx(self) -> u32 {
    match self {
      Self::Text { cmd_idx }
      | Self::Attr { cmd_idx, .. }
      | Self::List { cmd_idx, .. }
      | Self::Computed { cmd_idx, .. } => cmd_idx,
    }
  }
}

/// Reverse binding index: state slot → the commands it drives.
///
/// A compressed-sparse-row table built once at startup from the
/// flat forward binding arrays. `targets(slot)` is a contiguous
/// slice with no per-slot allocation; `dirty_commands` unions a
/// dirty-slot list into a reusable command set.
#[derive(Clone, Debug, Default)]
pub struct BindingGraph {
  /// Length `num_slots + 1`. Slot `s`'s targets are
  /// `entries[slot_offsets[s]..slot_offsets[s + 1]]`.
  slot_offsets: Box<[u32]>,
  /// Targets grouped by slot, in `slot_offsets` order.
  entries: Box<[BindingRef]>,
}

impl BindingGraph {
  /// Build the CSR from flat `(slot, target)` edges. `num_slots`
  /// bounds the slot axis; edges with `slot >= num_slots` are
  /// dropped (defensive — a malformed binding table can't push
  /// the offsets out of range). Edge order within a slot is
  /// preserved (stable counting-sort scatter).
  pub fn from_edges(num_slots: usize, edges: &[(u32, BindingRef)]) -> Self {
    // `offsets[s + 1]` accumulates slot `s`'s count, then the
    // prefix sum turns counts into start offsets in one pass.
    let mut offsets = vec![0u32; num_slots + 1];

    for &(slot, _) in edges {
      let s = slot as usize;

      if s < num_slots {
        offsets[s + 1] += 1;
      }
    }

    for s in 0..num_slots {
      offsets[s + 1] += offsets[s];
    }

    let total = offsets[num_slots] as usize;
    let mut entries = vec![BindingRef::Text { cmd_idx: 0 }; total];
    // `cursor[s]` advances as slot `s`'s edges scatter in.
    let mut cursor = offsets.clone();

    for &(slot, target) in edges {
      let s = slot as usize;

      if s < num_slots {
        let pos = cursor[s] as usize;

        entries[pos] = target;
        cursor[s] += 1;
      }
    }

    Self {
      slot_offsets: offsets.into_boxed_slice(),
      entries: entries.into_boxed_slice(),
    }
  }

  /// Number of slots the graph is built over.
  pub fn num_slots(&self) -> usize {
    self.slot_offsets.len().saturating_sub(1)
  }

  /// The targets driven by `slot`, or empty for an unbound /
  /// out-of-range slot.
  pub fn targets(&self, slot: u32) -> &[BindingRef] {
    let s = slot as usize;

    if s + 1 >= self.slot_offsets.len() {
      return &[];
    }

    let lo = self.slot_offsets[s] as usize;
    let hi = self.slot_offsets[s + 1] as usize;

    &self.entries[lo..hi]
  }

  /// Union the command indices driven by `dirty` into `out`
  /// (cleared first). The bit-set de-duplicates shared targets;
  /// `out` is the caller's reused buffer.
  pub fn dirty_commands(&self, dirty: &[u32], out: &mut DirtyCommands) {
    out.clear();

    for &slot in dirty {
      for target in self.targets(slot) {
        out.mark(target.cmd_idx());
      }
    }
  }
}

/// One edit transforming the old keyed list into the new one.
///
/// Produced by [`reconcile_list`]; consumed by the command-
/// stream and view-layer appliers to touch only the items that
/// actually changed. `Remove.from` indexes the OLD list;
/// `Insert.to` / `Move.to` index the NEW list.
///
/// With the default `[]str` value-as-key, a content change is a
/// `Remove` + `Insert` of one item (the key changed) — still
/// O(1) per change, the headline metric. A stable explicit key
/// over a changing payload would instead surface an in-place
/// update; that path is unbuilt (no surface syntax yet).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListEdit {
  /// The new item at new-list index `to` has no old
  /// counterpart — render it fresh.
  Insert { to: u32 },
  /// The old item at old-list index `from` is gone in the new
  /// list — drop it.
  Remove { from: u32 },
  /// The old item at `from` is reused at new-list position
  /// `to`. Emitted only for items whose relative order
  /// actually changed; in-order survivors carry no edit.
  Move { from: u32, to: u32 },
}

/// Diff two keyed lists into a minimal edit script.
///
/// `old` / `new` are the per-item keys (for `[]str`, the item
/// strings). Matching is multiset-positional: duplicate keys
/// pair first-old-with-first-new, so a list with repeats still
/// diffs sensibly. Surviving items that keep their relative
/// order (the longest increasing subsequence of reused old
/// indices) carry no edit; everything else is the minimal set
/// of inserts, removes, and moves.
///
/// Edit order: every `Remove` (ascending old index) first, then
/// `Insert` / `Move` in new-list order — so a view applier can
/// drop gone items, then place the rest left-to-right.
pub fn reconcile_list<K>(old: &[K], new: &[K]) -> Vec<ListEdit>
where
  K: Eq + std::hash::Hash,
{
  use std::collections::HashMap;
  use std::collections::VecDeque;

  // key → old indices in order. A queue (not a single index)
  // so repeated keys pair positionally instead of collapsing.
  let mut old_positions: HashMap<&K, VecDeque<u32>> = HashMap::new();

  for (i, key) in old.iter().enumerate() {
    old_positions.entry(key).or_default().push_back(i as u32);
  }

  // Source old index for each new item (`None` = fresh insert).
  let mut new_to_old: Vec<Option<u32>> = Vec::with_capacity(new.len());
  let mut matched_old = vec![false; old.len()];

  for key in new {
    let source = old_positions.get_mut(key).and_then(VecDeque::pop_front);

    if let Some(old_idx) = source {
      matched_old[old_idx as usize] = true;
    }

    new_to_old.push(source);
  }

  // Reused items, in new order, by their old index. The LIS of
  // this sequence is the largest set that can stay put; the
  // rest must move.
  let matched_new: Vec<usize> = (0..new.len())
    .filter(|&np| new_to_old[np].is_some())
    .collect();
  let reused_old: Vec<u32> = matched_new
    .iter()
    .map(|&np| new_to_old[np].unwrap())
    .collect();
  let stayers = longest_increasing_subsequence(&reused_old);

  let mut new_pos_stays = vec![false; new.len()];

  for &matched_idx in &stayers {
    new_pos_stays[matched_new[matched_idx]] = true;
  }

  let mut edits = Vec::new();

  for (old_idx, &matched) in matched_old.iter().enumerate() {
    if !matched {
      edits.push(ListEdit::Remove {
        from: old_idx as u32,
      });
    }
  }

  for (new_pos, source) in new_to_old.iter().enumerate() {
    match source {
      None => edits.push(ListEdit::Insert { to: new_pos as u32 }),
      Some(old_idx) if !new_pos_stays[new_pos] => {
        edits.push(ListEdit::Move {
          from: *old_idx,
          to: new_pos as u32,
        });
      }
      Some(_) => {}
    }
  }

  edits
}

/// Indices (into `seq`) of a longest strictly-increasing
/// subsequence, via O(n log n) patience sorting. `seq`'s values
/// are distinct old indices, so "strictly increasing" is exact.
fn longest_increasing_subsequence(seq: &[u32]) -> Vec<usize> {
  if seq.is_empty() {
    return Vec::new();
  }

  // `tails[k]` = index into `seq` of the smallest tail value of
  // an increasing subsequence of length `k + 1`. `prev` links
  // each element to its predecessor for reconstruction.
  let mut tails: Vec<usize> = Vec::new();
  let mut prev = vec![usize::MAX; seq.len()];

  for i in 0..seq.len() {
    let mut lo = 0;
    let mut hi = tails.len();

    while lo < hi {
      let mid = (lo + hi) / 2;

      if seq[tails[mid]] < seq[i] {
        lo = mid + 1;
      } else {
        hi = mid;
      }
    }

    if lo > 0 {
      prev[i] = tails[lo - 1];
    }

    if lo == tails.len() {
      tails.push(i);
    } else {
      tails[lo] = i;
    }
  }

  let mut result = Vec::with_capacity(tails.len());
  let mut cursor = *tails.last().unwrap();

  loop {
    result.push(cursor);

    if prev[cursor] == usize::MAX {
      break;
    }

    cursor = prev[cursor];
  }

  result.reverse();
  result
}

/// `(cmd_idx, closure_name, capture_map)` triples ready to feed
/// [`apply_computed_bindings`]. The `capture_map` is the same
/// `(param_index, slot_index)` shape click handlers use to
/// resolve a closure's captures against the shared state table.
pub type ResolvedComputedBindings = Vec<(usize, Symbol, Vec<(usize, usize)>)>;

/// Splat each list binding's per-item recipe into the commands
/// buffer. The placeholder `UiCommand::Text(_)` at `cmd_idx` is
/// REPLACED with N rendered items — `Vec::splice` shifts the
/// tail rightward, so any binding past the anchor would need its
/// `cmd_idx` remapped. The current template shapes carry no
/// binding past a list anchor, so a single front-to-back pass is
/// safe; layouts that break that should bake the remap into the
/// executor's index pass.
///
/// Shared by the `aot` and `zo run` paths — the single list
/// applier, so the compiled and interpreted runtimes can't
/// drift.
pub fn apply_list_bindings(
  new_cmds: &mut Vec<UiCommand>,
  list_binds: &[(usize, usize, Vec<ListItemCmd>)],
  cells: &[StateCell],
) {
  let mut offset: isize = 0;

  for (cmd_idx, slot_idx, recipe) in list_binds {
    let target = (*cmd_idx as isize + offset) as usize;

    // Borrow the items under the cell's lock — avoids cloning
    // `Vec<String>` per event just to walk it. `None` means the
    // cell isn't `Strs(_)`, so leave the placeholder alone.
    let Some(rendered) = cells[*slot_idx].with_strs(|items| {
      let mut out: Vec<UiCommand> =
        Vec::with_capacity(items.len() * recipe.len().max(1));

      for item in items {
        for step in recipe {
          match step {
            ListItemCmd::Element { tag, attrs } => {
              out.push(UiCommand::Element {
                tag: tag.clone(),
                attrs: attrs.clone(),
                self_closing: false,
              });
            }
            ListItemCmd::EndElement => out.push(UiCommand::EndElement),
            ListItemCmd::Text(s) => out.push(UiCommand::Text(s.clone())),
            ListItemCmd::TextFromItem => {
              out.push(UiCommand::Text(item.clone()));
            }
          }
        }
      }

      out
    }) else {
      continue;
    };

    let new_len = rendered.len();

    new_cmds.splice(target..target + 1, rendered);
    offset += new_len as isize - 1;
  }
}

/// Re-run each computed binding's closure over the current state
/// cells and stamp the returned value (rendered via `display()`)
/// into its bound `UiCommand::Text` slot. Shared by the
/// per-event patch loop and the initial-render pass — both
/// drive the same evaluator + string snapshot so they can't
/// drift.
pub fn apply_computed_bindings(
  new_cmds: &mut [UiCommand],
  computed_binds: &ResolvedComputedBindings,
  cells: &[StateCell],
  sir: &[Insn],
  strings: &[String],
) {
  for (cmd_idx, closure_name, cap_map) in computed_binds {
    let mut eval = HandlerEvaluator::new();

    let result =
      eval.execute(sir, *closure_name, cells, cap_map, strings, None);

    if let Some(val) = result
      && let Some(UiCommand::Text(s)) = new_cmds.get_mut(*cmd_idx)
    {
      *s = val.display();
    }
  }
}

/// Patch a `Text` command's content to `new` when it differs.
/// Returns whether it changed — so the caller marks the command
/// dirty only on a real change (steady-state UI loops re-fire on
/// unchanged slots, and skipping the no-op keeps the view quiet).
fn apply_text(cmd: &mut UiCommand, new: &str) -> bool {
  match cmd {
    UiCommand::Text(text) if text != new => {
      *text = new.to_string();
      true
    }
    _ => false,
  }
}

/// Patch attribute `name` on an `Element` command to `value` via
/// `UiCommand::set_attr`. Returns whether the attribute's display
/// value actually changed (so an unchanged attr leaves the
/// command byte-identical and unmarked).
fn apply_attr(cmd: &mut UiCommand, name: &str, value: &str) -> bool {
  if attr_value(cmd, name).as_deref() == Some(value) {
    return false;
  }

  cmd.set_attr(name, value);
  true
}

/// The current display string of attribute `name` on `cmd`, or
/// `None` when `cmd` is not an `Element` or has no such attr.
fn attr_value(cmd: &UiCommand, name: &str) -> Option<String> {
  let UiCommand::Element { attrs, .. } = cmd else {
    return None;
  };

  attrs.iter().find(|a| a.name() == name).map(|a| match a {
    Attr::Prop { value, .. } => value.to_display(),
    Attr::Dynamic { initial, .. } => initial.to_display(),
    Attr::Style { value, .. } => value.clone(),
    Attr::Event { .. } => String::new(),
  })
}

/// Fine-grained refresh: walk only the dirtied slots' `Text` /
/// `Attr` targets, patch each command in place from
/// `value(slot)`, and record every command that actually changed
/// in `out` (cleared first). Commands bound to non-dirty slots
/// are never touched — they stay byte-identical.
///
/// `Computed` / `List` targets are skipped here: their new value
/// comes from re-running a closure or a keyed reconcile, both
/// path-specific, so the caller drives them. `attr_names[idx]`
/// names the attribute each `Attr` target patches; `value(slot)`
/// yields the slot's display string (`None` to skip the slot).
pub fn refresh_dirty(
  graph: &BindingGraph,
  dirty: &[u32],
  attr_names: &[String],
  cmds: &mut [UiCommand],
  out: &mut DirtyCommands,
  value: impl Fn(u32) -> Option<String>,
) {
  out.clear();

  for &slot in dirty {
    let Some(text) = value(slot) else {
      continue;
    };

    for target in graph.targets(slot) {
      match *target {
        BindingRef::Text { cmd_idx } => {
          if let Some(cmd) = cmds.get_mut(cmd_idx as usize)
            && apply_text(cmd, &text)
          {
            out.mark(cmd_idx);
          }
        }
        BindingRef::Attr { cmd_idx, attr_idx } => {
          let name = attr_names.get(attr_idx as usize);

          if let Some(cmd) = cmds.get_mut(cmd_idx as usize)
            && let Some(name) = name
            && apply_attr(cmd, name, &text)
          {
            out.mark(cmd_idx);
          }
        }
        BindingRef::Computed { .. } | BindingRef::List { .. } => {}
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn bitset_mark_contains_drain() {
    let mut set = BitSet::with_capacity(200);

    set.mark(3);
    set.mark(64);
    set.mark(199);

    assert!(set.contains(3));
    assert!(set.contains(64));
    assert!(set.contains(199));
    assert!(!set.contains(0));
    assert!(!set.contains(198));

    let mut drained = Vec::new();
    set.drain_into(&mut drained);

    // Ascending order across word boundaries.
    assert_eq!(drained, vec![3, 64, 199]);
    // Drain clears.
    assert!(set.is_empty());
  }

  #[test]
  fn bitset_out_of_range_mark_is_noop() {
    let mut set = BitSet::with_capacity(8);

    set.mark(1000);

    assert!(set.is_empty());
    assert!(!set.contains(1000));
  }

  #[test]
  fn bitset_clear_keeps_capacity() {
    let mut set = BitSet::with_capacity(128);

    set.mark(5);
    set.mark(70);
    let cap = set.capacity();
    set.clear();

    assert!(set.is_empty());
    assert_eq!(set.capacity(), cap);
  }

  #[test]
  fn bitset_ensure_grows_preserving_bits() {
    let mut set = BitSet::with_capacity(8);

    set.mark(3);
    set.ensure(200);

    assert!(set.capacity() >= 200);
    // Grow keeps already-marked bits.
    assert!(set.contains(3));

    set.mark(199);
    assert_eq!(set.to_vec(), vec![3, 199]);
  }

  #[test]
  fn bitset_drain_dedup_idempotent_marks() {
    let mut set = BitSet::with_capacity(64);

    set.mark(10);
    set.mark(10);
    set.mark(10);

    let mut out = Vec::new();
    set.drain_into(&mut out);

    // A set: repeated marks collapse to one index.
    assert_eq!(out, vec![10]);
  }

  #[test]
  fn binding_graph_csr_groups_by_slot() {
    // Slot 0 → cmd 5; slot 2 → cmds 7, 9; slot 1 → none.
    let edges = vec![
      (0u32, BindingRef::Text { cmd_idx: 5 }),
      (2u32, BindingRef::Text { cmd_idx: 7 }),
      (2u32, BindingRef::Text { cmd_idx: 9 }),
    ];
    let graph = BindingGraph::from_edges(3, &edges);

    assert_eq!(graph.num_slots(), 3);
    assert_eq!(graph.targets(0), &[BindingRef::Text { cmd_idx: 5 }]);
    assert_eq!(graph.targets(1), &[]);
    assert_eq!(
      graph.targets(2),
      &[
        BindingRef::Text { cmd_idx: 7 },
        BindingRef::Text { cmd_idx: 9 },
      ]
    );
  }

  #[test]
  fn binding_graph_dirty_commands_exact() {
    let edges = vec![
      (0u32, BindingRef::Text { cmd_idx: 1 }),
      (
        1u32,
        BindingRef::Attr {
          cmd_idx: 4,
          attr_idx: 0,
        },
      ),
      (1u32, BindingRef::Text { cmd_idx: 6 }),
    ];
    let graph = BindingGraph::from_edges(2, &edges);
    let mut out = DirtyCommands::with_capacity(8);

    // Dirtying slot 0 touches only command 1.
    graph.dirty_commands(&[0], &mut out);
    assert_eq!(out.to_vec(), vec![1]);

    // Dirtying slot 1 fans out to commands 4 and 6.
    graph.dirty_commands(&[1], &mut out);
    assert_eq!(out.to_vec(), vec![4, 6]);
  }

  #[test]
  fn binding_graph_shared_slot_fans_out_deduped() {
    // One slot driving two distinct commands, plus a target
    // shared with another slot — the bit-set de-duplicates.
    let edges = vec![
      (0u32, BindingRef::Text { cmd_idx: 2 }),
      (0u32, BindingRef::Text { cmd_idx: 5 }),
      (1u32, BindingRef::Text { cmd_idx: 5 }),
    ];
    let graph = BindingGraph::from_edges(2, &edges);
    let mut out = DirtyCommands::with_capacity(8);

    graph.dirty_commands(&[0, 1], &mut out);

    // 5 appears once despite two edges pointing at it.
    assert_eq!(out.to_vec(), vec![2, 5]);
  }

  #[test]
  fn binding_graph_drops_out_of_range_edges() {
    // Slot 9 is past num_slots — must not panic or grow offsets.
    let edges = vec![
      (0u32, BindingRef::Text { cmd_idx: 1 }),
      (9u32, BindingRef::Text { cmd_idx: 3 }),
    ];
    let graph = BindingGraph::from_edges(2, &edges);

    assert_eq!(graph.targets(0), &[BindingRef::Text { cmd_idx: 1 }]);
    assert_eq!(graph.targets(9), &[]);
  }

  // --- §5 keyed list reconciliation ---

  fn keys(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
  }

  #[test]
  fn reconcile_identical_is_empty() {
    let a = keys(&["a", "b", "c"]);

    assert_eq!(reconcile_list(&a, &a), vec![]);
  }

  #[test]
  fn reconcile_push_is_single_insert() {
    let old = keys(&["a", "b"]);
    let new = keys(&["a", "b", "c"]);

    assert_eq!(reconcile_list(&old, &new), vec![ListEdit::Insert { to: 2 }]);
  }

  #[test]
  fn reconcile_pop_is_single_remove() {
    let old = keys(&["a", "b", "c"]);
    let new = keys(&["a", "b"]);

    assert_eq!(
      reconcile_list(&old, &new),
      vec![ListEdit::Remove { from: 2 }]
    );
  }

  #[test]
  fn reconcile_remove_middle() {
    let old = keys(&["a", "b", "c"]);
    let new = keys(&["a", "c"]);

    assert_eq!(
      reconcile_list(&old, &new),
      vec![ListEdit::Remove { from: 1 }]
    );
  }

  #[test]
  fn reconcile_mutate_one_item_is_remove_plus_insert() {
    // Value-as-key: changing item 1's text changes its key, so
    // the keyed diff drops the old and inserts the new — still
    // O(1) edits regardless of list length (the headline).
    let old = keys(&["a", "b", "c"]);
    let new = keys(&["a", "X", "c"]);

    assert_eq!(
      reconcile_list(&old, &new),
      vec![ListEdit::Remove { from: 1 }, ListEdit::Insert { to: 1 }]
    );
  }

  #[test]
  fn reconcile_reorder_is_single_move() {
    // [a,b,c] → [c,a,b]: a,b keep relative order (the LIS), only
    // c moves to the front — one Move, not three.
    let old = keys(&["a", "b", "c"]);
    let new = keys(&["c", "a", "b"]);

    assert_eq!(
      reconcile_list(&old, &new),
      vec![ListEdit::Move { from: 2, to: 0 }]
    );
  }

  #[test]
  fn reconcile_swap_ends() {
    // [a,b,c,d] → [d,b,c,a]: b,c stay; a and d swap ends.
    let old = keys(&["a", "b", "c", "d"]);
    let new = keys(&["d", "b", "c", "a"]);

    let edits = reconcile_list(&old, &new);

    assert!(edits.contains(&ListEdit::Move { from: 3, to: 0 }));
    assert!(edits.contains(&ListEdit::Move { from: 0, to: 3 }));
    // b, c carry no edit (they're the increasing run that stays).
    assert_eq!(edits.len(), 2);
  }

  #[test]
  fn reconcile_full_reassign_is_minimal() {
    // No shared keys → drop all old, insert all new.
    let old = keys(&["a", "b", "c"]);
    let new = keys(&["x", "y", "z"]);

    assert_eq!(
      reconcile_list(&old, &new),
      vec![
        ListEdit::Remove { from: 0 },
        ListEdit::Remove { from: 1 },
        ListEdit::Remove { from: 2 },
        ListEdit::Insert { to: 0 },
        ListEdit::Insert { to: 1 },
        ListEdit::Insert { to: 2 },
      ]
    );
  }

  #[test]
  fn reconcile_empty_to_filled() {
    let old: Vec<String> = Vec::new();
    let new = keys(&["a", "b"]);

    assert_eq!(
      reconcile_list(&old, &new),
      vec![ListEdit::Insert { to: 0 }, ListEdit::Insert { to: 1 }]
    );
  }

  #[test]
  fn reconcile_filled_to_empty() {
    let old = keys(&["a", "b"]);
    let new: Vec<String> = Vec::new();

    assert_eq!(
      reconcile_list(&old, &new),
      vec![ListEdit::Remove { from: 0 }, ListEdit::Remove { from: 1 }]
    );
  }

  #[test]
  fn reconcile_duplicate_keys_match_positionally() {
    // Two "a"s: removing one drops exactly one old slot, not
    // both (multiset-positional matching).
    let old = keys(&["a", "a", "b"]);
    let new = keys(&["a", "b"]);

    assert_eq!(
      reconcile_list(&old, &new),
      vec![ListEdit::Remove { from: 1 }]
    );
  }

  #[test]
  fn reconcile_mutate_one_item_edits_flat_as_list_grows() {
    // The headline metric: changing one item of an N-item list
    // yields O(1) edits — flat as N grows (10, 100, 1000), so
    // the view layer touches one item region regardless of size.
    for n in [10usize, 100, 1000] {
      let old: Vec<u32> = (0..n as u32).collect();
      let mut new = old.clone();
      new[n / 2] = u32::MAX; // change the middle item's key

      let edits = reconcile_list(&old, &new);

      assert_eq!(
        edits.len(),
        2,
        "N={n}: one mutation is exactly Remove + Insert, not O(N)"
      );
    }
  }

  #[test]
  fn reconcile_push_edits_flat_as_list_grows() {
    // Appending one item is a single Insert regardless of N.
    for n in [10usize, 100, 1000] {
      let old: Vec<u32> = (0..n as u32).collect();
      let mut new = old.clone();
      new.push(n as u32);

      let edits = reconcile_list(&old, &new);

      assert_eq!(edits, vec![ListEdit::Insert { to: n as u32 }]);
    }
  }

  // --- §3 fine-grained refresh ---

  use zo_ui_protocol::{Attr, ElementTag, PropValue};

  #[test]
  fn refresh_dirty_touches_only_dirty_slot_targets() {
    // cmd 0 ← slot 0, cmd 1 ← slot 1. Writing slot 0 must leave
    // cmd 1 byte-identical (the §3 invariant).
    let edges = vec![
      (0u32, BindingRef::Text { cmd_idx: 0 }),
      (1u32, BindingRef::Text { cmd_idx: 1 }),
    ];
    let graph = BindingGraph::from_edges(2, &edges);
    let mut cmds = vec![
      UiCommand::Text("old0".into()),
      UiCommand::Text("old1".into()),
    ];
    let new_values = ["new0", "new1"];
    let mut out = DirtyCommands::with_capacity(2);

    refresh_dirty(&graph, &[0], &[], &mut cmds, &mut out, |slot| {
      Some(new_values[slot as usize].to_string())
    });

    assert_eq!(cmds[0], UiCommand::Text("new0".into()), "slot 0 updated");
    assert_eq!(
      cmds[1],
      UiCommand::Text("old1".into()),
      "slot 1's command is byte-identical"
    );
    assert_eq!(out.to_vec(), vec![0], "only cmd 0 marked dirty");
  }

  #[test]
  fn refresh_dirty_unchanged_value_marks_nothing() {
    let edges = vec![(0u32, BindingRef::Text { cmd_idx: 0 })];
    let graph = BindingGraph::from_edges(1, &edges);
    let mut cmds = vec![UiCommand::Text("same".into())];
    let mut out = DirtyCommands::with_capacity(1);

    refresh_dirty(&graph, &[0], &[], &mut cmds, &mut out, |_| {
      Some("same".to_string())
    });

    assert_eq!(cmds[0], UiCommand::Text("same".into()));
    assert!(out.is_empty(), "no-op write leaves the view quiet");
  }

  #[test]
  fn refresh_dirty_patches_attr_target() {
    let edges = vec![(
      0u32,
      BindingRef::Attr {
        cmd_idx: 0,
        attr_idx: 0,
      },
    )];
    let graph = BindingGraph::from_edges(1, &edges);
    let mut cmds = vec![UiCommand::Element {
      tag: ElementTag::Img,
      attrs: vec![Attr::Dynamic {
        name: "width".into(),
        var: 0,
        initial: PropValue::Num(10),
      }],
      self_closing: true,
    }];
    let attr_names = vec!["width".to_string()];
    let mut out = DirtyCommands::with_capacity(1);

    refresh_dirty(&graph, &[0], &attr_names, &mut cmds, &mut out, |_| {
      Some("128".to_string())
    });

    if let UiCommand::Element { attrs, .. } = &cmds[0] {
      assert_eq!(attrs[0].as_num(), Some(128), "width attr repatched");
    } else {
      panic!("expected Element");
    }

    assert_eq!(out.to_vec(), vec![0]);
  }

  #[test]
  fn refresh_dirty_shared_slot_fans_out() {
    // One slot drives two text commands — both repaint.
    let edges = vec![
      (0u32, BindingRef::Text { cmd_idx: 0 }),
      (0u32, BindingRef::Text { cmd_idx: 2 }),
    ];
    let graph = BindingGraph::from_edges(1, &edges);
    let mut cmds = vec![
      UiCommand::Text("a".into()),
      UiCommand::Text("untouched".into()),
      UiCommand::Text("b".into()),
    ];
    let mut out = DirtyCommands::with_capacity(3);

    refresh_dirty(&graph, &[0], &[], &mut cmds, &mut out, |_| {
      Some("X".to_string())
    });

    assert_eq!(cmds[0], UiCommand::Text("X".into()));
    assert_eq!(cmds[1], UiCommand::Text("untouched".into()));
    assert_eq!(cmds[2], UiCommand::Text("X".into()));
    assert_eq!(out.to_vec(), vec![0, 2]);
  }
}
