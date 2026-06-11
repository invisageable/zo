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

use crate::reactive::DirtyCommands;

use zo_ui_protocol::style::{
  Align, ComputedStyle, Display, FlexDirection, FlexWrap, Justify, Material,
  Size, StylePatch, cascade, css,
};
use zo_ui_protocol::{Attr, ElementTag, UiCommand};

use rustc_hash::FxHashMap as HashMap;
use taffy::style_helpers::{length, percent};
use taffy::{
  AlignItems, AvailableSpace, Dimension, Display as TaffyDisplay,
  FlexDirection as TaffyFlexDirection, FlexWrap as TaffyFlexWrap,
  JustifyContent, LengthPercentage, LengthPercentageAuto, NodeId,
  Rect as TaffyRect, Size as TaffySize, Style as TaffyStyle, TaffyTree,
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
  /// Interaction-state patches per placement, parallel to `authors`.
  /// The renderer overlays the patch matching the element's current
  /// state (hover/active/focus/disabled) at paint time.
  interactions: Vec<InteractionAuthors>,
  /// The enclosing paintable container per placed leaf, as a placement
  /// index into this same parallel order (`None` for a leaf that sits
  /// directly on the root). A retained-mode runtime that nests glass
  /// (UIKit: a child must live in the effect view's `contentView` so
  /// the glass composites it) reads this to reparent; a flat runtime
  /// (egui) ignores it.
  parents: Vec<Option<usize>>,
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
    let children = builder.children(commands, None);

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
      interactions: builder.interactions,
      parents: builder.parents,
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

  /// Patch only the placements whose source commands are dirty —
  /// the fine-grained half of [`Self::reconcile`]. The caller
  /// guarantees the dirty refresh rewrote text/attrs in place (no
  /// structural edits), so neither the structural scan nor the
  /// full-stream clone happens here: cost is O(dirty), not O(N).
  /// Returns the touched placements as `(placement, new_text)`.
  pub fn apply_dirty(
    &mut self,
    dirty: &DirtyCommands,
    commands: &[UiCommand],
  ) -> Vec<(usize, String)> {
    let mut changed = Vec::new();

    dirty.for_each_set(|cmd| {
      let cmd = cmd as usize;

      // Map the dirty command to its placement: an exact
      // `cmd_index` hit is a free-text leaf; otherwise the dirty
      // index is a text/attr command inside the preceding
      // placement's region. `cmd_index` is ascending (built in
      // walk order), so partition_point finds it.
      let at = self.cmd_index.partition_point(|&idx| idx <= cmd);

      if at == 0 {
        return;
      }

      let placement = at - 1;
      let source_idx = self.cmd_index[placement];
      let text = leaf_text(commands, source_idx);

      if text != self.texts[placement] {
        self.texts[placement] = text.clone();

        let leaf = Leaf {
          text: text.clone(),
          style: self.styles[placement],
        };

        self
          .tree
          .set_node_context(self.nodes[placement], Some(leaf))
          .expect("taffy set context");

        changed.push((placement, text));
      } else if matches!(commands.get(cmd), Some(UiCommand::Element { .. }))
        && changed.last().map(|(p, _)| *p) != Some(placement)
      {
        // An attr-only patch (`value`, `class`) moves no text but
        // the view must still repaint the widget. Geometry is
        // text-driven, so no Taffy re-measure.
        changed.push((placement, text));
      }

      // Keep `source` in sync entry-wise — the next structural
      // comparison must see the patched stream without paying a
      // full-stream clone per event.
      if cmd < self.source.len() {
        self.source[cmd] = commands[cmd].clone();
      }
    });

    changed
  }

  /// Append a list's tail items without rebuilding the tree — the
  /// push fast path. Each new item clones the ANCHOR item's shape
  /// (the last existing item): its container node's Taffy style
  /// and its text leaf's resolved style rows — recipe items are
  /// homogeneous, so the clone is exact. Placements after the
  /// splice shift their command indices; `source` splices the new
  /// commands in. Returns the new placement indices, or `None`
  /// when the shape disqualifies the fast path (no existing item
  /// to anchor on, or an item too deep to mirror) — the caller
  /// falls back to a full rebuild.
  pub fn append_list_items(
    &mut self,
    at: usize,
    added: usize,
    commands: &[UiCommand],
  ) -> Option<std::ops::Range<usize>> {
    // The anchor: the placement of the last existing item's text
    // leaf — the one whose command sits immediately before the
    // splice. Its parent node is the item container (`<li>`),
    // whose parent is the list container (`<ul>`).
    let anchor = (0..self.cmd_index.len())
      .rev()
      .find(|&i| self.cmd_index[i] < at)?;

    let anchor_item = self.tree.parent(self.nodes[anchor])?;
    let list_node = self.tree.parent(anchor_item)?;
    let item_style = self.tree.style(anchor_item).ok()?.clone();

    // Shift downstream placements past the splice point.
    for idx in self.cmd_index.iter_mut() {
      if *idx >= at {
        *idx += added;
      }
    }

    let first_new = self.cmd_index.len();
    let mut cursor = at;

    while cursor < at + added {
      // Each item group: a container Element, its text content,
      // its EndElement. Anything deeper than the anchor's
      // two-node shape bails to the rebuild.
      let UiCommand::Element { tag, .. } = &commands[cursor] else {
        return None;
      };

      if is_leaf_tag(tag) {
        return None;
      }

      // The text content up to the matching EndElement.
      let text = collapse_text(commands, cursor + 1);
      let text_idx = cursor + 1;

      let mut depth = 1usize;

      cursor += 1;

      while cursor < at + added && depth > 0 {
        match &commands[cursor] {
          UiCommand::Element {
            self_closing: false,
            ..
          } => return None,
          UiCommand::EndElement => depth -= 1,
          _ => {}
        }

        cursor += 1;
      }

      let style = self.styles[anchor];
      let leaf = Leaf {
        text: text.clone(),
        style,
      };

      let taffy_leaf_style = TaffyStyle {
        margin: edges_to_margin(&style),
        size: TaffySize {
          width: Dimension::auto(),
          height: Dimension::auto(),
        },
        ..Default::default()
      };

      let leaf_node = self
        .tree
        .new_leaf_with_context(taffy_leaf_style, leaf)
        .expect("taffy leaf");
      let item_node = self
        .tree
        .new_with_children(item_style.clone(), &[leaf_node])
        .expect("taffy item");

      self
        .tree
        .add_child(list_node, item_node)
        .expect("taffy child");
      self.cmd_index.push(text_idx);
      self.nodes.push(leaf_node);
      self.styles.push(style);
      self.authors.push(self.authors[anchor]);
      self.interactions.push(self.interactions[anchor].clone());
      self.parents.push(self.parents[anchor]);
      self.texts.push(text);
    }

    self
      .source
      .splice(at..at, commands[at..at + added].iter().cloned());

    Some(first_new..self.cmd_index.len())
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

  /// The interaction-state patches for each placement, parallel to
  /// `authors`. Empty entries mean no state rule targets the element
  /// — the renderer skips state tracking for those.
  pub fn interactions(&self) -> &[InteractionAuthors] {
    &self.interactions
  }

  /// The enclosing paintable container for each placed leaf, as a
  /// placement index into the same parallel order `solve` returns
  /// (`None` for a leaf placed directly on the root). A runtime that
  /// nests glass reparents a leaf into its parent's surface; a flat
  /// runtime leaves every leaf on the root container.
  pub fn parents(&self) -> &[Option<usize>] {
    &self.parents
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
  author: Vec<css::CssRule>,
  cursor: usize,
  cmd_index: Vec<usize>,
  nodes: Vec<NodeId>,
  styles: Vec<ComputedStyle>,
  authors: Vec<StylePatch>,
  /// Interaction-state patches per placement — see
  /// `LayoutTree::interactions`.
  interactions: Vec<InteractionAuthors>,
  /// The enclosing paintable container per placement, parallel to
  /// `cmd_index` — see `LayoutTree::parents`.
  parents: Vec<Option<usize>>,
  texts: Vec<String>,
}

impl Builder {
  fn new(author: Vec<css::CssRule>) -> Self {
    Self {
      tree: TaffyTree::new(),
      author,
      cursor: 0,
      cmd_index: Vec::new(),
      nodes: Vec::new(),
      styles: Vec::new(),
      authors: Vec::new(),
      interactions: Vec::new(),
      parents: Vec::new(),
      texts: Vec::new(),
    }
  }

  /// Walk one container's children up to its `EndElement`, returning
  /// the child node ids so the parent can attach them. Buttons and
  /// text-tags collapse their text children into one leaf (Taffy has
  /// no inline-formatting context). `parent` is the placement index of
  /// the enclosing paintable container, recorded on every placement so
  /// a nesting runtime can reparent — `None` directly under the root.
  fn children(
    &mut self,
    cmds: &[UiCommand],
    parent: Option<usize>,
  ) -> Vec<NodeId> {
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
          let node = self.leaf(LeafPlacement {
            idx,
            text,
            style: ComputedStyle::ROOT,
            author: StylePatch::EMPTY,
            interactions: InteractionAuthors::default(),
            size: None,
            parent,
          });

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
          let author = resolve_author(&self.author, tag.as_str(), attrs);
          let interactions =
            resolve_interactions(&self.author, tag.as_str(), attrs);
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
            let node = self.leaf(LeafPlacement {
              idx,
              text,
              style,
              author,
              interactions,
              size,
              parent,
            });

            children.push(node);

            if !self_closing {
              skip_to_end(cmds, &mut self.cursor);
            }
          } else {
            // Container. Its main axis follows a declared `display:
            // flex` direction, else the inline-vs-block flow of kids.
            let direction = container_direction(&style, cmds, self.cursor);

            if is_paintable(&style, author.as_ref()) {
              // A declared surface (colour / image / glass): record the
              // placement BEFORE its children so the flat subview order
              // is back-to-front — the surface sits behind the content
              // it wraps. The text mirrors `leaf_text` so `reconcile`
              // sees no spurious change (the backdrop ignores it).
              let node = self
                .tree
                .new_leaf(to_taffy(&style, direction))
                .expect("taffy container");

              // This container's own placement index — its children
              // point here as their parent.
              let placement = self.cmd_index.len();

              self.cmd_index.push(idx);
              self.nodes.push(node);
              self.styles.push(style);
              self.authors.push(author.unwrap_or(StylePatch::EMPTY));
              self.interactions.push(interactions);
              self.parents.push(parent);
              self.texts.push(leaf_text(cmds, idx));

              if !self_closing {
                // Only glass nests its children: UIKit composites a
                // child into the glass effect view's `contentView`. A
                // colour / image surface stays a flat sibling, with its
                // children layered on top (no compositing requirement).
                let inner = if matches!(style.material, Material::Glass(_)) {
                  Some(placement)
                } else {
                  parent
                };
                let kids = self.children(cmds, inner);

                self
                  .tree
                  .set_children(node, &kids)
                  .expect("taffy set_children");
              }

              children.push(node);
            } else {
              // Geometry-only: no surface, so no view — the common
              // container pays nothing. Its children keep the incoming
              // parent (this node paints nothing to nest them into).
              let kids = if self_closing {
                Vec::new()
              } else {
                self.children(cmds, parent)
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
    }

    children
  }

  /// Create a measured leaf node, recording it in the side tables.
  /// `size` pins an explicit box (images, inputs); otherwise the box
  /// is `auto` and the measure closure sizes it from the text.
  /// `parent` is the enclosing paintable container's placement index.
  fn leaf(&mut self, placement: LeafPlacement) -> NodeId {
    let LeafPlacement {
      idx,
      text,
      style,
      author,
      interactions,
      size,
      parent,
    } = placement;
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
    self.interactions.push(interactions);
    self.parents.push(parent);
    self.texts.push(text);

    node
  }
}

/// Everything one measured leaf records in the side tables.
struct LeafPlacement {
  /// Index of the element's command in the source stream.
  idx: usize,
  /// The collapsed text the measure closure sizes.
  text: String,
  /// The cascaded style (UA + author).
  style: ComputedStyle,
  /// The author patch (declared properties to paint).
  author: StylePatch,
  /// Interaction-state patches for the paint-time overlay.
  interactions: InteractionAuthors,
  /// Explicit box override (images, inputs); `None` sizes from text.
  size: Option<TaffySize<Dimension>>,
  /// Enclosing paintable container's placement index.
  parent: Option<usize>,
}

/// Parse every `StyleSheet` command into one ordered list of author
/// rules plus the combined image catalog the cascade folds in. Each
/// sheet's `background_image` handles are offset into the combined
/// catalog so indices stay valid when several sheets are concatenated.
fn collect_author(commands: &[UiCommand]) -> (Vec<css::CssRule>, Vec<String>) {
  let mut rules = Vec::new();
  let mut images = Vec::new();

  for cmd in commands {
    if let UiCommand::StyleSheet { css, .. } = cmd {
      let mut sheet = css::parse(css);
      let base = images.len() as u32;

      if base > 0 {
        for rule in &mut sheet.rules {
          if let Some(id) = rule.patch.background_image.as_mut() {
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

/// Resolve an element's author patch: its tag rule, then every
/// `.class` rule its `class` attribute names folded on top (class wins
/// — higher specificity). Keeps native styling in step with the web,
/// where `.card { … }` already applies. `None` when nothing targets it.
fn resolve_author(
  rules: &[css::CssRule],
  tag: &str,
  attrs: &[Attr],
) -> Option<StylePatch> {
  let mut author = css::author_patch(rules, tag);

  if let Some(classes) = class_attr(attrs) {
    for class in classes.split_whitespace() {
      if let Some(patch) = css::author_patch(rules, &format!(".{class}")) {
        author.get_or_insert(StylePatch::EMPTY).overlay(&patch);
      }
    }
  }

  author
}

/// Per-element patches for each interaction state, resolved once at
/// build (the rules are static); the renderer overlays the one
/// matching the element's current state at paint time.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InteractionAuthors {
  pub hover: Option<StylePatch>,
  pub active: Option<StylePatch>,
  pub focus: Option<StylePatch>,
  pub disabled: Option<StylePatch>,
}

impl InteractionAuthors {
  /// True when no state rule targets the element — the renderer's
  /// fast path skips state tracking entirely.
  pub fn is_empty(&self) -> bool {
    self.hover.is_none()
      && self.active.is_none()
      && self.focus.is_none()
      && self.disabled.is_none()
  }
}

/// Resolve an element's interaction-state patches: tag rules, then
/// `.class` rules folded on top — the same specificity order as
/// `resolve_author`.
fn resolve_interactions(
  rules: &[css::CssRule],
  tag: &str,
  attrs: &[Attr],
) -> InteractionAuthors {
  let fold = |state: css::Interaction| {
    let mut merged = css::author_state_patch(rules, tag, state);

    if let Some(classes) = class_attr(attrs) {
      for class in classes.split_whitespace() {
        if let Some(patch) =
          css::author_state_patch(rules, &format!(".{class}"), state)
        {
          merged.get_or_insert(StylePatch::EMPTY).overlay(&patch);
        }
      }
    }

    merged
  };

  InteractionAuthors {
    hover: fold(css::Interaction::Hover),
    active: fold(css::Interaction::Active),
    focus: fold(css::Interaction::Focus),
    disabled: fold(css::Interaction::Disabled),
  }
}

/// The element's `class` attribute value, if any.
fn class_attr(attrs: &[Attr]) -> Option<&str> {
  attrs.iter().find(|attr| attr.name() == "class")?.as_str()
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
    flex_wrap: match style.flex_wrap {
      FlexWrap::Wrap => TaffyFlexWrap::Wrap,
      FlexWrap::NoWrap => TaffyFlexWrap::NoWrap,
    },
    flex_grow: style.flex_grow,
    flex_shrink: style.flex_shrink,
    justify_content: Some(to_justify(style.justify_content)),
    align_items: Some(to_align(style.align_items)),
    gap: length(style.gap),
    size: to_size(style.width, style.height),
    max_size: to_size(style.max_width, style.max_height),
    aspect_ratio: (style.aspect_ratio > 0.0).then_some(style.aspect_ratio),
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

/// Whether a container declares a surface to paint behind its children
/// — a declared `background` colour (not the inherited default), a
/// `background-image`, or a glass material. A plain layout container
/// paints nothing, so it stays a geometry-only node with no view.
fn is_paintable(style: &ComputedStyle, author: Option<&StylePatch>) -> bool {
  author.is_some_and(|patch| patch.background.is_some())
    || style.background_image.is_some()
    || style.material != Material::Solid
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
fn to_size(width: Size, height: Size) -> TaffySize<Dimension> {
  TaffySize {
    width: to_dimension(width),
    height: to_dimension(height),
  }
}

fn to_dimension(size: Size) -> Dimension {
  match size {
    Size::Auto => Dimension::auto(),
    Size::Px(value) => Dimension::length(value),
    // zo stores percent as 0–100; taffy wants a 0–1 fraction.
    Size::Percent(value) => Dimension::percent(value / 100.0),
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
  fn class_rule_resolves_on_native() {
    // Native must match the web: a `.card` rule applies to an element
    // carrying that class, folded over any tag rule.
    let rules = css::parse(".card { material: glass; }").rules;
    let attrs = vec![Attr::parse_prop("class", "card")];
    let author = resolve_author(&rules, "div", &attrs).unwrap();

    assert!(matches!(author.material, Some(Material::Glass(_))));
  }

  #[test]
  fn paintable_container_is_placed_before_its_children() {
    // `div { material: glass }` makes the container paintable, so it
    // earns a placement — recorded before its children for back-to-
    // front z-order (the surface sits behind the content).
    let cmds = vec![
      UiCommand::StyleSheet {
        css: "div { material: glass; }".into(),
        scope: zo_ui_protocol::StyleScope::Global,
        scope_hash: None,
      },
      element(ElementTag::Div),
      element(ElementTag::P),
      text("a"),
      UiCommand::EndElement,
      UiCommand::EndElement,
    ];

    let mut tree = LayoutTree::build(&cmds);
    let rects = tree.solve((320.0, 480.0));

    assert_eq!(rects.len(), 2, "the div surface and its paragraph");
    assert_eq!(rects[0].0, 1, "the div (cmd 1) is placed first (behind)");
    assert_eq!(rects[1].0, 2, "its paragraph (cmd 2) after (in front)");
  }

  #[test]
  fn paintable_container_sizes_to_its_children() {
    // A placed paintable container must still lay out its children —
    // `new_leaf` + `set_children` has to behave like `new_with_children`,
    // else the card (and the content inside it) collapse to nothing.
    let cmds = vec![
      UiCommand::StyleSheet {
        css: "div { material: glass; padding: 24px; }".into(),
        scope: zo_ui_protocol::StyleScope::Global,
        scope_hash: None,
      },
      element(ElementTag::Div),
      button("0"),
      text("-"),
      UiCommand::EndElement,
      UiCommand::EndElement,
    ];

    let mut tree = LayoutTree::build(&cmds);
    let rects = tree.solve((320.0, 480.0));

    let card = rects[0].1;
    let inner = rects[1].1;

    assert!(
      card.width > 0.0 && card.height > 0.0,
      "card sized: {card:?}"
    );
    assert!(
      inner.width > 0.0 && inner.height > 0.0,
      "button inside the card is sized: {inner:?}"
    );
  }

  #[test]
  fn glass_container_nests_its_children() {
    // A glass `.card` must report its children's parent as the card's
    // own placement, so a nesting runtime reparents them into the
    // glass effect view's `contentView` (where UIKit composites them).
    let cmds = vec![
      UiCommand::StyleSheet {
        css: "div { material: glass; }".into(),
        scope: zo_ui_protocol::StyleScope::Global,
        scope_hash: None,
      },
      element(ElementTag::Div),
      button("0"),
      text("-"),
      UiCommand::EndElement,
      button("1"),
      text("+"),
      UiCommand::EndElement,
      UiCommand::EndElement,
    ];

    let tree = LayoutTree::build(&cmds);
    let parents = tree.parents();

    // Placement 0 is the glass div (under the root); 1 and 2 are its
    // two buttons, each parented to placement 0.
    assert_eq!(parents[0], None, "the glass div sits on the root");
    assert_eq!(parents[1], Some(0), "first button nests in the glass");
    assert_eq!(parents[2], Some(0), "second button nests in the glass");
  }

  #[test]
  fn colour_container_does_not_nest_its_children() {
    // A solid-colour surface stays a flat sibling: UIKit layers its
    // children on top with no compositing requirement, so they keep
    // the root as their parent (no reparent into the colour view).
    let cmds = vec![
      UiCommand::StyleSheet {
        css: "div { background: #f00; }".into(),
        scope: zo_ui_protocol::StyleScope::Global,
        scope_hash: None,
      },
      element(ElementTag::Div),
      button("0"),
      text("-"),
      UiCommand::EndElement,
      UiCommand::EndElement,
    ];

    let tree = LayoutTree::build(&cmds);
    let parents = tree.parents();

    assert_eq!(parents[0], None, "the colour div sits on the root");
    assert_eq!(parents[1], None, "its button stays a flat sibling");
  }

  #[test]
  fn plain_container_is_not_placed() {
    // A `<div>` with no declared surface stays geometry-only.
    let cmds = vec![
      element(ElementTag::Div),
      element(ElementTag::P),
      text("a"),
      UiCommand::EndElement,
      UiCommand::EndElement,
    ];

    let mut tree = LayoutTree::build(&cmds);
    let rects = tree.solve((320.0, 480.0));

    assert_eq!(rects.len(), 1, "only the paragraph is placed, not the div");
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

#[cfg(test)]
mod apply_dirty_tests {
  use super::*;

  use crate::reactive::DirtyCommands;

  use zo_ui_protocol::{Attr, ElementTag, PropValue};

  /// `<div> <button>a</button> <span>b</span> </div>` — two
  /// placements, text at command indices 2 and 5.
  fn two_leaf_stream() -> Vec<UiCommand> {
    vec![
      UiCommand::Element {
        tag: ElementTag::Div,
        attrs: vec![],
        self_closing: false,
      },
      UiCommand::Element {
        tag: ElementTag::Button,
        attrs: vec![],
        self_closing: false,
      },
      UiCommand::Text("a".into()),
      UiCommand::EndElement,
      UiCommand::Element {
        tag: ElementTag::Span,
        attrs: vec![],
        self_closing: false,
      },
      UiCommand::Text("b".into()),
      UiCommand::EndElement,
      UiCommand::EndElement,
    ]
  }

  #[test]
  fn dirty_text_touches_exactly_its_placement() {
    let commands = two_leaf_stream();
    let mut tree = LayoutTree::build(&commands);

    tree.solve((320.0, 240.0));

    let mut patched = commands.clone();

    patched[5] = UiCommand::Text("B!".into());

    let mut dirty = DirtyCommands::with_capacity(patched.len());

    dirty.mark(5);

    let changed = tree.apply_dirty(&dirty, &patched);

    assert_eq!(changed.len(), 1, "exactly one placement repaints");
    assert_eq!(changed[0].1, "B!");

    // The other placement's cached text is untouched.
    assert!(tree.texts.iter().any(|t| t == "a"));
  }

  #[test]
  fn unchanged_dirty_command_repaints_nothing() {
    let commands = two_leaf_stream();
    let mut tree = LayoutTree::build(&commands);

    tree.solve((320.0, 240.0));

    let mut dirty = DirtyCommands::with_capacity(commands.len());

    dirty.mark(5);

    // Marked dirty but the text is identical — no repaint.
    let changed = tree.apply_dirty(&dirty, &commands);

    assert!(changed.is_empty(), "no-op write must not repaint");
  }

  #[test]
  fn attr_only_patch_surfaces_its_placement() {
    let commands = two_leaf_stream();
    let mut tree = LayoutTree::build(&commands);

    tree.solve((320.0, 240.0));

    let mut patched = commands.clone();

    patched[4] = UiCommand::Element {
      tag: ElementTag::Span,
      attrs: vec![Attr::Prop {
        name: "class".into(),
        value: PropValue::Str("lit".into()),
      }],
      self_closing: false,
    };

    let mut dirty = DirtyCommands::with_capacity(patched.len());

    dirty.mark(4);

    let changed = tree.apply_dirty(&dirty, &patched);

    assert_eq!(changed.len(), 1, "attr patch repaints its widget");
  }
}

#[cfg(test)]
mod append_list_items_tests {
  use super::*;

  use zo_ui_protocol::ElementTag;

  fn li(text: &str) -> Vec<UiCommand> {
    vec![
      UiCommand::Element {
        tag: ElementTag::Li,
        attrs: vec![],
        self_closing: false,
      },
      UiCommand::Text(text.into()),
      UiCommand::EndElement,
    ]
  }

  /// `<ul><li>a</li><li>b</li></ul><button>go</button>`.
  fn list_stream() -> Vec<UiCommand> {
    let mut cmds = vec![UiCommand::Element {
      tag: ElementTag::Ul,
      attrs: vec![],
      self_closing: false,
    }];

    cmds.extend(li("a"));
    cmds.extend(li("b"));
    cmds.push(UiCommand::EndElement);
    cmds.push(UiCommand::Element {
      tag: ElementTag::Button,
      attrs: vec![],
      self_closing: false,
    });
    cmds.push(UiCommand::Text("go".into()));
    cmds.push(UiCommand::EndElement);

    cmds
  }

  #[test]
  fn tail_append_adds_one_placement_and_shifts_downstream() {
    let commands = list_stream();
    let mut tree = LayoutTree::build(&commands);

    tree.solve((320.0, 480.0));

    let placements_before = tree.cmd_index.len();
    let button_before = *tree.cmd_index.last().unwrap();

    // Push "c": splice one recipe stride before the `</ul>` at
    // index 7.
    let mut patched = commands.clone();

    patched.splice(7..7, li("c"));

    let range = tree
      .append_list_items(7, 3, &patched)
      .expect("tail append rides the fast path");

    assert_eq!(range.len(), 1, "one new leaf placement");
    assert_eq!(tree.cmd_index.len(), placements_before + 1);
    assert_eq!(
      *tree.cmd_index.last().unwrap(),
      8,
      "the placement records the item's text leaf (element + 1), \
       matching the build convention"
    );

    // The button placement shifted by the spliced length.
    let button_after = tree.cmd_index[placements_before - 1];

    assert_eq!(button_after, button_before + 3);

    // The new leaf measures into geometry on the next solve.
    let rects = tree.solve((320.0, 480.0));

    assert_eq!(rects.len(), placements_before + 1);
    assert!(tree.texts.iter().any(|t| t == "c"));
  }

  #[test]
  fn append_without_existing_items_falls_back() {
    // `<ul></ul>` — nothing to anchor on; the caller rebuilds.
    let commands = vec![
      UiCommand::Element {
        tag: ElementTag::Ul,
        attrs: vec![],
        self_closing: false,
      },
      UiCommand::EndElement,
    ];
    let mut tree = LayoutTree::build(&commands);

    tree.solve((320.0, 480.0));

    let mut patched = commands.clone();

    patched.splice(1..1, li("a"));

    assert!(tree.append_list_items(1, 3, &patched).is_none());
  }
}
