//! Integration tests for native compilation targets

use fret::{Pipeline, Target};

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Create a test project with the given configuration
fn create_test_project(
  dir: &TempDir,
  config: &str,
  source_code: &str,
) -> PathBuf {
  let project_dir = dir.path().to_path_buf();
  fs::write(project_dir.join("fret.oz"), config).unwrap();

  // Create source directory
  let src_dir = project_dir.join("src");
  fs::create_dir(&src_dir).unwrap();

  // Write main.zo
  fs::write(src_dir.join("main.zo"), source_code).unwrap();

  project_dir
}

#[test]
#[ignore = "requires native codegen implementation"]
fn test_native_target() {
  let temp_dir = TempDir::new().unwrap();
  let config = r#"
@pack = (
  name: "test-native",
  version: "1.0.0",
  authors: ["Test Author"],
  license: "MIT",
  debug_symbols: true,
  optimization_level: 0,
)
"#;

  let source = r#"
fun main() {
  showln("Hello from native!");
}
"#;

  let project_dir = create_test_project(&temp_dir, config, source);
  let pipeline = Pipeline::simple_mode();

  // Test with current platform target
  let result =
    pipeline.execute_with_target(project_dir.clone(), Some(Target::current()));

  // Compilation should succeed
  assert!(result.is_ok(), "Native compilation failed: {:?}", result);

  // Check that the returned binary path exists
  let binary_path = result.unwrap();
  assert!(
    binary_path.exists(),
    "Binary should exist at {:?}",
    binary_path
  );
}

#[test]
fn test_target_selection() {
  // Test that all native targets can be created
  let targets = vec![
    Target::X86_64Linux,
    Target::X86_64MacOS,
    Target::X86_64Windows,
    Target::Aarch64Linux,
    Target::Aarch64MacOS,
  ];

  for target in targets {
    // All targets are native
    assert!(target.is_native());

    // Check output extension
    match target {
      Target::X86_64Windows => assert_eq!(target.output_extension(), ".exe"),
      _ => assert_eq!(target.output_extension(), ""),
    }

    // Check triple - all native targets have triples
    let triple = target.triple();
    assert!(!triple.is_empty());
    assert!(triple.contains('-'));
  }
}

#[test]
fn test_current_target() {
  let current = Target::current();

  // Should match the host platform
  #[cfg(all(target_arch = "x86_64", target_os = "linux"))]
  assert_eq!(current, Target::X86_64Linux);

  #[cfg(all(target_arch = "x86_64", target_os = "macos"))]
  assert_eq!(current, Target::X86_64MacOS);

  #[cfg(all(target_arch = "x86_64", target_os = "windows"))]
  assert_eq!(current, Target::X86_64Windows);

  #[cfg(all(target_arch = "aarch64", target_os = "linux"))]
  assert_eq!(current, Target::Aarch64Linux);

  #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
  assert_eq!(current, Target::Aarch64MacOS);
}
