//! Shared layout solve: `&[UiCommand]` → `(cmd_idx, Rect)` via Taffy.
//!
//! Every native runtime (egui, UIKit, Android later) calls this so
//! geometry is identical across targets; only the placement of native
//! widgets at the solved rects differs per target. Web keeps emitting
//! HTML/CSS and the browser lays out — Taffy just mirrors that spec.
//!
//! Containers become geometry-only Taffy nodes; only the leaves a
//! runtime actually paints (buttons, free text, images, inputs,
//! text tags) carry a `(cmd_index, NodeId)` entry. Text is measured
//! by a deterministic, font-free closure so the boxes match exactly
//! on every platform (swap for a `cosmic-text` shaper when tight
//! glyph metrics are needed — a separate plan).

use zo_ui_protocol::style::{
  Align, ComputedStyle, Display, FlexDirection, Justify, Size as ZoSize,
  StylePatch, cascade, css,
};
use zo_ui_protocol::{Attr, ElementTag, UiCommand};

use rustc_hash::FxHashMap as HashMap;
use taffy::style_helpers::{length, percent};
use taffy::{
  AlignItems, AvailableSpace, Dimension, Display as TaffyDisplay,
  FlexDirection as TaffyFlexDirection, JustifyContent, LengthPercentage,
  LengthPercentageAuto, NodeId, Rect as TaffyRect, Size as TaffySize,
  Style as TaffyStyle, TaffyTree,
};

/// Default image side when an `<img>` declares no dimensions, matching
/// the egui renderer's fallback.
const IMAGE_FALLBACK: f32 = 256.0;

/// Default width given to a text `<input>`/`<textarea>` leaf; its
/// height falls out of the line measure.
const INPUT_WIDTH: f32 = 180.0;

/// Centre-cluster gap between the synthetic root's children.
const ROOT_GAP: f32 = 8.0;

/// Solved box for one command, in the root's coordinate space.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
  pub x: f32,
  pub y: f32,
  pub width: f32,
  pub height: f32,
}

/// Leaf payload Taffy hands back to the measure closure: the text to
/// size and the style that governs its font + padding.
struct Leaf {
  text: String,
  style: ComputedStyle,
}

/// A built Taffy tree plus the command-index ↔ node mapping. Built
/// once per command stream and kept alive: `solve` re-runs whenever
/// the viewport changes, and `reconcile` folds a new command stream
/// into the existing tree so a retained-mode runtime (UIKit, …)
/// updates only what changed instead of rebuilding every widget.
pub struct LayoutTree {
  tree: TaffyTree<Leaf>,
  root: NodeId,
  /// `nodes[i]` is the Taffy node for `commands[cmd_index[i]]`. Only
  /// placed leaves (Element-leaf / free Text) get an entry; the four
  /// side tables below stay parallel to it.
  cmd_index: Vec<usize>,
  nodes: Vec<NodeId>,
  /// Resolved style per placed leaf — kept so `reconcile` can rebuild
  /// a leaf's measure context when its text changes.
  styles: Vec<ComputedStyle>,
  /// The author patch per placed leaf (`EMPTY` when no rule targets
  /// it). Its `Some` fields tell a runtime which declared properties
  /// to paint over a native widget's defaults.
  authors: Vec<StylePatch>,
  /// Last text per placed leaf, diffed against the next stream.
  texts: Vec<String>,
  /// The command stream this tree was built from. `reconcile`
  /// compares a new stream against it to decide fast-path vs rebuild.
  source: Vec<UiCommand>,
  /// The stylesheet image catalog: `images[id]` is the URL for a
  /// `background_image` handle on any resolved `ComputedStyle`.
  images: Vec<String>,
  /// The resolved `body` style — the root container's backdrop
  /// (`background-image` / `background`) when the program declares one.
  root_style: ComputedStyle,
}

impl LayoutTree {
  /// Build the tree. Top-level siblings (fragment children) hang off a
  /// synthetic root flex container sized to the viewport in `solve`.
  pub fn build(commands: &[UiCommand]) -> Self {
    let (author, images) = collect_author(commands);

    // The container backdrop comes from the `body` rule (image or
    // colour); resolve it before `author` moves into the builder.
    let body_style = cascade::resolve(
      "body",
      css::author_patch(&author, "body").as_ref(),
      None,
    );

    let mut builder = Builder::new(author);
    let children = builder.children(commands);

    // Synthetic root: a flex container whose direction follows the
    // inline-vs-block nature of its children (ports the desktop
    // `children_are_inline_flow` heuristic at depth 0). The cluster is
    // centred so a fragment counter sits in the middle of the window.
    let direction = if children_are_inline_flow(commands, 0) {
      TaffyFlexDirection::Row
    } else {
      TaffyFlexDirection::Column
    };

    let root_style = TaffyStyle {
      display: TaffyDisplay::Flex,
      flex_direction: direction,
      justify_content: Some(JustifyContent::Center),
      align_items: Some(AlignItems::Center),
      gap: length(ROOT_GAP),
      size: percent(1.0_f32),
      ..Default::default()
    };

    let root = builder
      .tree
      .new_with_children(root_style, &children)
      .expect("taffy root");

    Self {
      tree: builder.tree,
      root,
      cmd_index: builder.cmd_index,
      nodes: builder.nodes,
      styles: builder.styles,
      authors: builder.authors,
      texts: builder.texts,
      source: commands.to_vec(),
      images,
      root_style: body_style,
    }
  }

  /// Fold a new command stream into this tree without rebuilding it.
  ///
  /// When the new stream is structurally identical (same elements in
  /// the same order — only text/attribute content differs), this is
  /// the fast path: each changed leaf's measure context is swapped in
  /// place (which dirties just that node), and the changed
  /// `(placement index, new text)` pairs are returned so the runtime
  /// repaints only those widgets. A following `solve` re-lays out
  /// incrementally — Taffy reuses every clean subtree.
  ///
  /// Returns `None` when the stream changed structurally (items
  /// added/removed); the caller then rebuilds.
  pub fn reconcile(
    &mut self,
    commands: &[UiCommand],
  ) -> Option<Vec<(usize, String)>> {
    if !structurally_equal(&self.source, commands) {
      return None;
    }

    let mut changed = Vec::new();

    for i in 0..self.cmd_index.len() {
      let text = leaf_text(commands, self.cmd_index[i]);

      if text != self.texts[i] {
        self.texts[i] = text.clone();

        let leaf = Leaf {
          text: text.clone(),
          style: self.styles[i],
        };

        self
          .tree
          .set_node_context(self.nodes[i], Some(leaf))
          .expect("taffy set context");

        changed.push((i, text));
      }
    }

    self.source = commands.to_vec();

    Some(changed)
  }

  /// The resolved style for each placed leaf, parallel to `solve`'s
  /// returned order. Runtimes read it to paint native widgets (font,
  /// colour) so the on-screen text matches the measured geometry.
  pub fn styles(&self) -> &[ComputedStyle] {
    &self.styles
  }

  /// The author patch for each placed leaf, parallel to `solve`'s
  /// returned order. A runtime paints a declared colour only where
  /// the patch's field is `Some`, leaving native defaults otherwise.
  pub fn authors(&self) -> &[StylePatch] {
    &self.authors
  }

  /// The stylesheet image catalog. A `ComputedStyle`'s
  /// `background_image` is an index into this slice.
  pub fn images(&self) -> &[String] {
    &self.images
  }

  /// The resolved `body` style — the root container's backdrop. A
  /// runtime paints the container from its `background_image` /
  /// `background` instead of a hardcoded colour.
  pub fn root_style(&self) -> ComputedStyle {
    self.root_style
  }

  /// Solve against the viewport and return one absolute `Rect` per
  /// placed leaf. Taffy reports each node's position relative to its
  /// parent, so a post-solve walk accumulates ancestor offsets into
  /// the root's coordinate space.
  pub fn solve(&mut self, available: (f32, f32)) -> Vec<(usize, Rect)> {
    let space = TaffySize {
      width: AvailableSpace::Definite(available.0),
      height: AvailableSpace::Definite(available.1),
    };

    self
      .tree
      .compute_layout_with_measure(self.root, space, measure_leaf)
      .expect("taffy solve");

    let mut absolute: HashMap<NodeId, Rect> = HashMap::default();
    self.collect_absolute(self.root, (0.0, 0.0), &mut absolute);

    self
      .cmd_index
      .iter()
      .zip(&self.nodes)
      .map(|(&idx, node)| (idx, absolute[node]))
      .collect()
  }

  /// Walk the solved tree, folding each node's parent-relative
  /// location into an absolute rect.
  fn collect_absolute(
    &self,
    node: NodeId,
    origin: (f32, f32),
    out: &mut HashMap<NodeId, Rect>,
  ) {
    let layout = self.tree.layout(node).expect("taffy layout");
    let x = origin.0 + layout.location.x;
    let y = origin.1 + layout.location.y;

    out.insert(
      node,
      Rect {
        x,
        y,
        width: layout.size.width,
        height: layout.size.height,
      },
    );

    for child in self.tree.children(node).unwrap_or_default() {
      self.collect_absolute(child, (x, y), out);
    }
  }
}

/// Accumulates the placed-leaf side tables while walking the command
/// stream into a Taffy tree. Containers become geometry-only nodes;
/// every leaf (and free text) gets a row in `cmd_index` / `nodes` /
/// `styles` / `texts`. Keeping the stream in a method parameter (not a
/// field) lets each arm borrow it immutably while the walk mutates the
/// builder.
struct Builder {
  tree: TaffyTree<Leaf>,
  /// Author rules parsed from the stream's stylesheets, scanned per
  /// element to resolve its style and record what it declared.
  author: Vec<(String, StylePatch)>,
  cursor: usize,
  cmd_index: Vec<usize>,
  nodes: Vec<NodeId>,
  styles: Vec<ComputedStyle>,
  authors: Vec<StylePatch>,
  texts: Vec<String>,
}

impl Builder {
  fn new(author: Vec<(String, StylePatch)>) -> Self {
    Self {
      tree: TaffyTree::new(),
      author,
      cursor: 0,
      cmd_index: Vec::new(),
      nodes: Vec::new(),
      styles: Vec::new(),
      authors: Vec::new(),
      texts: Vec::new(),
    }
  }

  /// Walk one container's children up to its `EndElement`, returning
  /// the child node ids so the parent can attach them. Buttons and
  /// text-tags collapse their text children into one leaf (Taffy has
  /// no inline-formatting context).
  fn children(&mut self, cmds: &[UiCommand]) -> Vec<NodeId> {
    let mut children = Vec::new();

    while self.cursor < cmds.len() {
      match &cmds[self.cursor] {
        UiCommand::EndElement => {
          self.cursor += 1;
          return children;
        }

        // Events + stylesheets carry no geometry.
        UiCommand::Event { .. } | UiCommand::StyleSheet { .. } => {
          self.cursor += 1;
        }

        // Free-standing text (`{count}`) → a measured leaf with root
        // typography (no tag to key the cascade on, no author patch).
        UiCommand::Text(text) => {
          let idx = self.cursor;
          let text = text.clone();
          let node =
            self.leaf(idx, text, ComputedStyle::ROOT, StylePatch::EMPTY, None);

          children.push(node);
          self.cursor += 1;
        }

        UiCommand::Element {
          tag,
          attrs,
          self_closing,
        } => {
          let idx = self.cursor;
          let self_closing = *self_closing;
          let author = css::author_patch(&self.author, tag.as_str());
          let style = cascade::resolve(tag.as_str(), author.as_ref(), None);
          let leaf = is_leaf_tag(tag);
          let size = leaf_size_override(tag, attrs);

          self.cursor += 1;

          if leaf {
            let text = if self_closing {
              String::new()
            } else {
              collapse_text(cmds, self.cursor)
            };
            let author = author.unwrap_or(StylePatch::EMPTY);
            let node = self.leaf(idx, text, style, author, size);

            children.push(node);

            if !self_closing {
              skip_to_end(cmds, &mut self.cursor);
            }
          } else {
            // Container: a geometry-only node. Its main axis follows a
            // declared `display: flex` direction, else the inline-vs-
            // block flow of its children.
            let direction = container_direction(&style, cmds, self.cursor);
            let kids = if self_closing {
              Vec::new()
            } else {
              self.children(cmds)
            };
            let node = self
              .tree
              .new_with_children(to_taffy(&style, direction), &kids)
              .expect("taffy container");

            children.push(node);
          }
        }
      }
    }

    children
  }

  /// Create a measured leaf node, recording it in the side tables.
  /// `size` pins an explicit box (images, inputs); otherwise the box
  /// is `auto` and the measure closure sizes it from the text.
  fn leaf(
    &mut self,
    idx: usize,
    text: String,
    style: ComputedStyle,
    author: StylePatch,
    size: Option<TaffySize<Dimension>>,
  ) -> NodeId {
    let taffy_style = TaffyStyle {
      // Padding is folded into the text measure, so the leaf's own box
      // padding stays zero — setting both would double-count it.
      margin: edges_to_margin(&style),
      size: size.unwrap_or(TaffySize {
        width: Dimension::auto(),
        height: Dimension::auto(),
      }),
      ..Default::default()
    };

    let node = self
      .tree
      .new_leaf_with_context(
        taffy_style,
        Leaf {
          text: text.clone(),
          style,
        },
      )
      .expect("taffy leaf");

    self.cmd_index.push(idx);
    self.nodes.push(node);
    self.styles.push(style);
    self.authors.push(author);
    self.texts.push(text);

    node
  }
}

/// Parse every `StyleSheet` command into one ordered list of author
/// rules plus the combined image catalog the cascade folds in. Each
/// sheet's `background_image` handles are offset into the combined
/// catalog so indices stay valid when several sheets are concatenated.
fn collect_author(
  commands: &[UiCommand],
) -> (Vec<(String, StylePatch)>, Vec<String>) {
  let mut rules = Vec::new();
  let mut images = Vec::new();

  for cmd in commands {
    if let UiCommand::StyleSheet { css, .. } = cmd {
      let mut sheet = css::parse(css);
      let base = images.len() as u32;

      if base > 0 {
        for (_, patch) in &mut sheet.rules {
          if let Some(id) = patch.background_image.as_mut() {
            *id += base;
          }
        }
      }

      rules.extend(sheet.rules);
      images.extend(sheet.images);
    }
  }

  (rules, images)
}

/// True when two command streams place the same widgets in the same
/// order — only text / attribute content may differ. The fast-path
/// gate for `reconcile`; a `false` means structure changed (items
/// added or removed) and the tree must be rebuilt.
fn structurally_equal(a: &[UiCommand], b: &[UiCommand]) -> bool {
  a.len() == b.len() && a.iter().zip(b).all(|(x, y)| same_shape(x, y))
}

fn same_shape(x: &UiCommand, y: &UiCommand) -> bool {
  match (x, y) {
    (
      UiCommand::Element {
        tag: a,
        self_closing: sa,
        ..
      },
      UiCommand::Element {
        tag: b,
        self_closing: sb,
        ..
      },
    ) => a == b && sa == sb,
    (UiCommand::EndElement, UiCommand::EndElement) => true,
    (UiCommand::Text(_), UiCommand::Text(_)) => true,
    (UiCommand::Event { .. }, UiCommand::Event { .. }) => true,
    (UiCommand::StyleSheet { .. }, UiCommand::StyleSheet { .. }) => true,
    _ => false,
  }
}

/// The text a placed leaf renders: a free `Text` node's own content,
/// or an element leaf's collapsed children (button / text-tag label).
fn leaf_text(cmds: &[UiCommand], idx: usize) -> String {
  match &cmds[idx] {
    UiCommand::Text(text) => text.clone(),
    UiCommand::Element {
      self_closing: false,
      ..
    } => collapse_text(cmds, idx + 1),
    _ => String::new(),
  }
}

/// `ComputedStyle` → `taffy::Style` for a container: display, the
/// resolved main axis, distribution, size, gap, padding, margin. The
/// single conversion point.
fn to_taffy(
  style: &ComputedStyle,
  direction: TaffyFlexDirection,
) -> TaffyStyle {
  TaffyStyle {
    display: match style.display {
      Display::None => TaffyDisplay::None,
      _ => TaffyDisplay::Flex,
    },
    flex_direction: direction,
    justify_content: Some(to_justify(style.justify_content)),
    align_items: Some(to_align(style.align_items)),
    gap: length(style.gap),
    size: to_size(style.width, style.height),
    padding: edges_to_padding(style),
    margin: edges_to_margin(style),
    ..Default::default()
  }
}

/// A container's main axis: an explicit `display: flex` honours its
/// declared direction; a block/inline container flows `Row` when all
/// its children are inline-level, else stacks in a `Column`.
fn container_direction(
  style: &ComputedStyle,
  cmds: &[UiCommand],
  children_start: usize,
) -> TaffyFlexDirection {
  if matches!(style.display, Display::Flex) {
    return match style.flex_direction {
      FlexDirection::Row => TaffyFlexDirection::Row,
      FlexDirection::Column => TaffyFlexDirection::Column,
    };
  }

  if children_are_inline_flow(cmds, children_start) {
    TaffyFlexDirection::Row
  } else {
    TaffyFlexDirection::Column
  }
}

/// Pin an explicit box for the leaves the text measure cannot size:
/// images take their declared (or fallback) dimensions, text inputs a
/// sensible default width.
fn leaf_size_override(
  tag: &ElementTag,
  attrs: &[Attr],
) -> Option<TaffySize<Dimension>> {
  match tag {
    ElementTag::Img => {
      let width = attr_num(attrs, "width").unwrap_or(IMAGE_FALLBACK as u32);
      let height = attr_num(attrs, "height").unwrap_or(IMAGE_FALLBACK as u32);

      Some(TaffySize {
        width: Dimension::length(width as f32),
        height: Dimension::length(height as f32),
      })
    }
    ElementTag::Input | ElementTag::Textarea => Some(TaffySize {
      width: Dimension::length(INPUT_WIDTH),
      height: Dimension::auto(),
    }),
    _ => None,
  }
}

/// Leaves a runtime paints directly: text tags (h1–h6, p, span), the
/// button, and the self-contained media/input widgets. Everything else
/// is a container whose children are placed on their own.
fn is_leaf_tag(tag: &ElementTag) -> bool {
  tag.is_text_tag()
    || matches!(
      tag,
      ElementTag::Button
        | ElementTag::Img
        | ElementTag::Input
        | ElementTag::Textarea
    )
}

/// Return `true` when every direct child between `start` and the
/// matching `EndElement` is inline-level (span, button, input,
/// textarea, img) or non-blank raw text. Empty containers return
/// `false`. The row-vs-column heuristic ported from the egui renderer.
fn children_are_inline_flow(cmds: &[UiCommand], start: usize) -> bool {
  let mut depth: usize = 0;
  let mut idx = start;
  let mut saw_any = false;

  while idx < cmds.len() {
    match &cmds[idx] {
      UiCommand::Element {
        tag, self_closing, ..
      } => {
        if depth == 0 {
          saw_any = true;

          if !is_inline_flow_tag(tag) {
            return false;
          }
        }

        if !self_closing {
          depth += 1;
        }
      }
      UiCommand::EndElement => {
        if depth == 0 {
          return saw_any;
        }

        depth -= 1;
      }
      UiCommand::Text(text) if depth == 0 && !text.trim().is_empty() => {
        saw_any = true;
      }
      _ => {}
    }

    idx += 1;
  }

  saw_any
}

/// Tags treated as inline flow at the parent-layout level — CSS
/// `display: inline | inline-block` for the subset zo models.
fn is_inline_flow_tag(tag: &ElementTag) -> bool {
  matches!(
    tag,
    ElementTag::Span
      | ElementTag::Button
      | ElementTag::Input
      | ElementTag::Textarea
      | ElementTag::Img
  )
}

/// Concatenate the direct `Text` children starting at `start`, up to
/// (but not including) the matching `EndElement`. Nested elements are
/// skipped — only direct text contributes to a collapsed leaf.
///
/// Public so the runtimes can recover a placed leaf's label from its
/// command index without re-deriving the walk (one collapse rule).
pub fn collapse_text(cmds: &[UiCommand], start: usize) -> String {
  let mut out = String::new();
  let mut depth: usize = 0;
  let mut idx = start;

  while idx < cmds.len() {
    match &cmds[idx] {
      UiCommand::Element { self_closing, .. } if !self_closing => {
        depth += 1;
      }
      UiCommand::EndElement => {
        if depth == 0 {
          break;
        }

        depth -= 1;
      }
      UiCommand::Text(text) => out.push_str(text),
      _ => {}
    }

    idx += 1;
  }

  out
}

/// Advance `cursor` past the matching `EndElement` for the element
/// whose children begin at `*cursor`.
fn skip_to_end(cmds: &[UiCommand], cursor: &mut usize) {
  let mut depth: usize = 0;

  while *cursor < cmds.len() {
    match &cmds[*cursor] {
      UiCommand::Element { self_closing, .. } if !self_closing => {
        depth += 1;
      }
      UiCommand::EndElement => {
        if depth == 0 {
          *cursor += 1;
          return;
        }

        depth -= 1;
      }
      _ => {}
    }

    *cursor += 1;
  }
}

/// Look up the numeric value of a named attribute.
fn attr_num(attrs: &[Attr], name: &str) -> Option<u32> {
  attrs.iter().find(|a| a.name() == name).and_then(|a| {
    a.as_num()
      .or_else(|| a.as_str().and_then(|s| s.parse().ok()))
  })
}

/// `zo::Size` width/height → `taffy::Size<Dimension>`.
fn to_size(width: ZoSize, height: ZoSize) -> TaffySize<Dimension> {
  TaffySize {
    width: to_dimension(width),
    height: to_dimension(height),
  }
}

fn to_dimension(size: ZoSize) -> Dimension {
  match size {
    ZoSize::Auto => Dimension::auto(),
    ZoSize::Px(value) => Dimension::length(value),
    // zo stores percent as 0–100; taffy wants a 0–1 fraction.
    ZoSize::Percent(value) => Dimension::percent(value / 100.0),
  }
}

fn edges_to_padding(style: &ComputedStyle) -> TaffyRect<LengthPercentage> {
  let edges = style.padding;

  TaffyRect {
    left: LengthPercentage::length(edges.left),
    right: LengthPercentage::length(edges.right),
    top: LengthPercentage::length(edges.top),
    bottom: LengthPercentage::length(edges.bottom),
  }
}

fn edges_to_margin(style: &ComputedStyle) -> TaffyRect<LengthPercentageAuto> {
  let edges = style.margin;

  TaffyRect {
    left: LengthPercentageAuto::length(edges.left),
    right: LengthPercentageAuto::length(edges.right),
    top: LengthPercentageAuto::length(edges.top),
    bottom: LengthPercentageAuto::length(edges.bottom),
  }
}

fn to_justify(justify: Justify) -> JustifyContent {
  match justify {
    Justify::Start => JustifyContent::Start,
    Justify::Center => JustifyContent::Center,
    Justify::End => JustifyContent::End,
    Justify::SpaceBetween => JustifyContent::SpaceBetween,
  }
}

fn to_align(align: Align) -> AlignItems {
  match align {
    Align::Stretch => AlignItems::Stretch,
    Align::Start => AlignItems::Start,
    Align::Center => AlignItems::Center,
    Align::End => AlignItems::End,
  }
}

/// Deterministic, font-free leaf measure: width ≈ glyphs × 0.6em,
/// height ≈ line, with the leaf's padding folded in. Identical on
/// every platform, so the solved boxes match exactly. Swap for a
/// `cosmic-text` shaper when tight text metrics are needed.
fn measure_leaf(
  known: TaffySize<Option<f32>>,
  _available: TaffySize<AvailableSpace>,
  _node: NodeId,
  leaf: Option<&mut Leaf>,
  _style: &TaffyStyle,
) -> TaffySize<f32> {
  if let (Some(width), Some(height)) = (known.width, known.height) {
    return TaffySize { width, height };
  }

  let Some(leaf) = leaf else {
    return TaffySize::ZERO;
  };

  let font_size = leaf.style.font_size;
  let padding = leaf.style.padding;
  let glyph_width = font_size * 0.6;
  let width = leaf.text.chars().count() as f32 * glyph_width
    + padding.left
    + padding.right;
  let height =
    font_size * leaf.style.line_height + padding.top + padding.bottom;

  TaffySize {
    width: known.width.unwrap_or(width),
    height: known.height.unwrap_or(height),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn button(id: &str) -> UiCommand {
    UiCommand::Element {
      tag: ElementTag::Button,
      attrs: vec![Attr::parse_prop("data-id", id)],
      self_closing: false,
    }
  }

  fn text(content: &str) -> UiCommand {
    UiCommand::Text(content.to_string())
  }

  fn element(tag: ElementTag) -> UiCommand {
    UiCommand::Element {
      tag,
      attrs: Vec::new(),
      self_closing: false,
    }
  }

  /// `<><button>-</button>{count}<button>+</button></>` — the counter
  /// fragment. The milestone invariant: `- count +` on one row.
  fn counter_with(count: &str) -> Vec<UiCommand> {
    vec![
      button("0"),
      text("-"),
      UiCommand::EndElement,
      text(count),
      button("1"),
      text("+"),
      UiCommand::EndElement,
    ]
  }

  fn counter() -> Vec<UiCommand> {
    counter_with("0")
  }

  #[test]
  fn counter_lays_out_on_one_row() {
    let cmds = counter();
    let mut tree = LayoutTree::build(&cmds);
    let rects = tree.solve((320.0, 480.0));

    assert_eq!(rects.len(), 3, "minus, count, plus are placed");

    // Returned in source order: button(-), text(0), button(+).
    let xs: Vec<f32> = rects.iter().map(|(_, r)| r.x).collect();

    assert!(
      xs[0] < xs[1] && xs[1] < xs[2],
      "ascending x across the row: {xs:?}"
    );

    // Same row: every pair's vertical interval overlaps.
    for pair in rects.windows(2) {
      let a = pair[0].1;
      let b = pair[1].1;
      let overlaps = a.y < b.y + b.height && b.y < a.y + a.height;

      assert!(overlaps, "on one row: {a:?} vs {b:?}");
    }

    // The buttons carry UA padding, so each is wider than the bare
    // single-glyph count between them.
    let count_width = rects[1].1.width;

    assert!(
      rects[0].1.width > count_width && rects[2].1.width > count_width,
      "padded buttons are wider than the bare count"
    );
  }

  #[test]
  fn counter_cluster_is_centred() {
    let cmds = counter();
    let mut tree = LayoutTree::build(&cmds);
    let rects = tree.solve((320.0, 480.0));

    let left = rects[0].1.x;
    let right = rects[2].1.x + rects[2].1.width;
    let mid = (left + right) / 2.0;

    // Cluster centred horizontally in the 320-wide viewport.
    assert!((mid - 160.0).abs() < 1.0, "cluster centred: mid={mid}");
  }

  #[test]
  fn block_children_stack_vertically() {
    let cmds = vec![
      element(ElementTag::Div),
      element(ElementTag::P),
      text("a"),
      UiCommand::EndElement,
      element(ElementTag::P),
      text("b"),
      UiCommand::EndElement,
      UiCommand::EndElement,
    ];

    let mut tree = LayoutTree::build(&cmds);
    let rects = tree.solve((320.0, 480.0));

    assert_eq!(rects.len(), 2, "two paragraphs are placed");

    let top = rects[0].1;
    let bottom = rects[1].1;

    assert!(
      bottom.y >= top.y + top.height,
      "paragraphs stack: {top:?} {bottom:?}"
    );
  }

  #[test]
  fn reconcile_text_change_is_fast_path() {
    let mut tree = LayoutTree::build(&counter());
    let before = tree.solve((320.0, 480.0));
    let count_before = before[1].1.width;

    // Count grows `0` → `10`: same structure, one changed leaf.
    let changed = tree
      .reconcile(&counter_with("10"))
      .expect("text-only change keeps structure");

    assert_eq!(changed.len(), 1, "only the count leaf changed");
    assert_eq!(changed[0].0, 1, "the middle placement");
    assert_eq!(changed[0].1, "10");

    // Re-solve widens the count box and keeps three on a row.
    let after = tree.solve((320.0, 480.0));

    assert_eq!(after.len(), 3);
    assert!(
      after[1].1.width > count_before,
      "the count box grows for the wider text: {} -> {}",
      count_before,
      after[1].1.width
    );

    let xs: Vec<f32> = after.iter().map(|(_, r)| r.x).collect();
    assert!(xs[0] < xs[1] && xs[1] < xs[2], "still a row: {xs:?}");
  }

  #[test]
  fn reconcile_structural_change_bails() {
    let mut tree = LayoutTree::build(&counter());

    // An extra button is a structural change — no fast path.
    let mut grown = counter();
    grown.push(button("2"));
    grown.push(text("="));
    grown.push(UiCommand::EndElement);

    assert!(
      tree.reconcile(&grown).is_none(),
      "adding a widget forces a rebuild"
    );
  }

  #[test]
  fn reconcile_no_change_reports_nothing() {
    let mut tree = LayoutTree::build(&counter());

    let changed = tree.reconcile(&counter()).expect("same structure");

    assert!(changed.is_empty(), "identical stream changes nothing");
  }
}
