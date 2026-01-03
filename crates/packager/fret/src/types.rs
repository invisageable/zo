//! Core data structures for the fret package manager.
//!
//! This module defines the fundamental types that flow through the fret
//! pipeline. All structures are designed following Data-Oriented Design
//! principles:
//! - Flat, cache-friendly layouts
//! - Minimal allocations
//! - Direct, transparent data transformations

use std::path::PathBuf;

/// The central build context that flows through the entire pipeline.
/// This is the primary data artifact that gets transformed at each stage.
#[derive(Debug)]
pub struct BuildContext {
  /// The parsed project configuration from fret.oz
  pub config: ProjectConfig,
  /// The root directory of the project being built
  pub project_root: PathBuf,
  /// Output directory for build artifacts
  pub output_dir: PathBuf,
  /// Source files discovered during the collection phase
  pub source_files: Vec<PathBuf>,
  /// Compilation flags to pass to zo-compiler
  pub compiler_flags: CompilerFlags,
  /// Build mode (debug/release)
  pub build_mode: BuildMode,
  /// Target triple (e.g., x86_64-unknown-linux-gnu)
  pub target: Target,
}
impl BuildContext {
  /// Create a new build context from project config
  pub fn new(config: ProjectConfig, project_root: PathBuf) -> Self {
    let build_mode = if config.debug_symbols {
      BuildMode::Debug
    } else {
      BuildMode::Release
    };

    let output_dir = project_root.join("build").join(match build_mode {
      BuildMode::Debug => "debug",
      BuildMode::Release => "release",
    });

    let compiler_flags = CompilerFlags {
      opt_level: config.optimization_level,
      debug_info: config.debug_symbols,
      target: Target::current(),
      raw_flags: Vec::new(),
    };

    Self {
      config,
      project_root,
      output_dir,
      source_files: Vec::new(),
      compiler_flags,
      build_mode,
      target: Target::current(),
    }
  }
}

/// Parsed representation of fret.oz configuration file.
/// Flat structure optimized for direct access patterns.
#[derive(Debug)]
pub struct ProjectConfig {
  /// Project name (must be valid identifier)
  pub name: String,
  /// Project version (semantic versioning)
  pub version: Version,
  /// Entry point file (e.g., "src/main.zo")
  pub entry_point: PathBuf,
  /// Source directory (default: "src")
  pub source_dir: PathBuf,
  /// Output binary name (defaults to project name)
  pub binary_name: String,
  /// Compiler optimization level (0-3)
  pub optimization_level: u8,
  /// Enable debug symbols
  pub debug_symbols: bool,
  /// Project authors
  pub authors: Vec<String>,
  /// Project license
  pub license: Option<String>,
}

/// Simple semantic version representation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
  pub major: u16,
  pub minor: u16,
  pub patch: u16,
}
impl std::fmt::Display for Version {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
  }
}

/// Compiler flags passed directly to zo-compiler-orchestrator
#[derive(Debug)]
pub struct CompilerFlags {
  /// Optimization level (0-3)
  pub opt_level: u8,
  /// Generate debug info
  pub debug_info: bool,
  /// Target architecture
  pub target: Target,
  /// Additional raw flags
  pub raw_flags: Vec<String>,
}
impl Default for CompilerFlags {
  fn default() -> Self {
    Self {
      opt_level: 0,
      debug_info: false,
      target: Target::current(),
      raw_flags: Vec::new(),
    }
  }
}

/// Build mode configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildMode {
  Debug,
  Release,
}

/// Target platform specification.
/// Only native targets that zo-compiler fully supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
  X86_64Linux,
  X86_64MacOS,
  X86_64Windows,
  Aarch64Linux,
  Aarch64MacOS,
}
impl Target {
  /// Get the current platform target
  pub fn current() -> Self {
    #[cfg(all(target_arch = "x86_64", target_os = "linux"))]
    return Target::X86_64Linux;

    #[cfg(all(target_arch = "x86_64", target_os = "macos"))]
    return Target::X86_64MacOS;

    #[cfg(all(target_arch = "x86_64", target_os = "windows"))]
    return Target::X86_64Windows;

    #[cfg(all(target_arch = "aarch64", target_os = "linux"))]
    return Target::Aarch64Linux;

    #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
    return Target::Aarch64MacOS;

    #[cfg(not(any(
      all(target_arch = "x86_64", target_os = "linux"),
      all(target_arch = "x86_64", target_os = "macos"),
      all(target_arch = "x86_64", target_os = "windows"),
      all(target_arch = "aarch64", target_os = "linux"),
      all(target_arch = "aarch64", target_os = "macos"),
    )))]
    compile_error!("Unsupported target platform");
  }

  /// Check if this is a native target that produces machine code
  pub fn is_native(&self) -> bool {
    matches!(
      self,
      Target::X86_64Linux
        | Target::X86_64MacOS
        | Target::X86_64Windows
        | Target::Aarch64Linux
        | Target::Aarch64MacOS
    )
  }


  /// Get the file extension for this target's output
  pub fn output_extension(&self) -> &'static str {
    match self {
      Target::X86_64Linux
      | Target::X86_64MacOS
      | Target::Aarch64Linux
      | Target::Aarch64MacOS => "",
      Target::X86_64Windows => ".exe",
    }
  }

  /// Get the target triple string
  pub fn triple(&self) -> &'static str {
    match self {
      Target::X86_64Linux => "x86_64-unknown-linux-gnu",
      Target::X86_64MacOS => "x86_64-apple-darwin",
      Target::X86_64Windows => "x86_64-pc-windows-msvc",
      Target::Aarch64Linux => "aarch64-unknown-linux-gnu",
      Target::Aarch64MacOS => "aarch64-apple-darwin",
    }
  }
}

/// Pipeline stage trait - each stage transforms BuildContext
pub trait Stage {
  /// Execute this stage, transforming the build context
  fn execute(&self, ctx: &mut BuildContext) -> Result<(), StageError>;
  /// Name of this stage for logging/debugging
  fn name(&self) -> &'static str;
}

/// Errors that can occur during pipeline execution
#[derive(Debug)]
pub enum StageError {
  /// IO error (file not found, permissions, etc.)
  Io(std::io::Error),
  /// Configuration parsing error
  ConfigParse(String),
  /// Source collection error
  SourceCollection(String),
  /// Compilation error from zo-compiler
  Compilation(String),
}
impl std::fmt::Display for StageError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      StageError::Io(e) => write!(f, "IO error: {}", e),
      StageError::ConfigParse(e) => write!(f, "Config parse error: {}", e),
      StageError::SourceCollection(e) => {
        write!(f, "Source collection error: {}", e)
      }
      StageError::Compilation(e) => write!(f, "Compilation error: {}", e),
    }
  }
}
impl std::error::Error for StageError {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    match self {
      StageError::Io(e) => Some(e),
      _ => None,
    }
  }
}
impl From<std::io::Error> for StageError {
  fn from(e: std::io::Error) -> Self {
    StageError::Io(e)
  }
}

/// The pipeline stages for Simple Mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineStage {
  /// Parse fret.oz configuration
  ParseConfig,
  /// Collect source files from source directory
  CollectSources,
  /// Compile all sources to executables (zo-compiler does this in one pass)
  Compile,
}

/// Result of a successful build
#[derive(Debug)]
pub struct BuildArtifact {
  /// Path to the generated binary
  pub binary_path: PathBuf,
  /// Total build time in milliseconds
  pub build_time_ms: u64,
  /// Number of source files compiled
  pub files_compiled: usize,
  /// Total lines of code processed
  pub total_loc: usize,
}
