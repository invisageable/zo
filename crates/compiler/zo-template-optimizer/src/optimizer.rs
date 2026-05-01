//! Template optimizer - Phase 1: Analysis of static vs dynamic parts
//!
//! This module classifies UiCommands based on their runtime behavior:
//! - **Static**: Never changes, can be cached/pre-rendered
//! - **Dynamic**: May change based on reactive data
//! - **Conditional**: Existence depends on runtime conditions
//!
//! Inspired by Malina.js's compile-time template analysis.

use zo_ui_protocol::{Attr, ElementTag, UiCommand};

/// Classification of a UiCommand based on compile-time analysis
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandClassification {
  /// Fully static - never changes after first render
  Static,
  /// Dynamic - may change due to reactive data
  Dynamic,
  /// Conditional - may not be rendered at all
  Conditional,
}

/// Metadata about a command for optimization
#[derive(Debug, Clone)]
pub struct CommandMetadata {
  /// Index in original command sequence
  pub index: usize,
  /// Classification (static/dynamic/conditional)
  pub classification: CommandClassification,
  /// Can be merged with adjacent commands
  pub mergeable: bool,
  /// Requires runtime interactivity (JS/event handling)
  pub needs_interactivity: bool,
}

/// Template optimizer that analyzes and transforms UiCommand sequences
pub struct TemplateOptimizer {
  // Future: Cache for analyzed templates
}

impl TemplateOptimizer {
  /// Create a new optimizer instance
  pub fn new() -> Self {
    Self {}
  }

  /// Analyze commands and classify them as static/dynamic/conditional
  pub fn analyze(&self, commands: &[UiCommand]) -> Vec<CommandClassification> {
    commands
      .iter()
      .map(|cmd| self.classify_command(cmd))
      .collect()
  }

  /// Classify a single command. An element is dynamic if it
  /// carries a reactive `Attr::Dynamic` binding; otherwise the
  /// eager compile-time values make it fully static.
  fn classify_command(&self, cmd: &UiCommand) -> CommandClassification {
    match cmd {
      UiCommand::Element { attrs, .. } => {
        if attrs.iter().any(|a| matches!(a, Attr::Dynamic { .. })) {
          CommandClassification::Dynamic
        } else {
          CommandClassification::Static
        }
      }
      UiCommand::EndElement => CommandClassification::Static,
      UiCommand::Text(_) => CommandClassification::Static,
      UiCommand::Event { .. } => CommandClassification::Dynamic,
      UiCommand::StyleSheet { .. } => CommandClassification::Static,
    }
  }

  /// Optimize command sequence (Phase 2 preview)
  pub fn optimize(&self, commands: Vec<UiCommand>) -> Vec<UiCommand> {
    self.merge_adjacent_text(commands, &[]).0
  }

  /// Merge adjacent `Text` commands into a single node, and
  /// return an `old_idx → new_idx` mapping so callers can
  /// remap any side-channel that points at command indices
  /// (reactive text bindings, attribute bindings).
  ///
  /// `bound_indices` lists input positions that MUST stay
  /// isolated — each is the target of a reactive text
  /// binding, and merging it with surrounding static text
  /// would cause the runtime patch (`*s = new_value`) to
  /// clobber the static prefix/suffix on every update.
  /// Without this barrier, `Text("clicked ") + Text({count})`
  /// merge into `Text("clicked 0")`, and the first click
  /// rewrites the whole string to `Text("1")`.
  ///
  /// Without the mapping, a binding that pointed at the
  /// second of two adjacent texts would dangle on the
  /// `EndElement` (or whatever followed the merged run)
  /// after the merge, and the runtime's
  /// `if let Some(UiCommand::Text(s)) = …` patch path would
  /// silently no-op on every reactive update.
  pub fn optimize_with_indices(
    &self,
    commands: Vec<UiCommand>,
    bound_indices: &[usize],
  ) -> (Vec<UiCommand>, Vec<usize>) {
    self.merge_adjacent_text(commands, bound_indices)
  }

  fn merge_adjacent_text(
    &self,
    commands: Vec<UiCommand>,
    bound_indices: &[usize],
  ) -> (Vec<UiCommand>, Vec<usize>) {
    let mut optimized = Vec::with_capacity(commands.len());
    let mut index_map = Vec::with_capacity(commands.len());
    let mut pending: Option<String> = None;
    let mut pending_out_idx: Option<usize> = None;

    for (in_idx, cmd) in commands.into_iter().enumerate() {
      match cmd {
        UiCommand::Text(s) => {
          let is_bound = bound_indices.contains(&in_idx);

          if is_bound {
            if let Some(buffer) = pending.take() {
              optimized.push(UiCommand::Text(buffer));
              pending_out_idx = None;
            }

            let out_idx = optimized.len();

            optimized.push(UiCommand::Text(s));
            index_map.push(out_idx);

            continue;
          }

          let out_idx = match pending_out_idx {
            Some(i) => i,
            None => {
              let i = optimized.len();
              pending_out_idx = Some(i);
              i
            }
          };

          match &mut pending {
            Some(buffer) => buffer.push_str(&s),
            None => pending = Some(s),
          }

          index_map.push(out_idx);
        }
        _ => {
          if let Some(buffer) = pending.take() {
            optimized.push(UiCommand::Text(buffer));
            pending_out_idx = None;
          }

          let out_idx = optimized.len();

          optimized.push(cmd);
          index_map.push(out_idx);
        }
      }
    }

    if let Some(buffer) = pending {
      optimized.push(UiCommand::Text(buffer));
    }

    (optimized, index_map)
  }

  /// Generate optimization metadata for all commands
  pub fn generate_metadata(
    &self,
    commands: &[UiCommand],
  ) -> Vec<CommandMetadata> {
    commands
      .iter()
      .enumerate()
      .map(|(index, cmd)| {
        let classification = self.classify_command(cmd);

        let needs_interactivity = matches!(
          cmd,
          UiCommand::Event { .. }
            | UiCommand::Element {
              tag: ElementTag::Button
                | ElementTag::Input
                | ElementTag::Textarea,
              ..
            }
        );

        let mergeable = matches!(cmd, UiCommand::Text(_));

        CommandMetadata {
          index,
          classification,
          mergeable,
          needs_interactivity,
        }
      })
      .collect()
  }
}

impl Default for TemplateOptimizer {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn div(id: &str) -> UiCommand {
    UiCommand::Element {
      tag: ElementTag::Div,
      attrs: vec![Attr::str_prop("data-id", id)],
      self_closing: false,
    }
  }

  fn button(id: u32, label: &str) -> Vec<UiCommand> {
    vec![
      UiCommand::Element {
        tag: ElementTag::Button,
        attrs: vec![Attr::parse_prop("data-id", &id.to_string())],
        self_closing: false,
      },
      UiCommand::Text(label.to_string()),
      UiCommand::EndElement,
    ]
  }

  #[test]
  fn test_classify_static_text_node() {
    let optimizer = TemplateOptimizer::new();
    let cmd = UiCommand::Text("Hello, world!".to_string());

    assert_eq!(
      optimizer.classify_command(&cmd),
      CommandClassification::Static
    );
  }

  #[test]
  fn test_classify_static_element() {
    let optimizer = TemplateOptimizer::new();
    let cmd = div("root");

    assert_eq!(
      optimizer.classify_command(&cmd),
      CommandClassification::Static
    );
  }

  #[test]
  fn test_classify_dynamic_element_when_attr_dynamic() {
    let optimizer = TemplateOptimizer::new();
    let cmd = UiCommand::Element {
      tag: ElementTag::Img,
      attrs: vec![Attr::Dynamic {
        name: "src".into(),
        var: 42,
        initial: zo_ui_protocol::PropValue::Str("a.png".into()),
      }],
      self_closing: true,
    };

    assert_eq!(
      optimizer.classify_command(&cmd),
      CommandClassification::Dynamic
    );
  }

  #[test]
  fn test_classify_event_as_dynamic() {
    let optimizer = TemplateOptimizer::new();
    let cmd = UiCommand::Event {
      widget_id: "0".into(),
      event_kind: zo_ui_protocol::EventKind::Click,
      handler: "on_click".into(),
    };

    assert_eq!(
      optimizer.classify_command(&cmd),
      CommandClassification::Dynamic
    );
  }

  #[test]
  fn test_merge_adjacent_text_nodes() {
    let optimizer = TemplateOptimizer::new();

    let commands = vec![
      UiCommand::Text("Hello ".into()),
      UiCommand::Text("world!".into()),
    ];

    let (optimized, _) = optimizer.merge_adjacent_text(commands, &[]);

    assert_eq!(optimized.len(), 1);

    match &optimized[0] {
      UiCommand::Text(s) => assert_eq!(s, "Hello world!"),
      _ => panic!("Expected TextNode command"),
    }
  }

  #[test]
  fn test_merge_preserves_non_text_boundaries() {
    let optimizer = TemplateOptimizer::new();

    let commands = vec![
      UiCommand::Text("before".into()),
      div("mid"),
      UiCommand::Text("after".into()),
    ];

    let (optimized, _) = optimizer.merge_adjacent_text(commands, &[]);

    assert_eq!(optimized.len(), 3);
    assert!(matches!(optimized[0], UiCommand::Text(_)));
    assert!(matches!(optimized[1], UiCommand::Element { .. }));
    assert!(matches!(optimized[2], UiCommand::Text(_)));
  }

  #[test]
  fn test_analyze_mixed_commands() {
    let optimizer = TemplateOptimizer::new();

    let mut commands = vec![div("root"), UiCommand::Text("static".into())];

    commands.extend(button(1, "Click"));
    commands.push(UiCommand::EndElement);

    let classifications = optimizer.analyze(&commands);

    assert_eq!(classifications.len(), commands.len());
    assert_eq!(classifications[0], CommandClassification::Static);
    assert_eq!(classifications[1], CommandClassification::Static);
    // Button element is classified static unless it carries a
    // dynamic attribute. `needs_interactivity` in the metadata
    // pass handles the event routing concern.
    assert_eq!(classifications[2], CommandClassification::Static);
  }

  #[test]
  fn test_metadata_marks_button_as_interactive() {
    let optimizer = TemplateOptimizer::new();
    let commands = button(1, "Click");

    let metadata = optimizer.generate_metadata(&commands);

    assert_eq!(metadata.len(), 3);
    // Element { Button }
    assert!(metadata[0].needs_interactivity);
    // TextNode
    assert!(metadata[1].mergeable);
    assert!(!metadata[1].needs_interactivity);
    // EndElement
    assert!(!metadata[2].needs_interactivity);
  }

  #[test]
  fn test_bound_text_blocks_merge() {
    // Regression: a binding-target text MUST stay isolated
    // so the runtime's `*s = new_value` patch only
    // overwrites the dynamic value. Without the barrier,
    // `Text("clicked ") + Text("0")` merges into
    // `Text("clicked 0")` and the first click rewrites the
    // whole string to `Text("1")`, clobbering the prefix.
    let optimizer = TemplateOptimizer::new();

    let commands = vec![
      UiCommand::Text("clicked ".into()),
      UiCommand::Text("0".into()),
      UiCommand::Text(" times".into()),
    ];

    // Bound input idx 1 — the dynamic `{count}` slot.
    let (optimized, index_map) = optimizer.merge_adjacent_text(commands, &[1]);

    assert_eq!(
      optimized.len(),
      3,
      "bound text must split the run into 3 segments"
    );

    match (&optimized[0], &optimized[1], &optimized[2]) {
      (UiCommand::Text(a), UiCommand::Text(b), UiCommand::Text(c)) => {
        assert_eq!(a, "clicked ");
        assert_eq!(b, "0");
        assert_eq!(c, " times");
      }
      _ => panic!("expected three Text commands, got {optimized:?}"),
    }

    // Index map: bound input 1 must point at the isolated
    // output 1 so the runtime patches the right slot.
    assert_eq!(index_map[1], 1);
    assert_eq!(index_map[0], 0);
    assert_eq!(index_map[2], 2);
  }

  #[test]
  fn test_bound_text_at_run_start() {
    // Bound text at the start of a text run still gets its
    // own slot. Prior runs of free texts (none here) flush;
    // texts after the bound one merge into a fresh run.
    let optimizer = TemplateOptimizer::new();

    let commands = vec![
      UiCommand::Text("0".into()),
      UiCommand::Text(" ms".into()),
      UiCommand::Text(" elapsed".into()),
    ];

    let (optimized, index_map) = optimizer.merge_adjacent_text(commands, &[0]);

    assert_eq!(optimized.len(), 2);
    assert!(matches!(&optimized[0], UiCommand::Text(s) if s == "0"));
    assert!(matches!(&optimized[1], UiCommand::Text(s) if s == " ms elapsed"));

    assert_eq!(index_map[0], 0);
    assert_eq!(index_map[1], 1);
    assert_eq!(index_map[2], 1); // merged into output 1
  }

  #[test]
  fn test_two_bound_texts_stay_isolated() {
    // Adjacent bound texts (e.g. two interps separated by
    // whitespace that the executor stripped) each get
    // their own slot.
    let optimizer = TemplateOptimizer::new();

    let commands =
      vec![UiCommand::Text("0".into()), UiCommand::Text("times".into())];

    let (optimized, index_map) =
      optimizer.merge_adjacent_text(commands, &[0, 1]);

    assert_eq!(optimized.len(), 2);
    assert_eq!(index_map[0], 0);
    assert_eq!(index_map[1], 1);
  }
}
