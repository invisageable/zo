//! Template optimizer - Phase 1: Analysis of static vs dynamic parts
//!
//! This module classifies UiCommands based on their runtime behavior:
//! - **Static**: Never changes, can be cached/pre-rendered
//! - **Dynamic**: May change based on reactive data
//! - **Conditional**: Existence depends on runtime conditions
//!
//! Inspired by Malina.js's compile-time template analysis.

use zo_ui_protocol::UiCommand;

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

  /// Classify a single command
  fn classify_command(&self, cmd: &UiCommand) -> CommandClassification {
    match cmd {
      // Structural commands - static if content is static
      UiCommand::BeginContainer { .. } => CommandClassification::Static,
      UiCommand::EndContainer => CommandClassification::Static,

      // Text with literal strings is static
      UiCommand::Text { content, .. } => {
        if self.is_static_string(content) {
          CommandClassification::Static
        } else {
          CommandClassification::Dynamic
        }
      }

      // Interactive elements are dynamic by nature
      UiCommand::Button { .. } => CommandClassification::Dynamic,
      UiCommand::TextInput { .. } => CommandClassification::Dynamic,
      UiCommand::Event { .. } => CommandClassification::Dynamic,

      // Images with static src are static
      UiCommand::Image { src, .. } => {
        if self.is_static_string(src) {
          CommandClassification::Static
        } else {
          CommandClassification::Dynamic
        }
      }
    }
  }

  /// Check if a string is fully static (no interpolations/variables)
  /// For now, we consider all strings static since we don't have
  /// interpolation yet. Future: check for `{}` placeholders.
  fn is_static_string(&self, _s: &str) -> bool {
    // TODO: Once we have string interpolation, check for dynamic parts
    // For now, all strings from templates are static literals
    true
  }

  /// Optimize command sequence (Phase 2 preview)
  pub fn optimize(&self, mut commands: Vec<UiCommand>) -> Vec<UiCommand> {
    commands = self.merge_adjacent_text(commands); // Phase 2.1: Merge adjacent text commands.
    commands = self.flatten_containers(commands); // Phase 2.2: Flatten unnecessary nesting

    commands
  }

  /// Merge adjacent Text commands with same style (Phase 2.1)
  fn merge_adjacent_text(&self, commands: Vec<UiCommand>) -> Vec<UiCommand> {
    let mut optimized = Vec::with_capacity(commands.len());
    let mut pending_text: Option<(String, zo_ui_protocol::TextStyle)> = None;

    for cmd in commands {
      match cmd {
        UiCommand::Text { content, style } => {
          if let Some((ref mut buffer, ref pending_style)) = pending_text {
            if pending_style == &style {
              // Same style - merge
              buffer.push_str(&content);
            } else {
              // Different style - flush pending and start new
              optimized.push(UiCommand::Text {
                content: buffer.clone(),
                style: pending_style.clone(),
              });
              pending_text = Some((content, style));
            }
          } else {
            // Start new pending text
            pending_text = Some((content, style));
          }
        }
        _ => {
          // Non-text command - flush any pending text first
          if let Some((buffer, style)) = pending_text.take() {
            optimized.push(UiCommand::Text {
              content: buffer,
              style,
            });
          }
          optimized.push(cmd);
        }
      }
    }

    // Flush any remaining pending text
    if let Some((buffer, style)) = pending_text {
      optimized.push(UiCommand::Text {
        content: buffer,
        style,
      });
    }

    optimized
  }

  /// Flatten unnecessary container nesting (Phase 2.2)
  ///
  /// Merges consecutive containers with the same direction:
  /// Before: BeginContainer(V), Text("a"), EndContainer, BeginContainer(V),
  /// Text("b"), EndContainer After:  BeginContainer(V), Text("a"), Text("b"),
  /// EndContainer
  fn flatten_containers(&self, commands: Vec<UiCommand>) -> Vec<UiCommand> {
    let mut optimized = Vec::with_capacity(commands.len());
    let mut i = 0;

    while i < commands.len() {
      match &commands[i] {
        UiCommand::BeginContainer { id, direction } => {
          // Check if next container (after EndContainer) has same direction
          if let Some(end_idx) = self.find_matching_end(i, &commands) {
            // Check what comes after this container
            if end_idx + 1 < commands.len()
              && let UiCommand::BeginContainer {
                direction: next_dir,
                ..
              } = &commands[end_idx + 1]
              && direction == next_dir
            {
              // Same direction! Flatten by merging contents
              optimized.push(UiCommand::BeginContainer {
                id: id.clone(),
                direction: direction.clone(),
              });

              // Add contents of first container
              for cmd in commands.iter().take(end_idx).skip(i + 1) {
                optimized.push(cmd.clone());
              }

              // Find end of second container
              if let Some(second_end) =
                self.find_matching_end(end_idx + 1, &commands)
              {
                // Add contents of second container
                for cmd in commands.iter().take(second_end).skip(end_idx + 2) {
                  optimized.push(cmd.clone());
                }

                optimized.push(UiCommand::EndContainer);

                // Skip past both containers
                i = second_end + 1;
                continue;
              }
            }
          }

          // Not flattenable - add as-is
          optimized.push(commands[i].clone());
          i += 1;
        }
        _ => {
          optimized.push(commands[i].clone());
          i += 1;
        }
      }
    }

    optimized
  }

  /// Find the matching EndContainer for a BeginContainer at index
  fn find_matching_end(
    &self,
    start: usize,
    commands: &[UiCommand],
  ) -> Option<usize> {
    let mut depth = 0;

    for (i, cmd) in commands.iter().enumerate().skip(start) {
      match cmd {
        UiCommand::BeginContainer { .. } => depth += 1,
        UiCommand::EndContainer => {
          depth -= 1;
          if depth == 0 {
            return Some(i);
          }
        }
        _ => {}
      }
    }

    None
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
          UiCommand::Button { .. }
            | UiCommand::TextInput { .. }
            | UiCommand::Event { .. }
        );
        let mergeable = matches!(cmd, UiCommand::Text { .. });

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

  use zo_ui_protocol::{ContainerDirection, TextStyle};

  #[test]
  fn test_classify_static_text() {
    let optimizer = TemplateOptimizer::new();
    let cmd = UiCommand::Text {
      content: "Hello, world!".to_string(),
      style: TextStyle::Normal,
    };

    assert_eq!(
      optimizer.classify_command(&cmd),
      CommandClassification::Static
    );
  }

  #[test]
  fn test_classify_button_as_dynamic() {
    let optimizer = TemplateOptimizer::new();
    let cmd = UiCommand::Button {
      id: 1,
      content: "Click me".to_string(),
    };

    assert_eq!(
      optimizer.classify_command(&cmd),
      CommandClassification::Dynamic
    );
  }

  #[test]
  fn test_merge_adjacent_text() {
    let optimizer = TemplateOptimizer::new();
    let commands = vec![
      UiCommand::Text {
        content: "Hello ".to_string(),
        style: TextStyle::Normal,
      },
      UiCommand::Text {
        content: "world!".to_string(),
        style: TextStyle::Normal,
      },
    ];

    let optimized = optimizer.merge_adjacent_text(commands);
    assert_eq!(optimized.len(), 1);

    if let UiCommand::Text { content, .. } = &optimized[0] {
      assert_eq!(content, "Hello world!");
    } else {
      panic!("Expected Text command");
    }
  }

  #[test]
  fn test_no_merge_different_styles() {
    let optimizer = TemplateOptimizer::new();
    let commands = vec![
      UiCommand::Text {
        content: "Hello".to_string(),
        style: TextStyle::Normal,
      },
      UiCommand::Text {
        content: "World".to_string(),
        style: TextStyle::Heading1,
      },
    ];

    let optimized = optimizer.merge_adjacent_text(commands);
    assert_eq!(optimized.len(), 2);
  }

  #[test]
  fn test_analyze_mixed_commands() {
    let optimizer = TemplateOptimizer::new();
    let commands = vec![
      UiCommand::BeginContainer {
        id: "root".to_string(),
        direction: ContainerDirection::Vertical,
      },
      UiCommand::Text {
        content: "Static text".to_string(),
        style: TextStyle::Heading1,
      },
      UiCommand::Button {
        id: 1,
        content: "Dynamic button".to_string(),
      },
      UiCommand::EndContainer,
    ];

    let classifications = optimizer.analyze(&commands);
    assert_eq!(classifications.len(), 4);
    assert_eq!(classifications[0], CommandClassification::Static);
    assert_eq!(classifications[1], CommandClassification::Static);
    assert_eq!(classifications[2], CommandClassification::Dynamic);
    assert_eq!(classifications[3], CommandClassification::Static);
  }

  #[test]
  fn test_metadata_generation() {
    let optimizer = TemplateOptimizer::new();
    let commands = vec![
      UiCommand::Text {
        content: "Hello".to_string(),
        style: TextStyle::Normal,
      },
      UiCommand::Button {
        id: 1,
        content: "Click".to_string(),
      },
    ];

    let metadata = optimizer.generate_metadata(&commands);
    assert_eq!(metadata.len(), 2);

    assert_eq!(metadata[0].classification, CommandClassification::Static);
    assert!(metadata[0].mergeable);
    assert!(!metadata[0].needs_interactivity);

    assert_eq!(metadata[1].classification, CommandClassification::Dynamic);
    assert!(!metadata[1].mergeable);
    assert!(metadata[1].needs_interactivity);
  }

  #[test]
  fn test_flatten_consecutive_containers() {
    let optimizer = TemplateOptimizer::new();
    let commands = vec![
      UiCommand::BeginContainer {
        id: "first".to_string(),
        direction: ContainerDirection::Vertical,
      },
      UiCommand::Text {
        content: "A".to_string(),
        style: TextStyle::Normal,
      },
      UiCommand::EndContainer,
      UiCommand::BeginContainer {
        id: "second".to_string(),
        direction: ContainerDirection::Vertical,
      },
      UiCommand::Text {
        content: "B".to_string(),
        style: TextStyle::Normal,
      },
      UiCommand::EndContainer,
    ];

    let optimized = optimizer.flatten_containers(commands);

    // Should be flattened to 1 container with 2 text nodes
    assert_eq!(optimized.len(), 4);
    assert!(matches!(optimized[0], UiCommand::BeginContainer { .. }));
    assert!(matches!(optimized[1], UiCommand::Text { .. }));
    assert!(matches!(optimized[2], UiCommand::Text { .. }));
    assert!(matches!(optimized[3], UiCommand::EndContainer));
  }

  #[test]
  fn test_no_flatten_different_directions() {
    let optimizer = TemplateOptimizer::new();
    let commands = vec![
      UiCommand::BeginContainer {
        id: "first".to_string(),
        direction: ContainerDirection::Vertical,
      },
      UiCommand::Text {
        content: "A".to_string(),
        style: TextStyle::Normal,
      },
      UiCommand::EndContainer,
      UiCommand::BeginContainer {
        id: "second".to_string(),
        direction: ContainerDirection::Horizontal,
      },
      UiCommand::Text {
        content: "B".to_string(),
        style: TextStyle::Normal,
      },
      UiCommand::EndContainer,
    ];

    let optimized = optimizer.flatten_containers(commands.clone());

    // Should NOT be flattened (different directions)
    assert_eq!(optimized.len(), commands.len());
  }

  #[test]
  fn test_find_matching_end() {
    let optimizer = TemplateOptimizer::new();
    let commands = vec![
      UiCommand::BeginContainer {
        id: "outer".to_string(),
        direction: ContainerDirection::Vertical,
      },
      UiCommand::BeginContainer {
        id: "inner".to_string(),
        direction: ContainerDirection::Vertical,
      },
      UiCommand::EndContainer,
      UiCommand::EndContainer,
    ];

    // Find end of outer container
    assert_eq!(optimizer.find_matching_end(0, &commands), Some(3));
    // Find end of inner container
    assert_eq!(optimizer.find_matching_end(1, &commands), Some(2));
  }
}
