//! Dependency graph for reactive template updates
//!
//! This module tracks which UI commands depend on which data sources.
//! Inspired by Malina.js's watcher system but adapted for compile-time
//! analysis.
//!
//! Key concepts:
//! - **Dependency Node**: A UI command that may need updates
//! - **Data Source**: A variable/expression that triggers updates
//! - **Dependency Graph**: The complete mapping of data → UI updates

use zo_interner::Symbol;
use zo_ui_protocol::UiCommand;

/// Represents a data source that can trigger UI updates
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DataSource {
  /// A variable reference (e.g., `name` in `<>{name}</>`)
  Variable(Symbol),
  /// A field access (e.g., `user.name`)
  FieldAccess(Symbol, Symbol),
  /// A function call result (e.g., `getCount()`)
  FunctionCall(Symbol),
  /// Static data (never changes)
  Static,
}

/// A node in the dependency graph representing a UI command
#[derive(Debug, Clone)]
pub struct DependencyNode {
  /// Index of the command in the original sequence
  pub command_index: usize,
  /// Data sources this command depends on
  pub depends_on: Vec<DataSource>,
  /// Indices of commands that must update if this updates
  pub triggers: Vec<usize>,
}

/// The complete dependency graph for a template
#[derive(Debug, Clone)]
pub struct DependencyGraph {
  /// All dependency nodes, indexed by command index
  pub nodes: Vec<DependencyNode>,
  /// Reverse mapping: DataSource → command indices that depend on it
  pub watchers: Vec<(DataSource, Vec<usize>)>,
}

impl DependencyGraph {
  /// Create a new empty dependency graph
  pub fn new() -> Self {
    Self {
      nodes: Vec::new(),
      watchers: Vec::new(),
    }
  }

  /// Build dependency graph from UI commands
  pub fn from_commands(commands: &[UiCommand]) -> Self {
    let mut graph = Self::new();

    // For now, all commands are static (no interpolation yet)
    // Future: parse commands for {expr} interpolations
    for (index, cmd) in commands.iter().enumerate() {
      let depends_on = Self::extract_dependencies(cmd);

      graph.nodes.push(DependencyNode {
        command_index: index,
        depends_on: depends_on.clone(),
        triggers: Vec::new(),
      });

      // Build reverse mapping
      for dep in depends_on {
        graph.add_watcher(dep, index);
      }
    }

    graph
  }

  /// Extract data dependencies from a UiCommand
  fn extract_dependencies(cmd: &UiCommand) -> Vec<DataSource> {
    match cmd {
      // Static structural commands
      UiCommand::BeginContainer { .. } => vec![DataSource::Static],
      UiCommand::EndContainer => vec![DataSource::Static],

      // Text commands - check for interpolations
      // TODO: Parse content for {expr} when we add string interpolation
      UiCommand::Text { .. } => vec![DataSource::Static],

      // Interactive elements - always dynamic
      UiCommand::Button { .. } => vec![DataSource::Static],
      UiCommand::TextInput { .. } => vec![DataSource::Static],
      UiCommand::Event { .. } => vec![DataSource::Static],

      // Images - check for dynamic src
      UiCommand::Image { .. } => vec![DataSource::Static],
    }
  }

  /// Add a watcher for a data source
  fn add_watcher(&mut self, source: DataSource, command_index: usize) {
    // Find existing watcher entry or create new one
    if let Some((_, watchers)) =
      self.watchers.iter_mut().find(|(s, _)| s == &source)
    {
      if !watchers.contains(&command_index) {
        watchers.push(command_index);
      }
    } else {
      self.watchers.push((source, vec![command_index]));
    }
  }

  /// Get all commands that depend on a specific data source
  pub fn get_dependents(&self, source: &DataSource) -> Vec<usize> {
    self
      .watchers
      .iter()
      .find(|(s, _)| s == source)
      .map(|(_, indices)| indices.clone())
      .unwrap_or_default()
  }

  /// Get the dependency node for a command
  pub fn get_node(&self, command_index: usize) -> Option<&DependencyNode> {
    self.nodes.get(command_index)
  }

  /// Calculate update order when a data source changes
  /// Returns command indices in the order they should be updated
  pub fn calculate_update_order(&self, source: &DataSource) -> Vec<usize> {
    let mut order = Vec::new();
    let mut visited = vec![false; self.nodes.len()];

    // Start with direct dependents
    for &idx in &self.get_dependents(source) {
      self.visit_node(idx, &mut visited, &mut order);
    }

    order
  }

  /// DFS visit for topological sort
  fn visit_node(
    &self,
    idx: usize,
    visited: &mut [bool],
    order: &mut Vec<usize>,
  ) {
    if visited[idx] {
      return;
    }

    visited[idx] = true;

    // Visit all nodes this one triggers
    if let Some(node) = self.nodes.get(idx) {
      for &trigger_idx in &node.triggers {
        self.visit_node(trigger_idx, visited, order);
      }
    }

    order.push(idx);
  }

  /// Get statistics about the dependency graph
  pub fn stats(&self) -> DependencyStats {
    let total_nodes = self.nodes.len();
    let static_nodes = self
      .nodes
      .iter()
      .filter(|n| n.depends_on.iter().all(|d| matches!(d, DataSource::Static)))
      .count();
    let dynamic_nodes = total_nodes - static_nodes;
    let total_watchers = self.watchers.len();

    DependencyStats {
      total_nodes,
      static_nodes,
      dynamic_nodes,
      total_watchers,
    }
  }
}

impl Default for DependencyGraph {
  fn default() -> Self {
    Self::new()
  }
}

/// Statistics about a dependency graph
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DependencyStats {
  /// Total number of nodes
  pub total_nodes: usize,
  /// Number of static nodes (never update)
  pub static_nodes: usize,
  /// Number of dynamic nodes (may update)
  pub dynamic_nodes: usize,
  /// Number of unique data sources being watched
  pub total_watchers: usize,
}

#[cfg(test)]
mod tests {
  use super::*;
  use zo_ui_protocol::{ContainerDirection, TextStyle};

  #[test]
  fn test_empty_graph() {
    let graph = DependencyGraph::new();
    assert_eq!(graph.nodes.len(), 0);
    assert_eq!(graph.watchers.len(), 0);
  }

  #[test]
  fn test_static_commands() {
    let commands = vec![
      UiCommand::BeginContainer {
        id: "root".to_string(),
        direction: ContainerDirection::Vertical,
      },
      UiCommand::Text {
        content: "Static text".to_string(),
        style: TextStyle::Normal,
      },
      UiCommand::EndContainer,
    ];

    let graph = DependencyGraph::from_commands(&commands);
    let stats = graph.stats();

    assert_eq!(stats.total_nodes, 3);
    assert_eq!(stats.static_nodes, 3);
    assert_eq!(stats.dynamic_nodes, 0);
  }

  #[test]
  fn test_get_dependents() {
    let commands = vec![UiCommand::Text {
      content: "test".to_string(),
      style: TextStyle::Normal,
    }];

    let graph = DependencyGraph::from_commands(&commands);
    let deps = graph.get_dependents(&DataSource::Static);

    // All static commands should be in the dependents list
    assert!(deps.contains(&0));
  }

  #[test]
  fn test_get_node() {
    let commands = vec![UiCommand::Text {
      content: "test".to_string(),
      style: TextStyle::Normal,
    }];

    let graph = DependencyGraph::from_commands(&commands);
    let node = graph.get_node(0);

    assert!(node.is_some());
    assert_eq!(node.unwrap().command_index, 0);
  }

  #[test]
  fn test_update_order() {
    let commands = vec![
      UiCommand::Text {
        content: "first".to_string(),
        style: TextStyle::Normal,
      },
      UiCommand::Text {
        content: "second".to_string(),
        style: TextStyle::Normal,
      },
    ];

    let graph = DependencyGraph::from_commands(&commands);
    let order = graph.calculate_update_order(&DataSource::Static);

    // Both nodes depend on Static, so both should be in update order
    assert_eq!(order.len(), 2);
  }
}
