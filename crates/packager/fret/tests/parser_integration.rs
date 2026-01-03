//! Integration tests for the fret.oz parser.

use fret::{Version, parse_config};

#[test]
fn test_minimal_config() {
  let source = r#"
@pack = (
  name: "minimal",
  version: "1.0.0",
)
"#;

  let config = parse_config(source).expect("Failed to parse minimal config");

  assert_eq!(config.name, "minimal");
  assert_eq!(config.version.major, 1);
  assert_eq!(config.version.minor, 0);
  assert_eq!(config.version.patch, 0);

  assert_eq!(config.binary_name, "minimal");
  assert_eq!(config.optimization_level, 0);
  assert!(config.debug_symbols);
}

#[test]
fn test_full_config() {
  let source = r#"
-- Complete configuration example
@pack = (
  name: "zo-compiler",
  version: "0.2.1",
  authors: [
    "invisageable <invisible@example.com>",
    "contributor <contrib@example.com>"
  ],
  license: "MIT",
  entry_point: "src/compiler.zo",
  source_dir: "source",
  binary_name: "zoc",
  optimization_level: 3,
  debug_symbols: false,
)
"#;

  let config = parse_config(source).expect("Failed to parse full config");

  println!("{config:?}");

  assert_eq!(config.name, "zo-compiler");
  assert_eq!(
    config.version,
    Version {
      major: 0,
      minor: 2,
      patch: 1
    }
  );
  assert_eq!(config.entry_point.to_str().unwrap(), "src/compiler.zo");
  assert_eq!(config.source_dir.to_str().unwrap(), "source");
  assert_eq!(config.binary_name, "zoc");
  assert_eq!(config.optimization_level, 3);
  assert!(!config.debug_symbols);
}

#[test]
fn test_config_with_escapes() {
  let source = r#"
@pack = (
  name: "test-escape",
  version: "1.0.0",
  authors: ["Name with \"quotes\"", "Tab\there"],
  license: "MIT\nOR\nApache-2.0",
)
"#;

  let config =
    parse_config(source).expect("Failed to parse config with escapes");

  assert_eq!(config.name, "test-escape");
}

#[test]
fn test_error_missing_required_field() {
  let source = r#"
@pack = (
  version: "1.0.0",
)
"#;

  let result = parse_config(source);
  assert!(result.is_err());

  if let Err(e) = result {
    let error_msg = e.to_string();
    assert!(error_msg.contains("Missing required field 'name'"));
  }
}

#[test]
fn test_error_invalid_version() {
  let source = r#"
@pack = (
  name: "test",
  version: "1.0",  -- Missing patch version.
)
"#;

  let result = parse_config(source);
  assert!(result.is_err());

  if let Err(e) = result {
    let error_msg = e.to_string();
    assert!(error_msg.contains("Version must be in format"));
  }
}

#[test]
fn test_error_unknown_field() {
  let source = r#"
@pack = (
  name: "test",
  version: "1.0.0",
  unknown_field: "value",
)
"#;

  let result = parse_config(source);
  assert!(result.is_err());

  if let Err(e) = result {
    let error_msg = e.to_string();
    assert!(error_msg.contains("Unknown field"));
  }
}

#[test]
fn test_whitespace_and_formatting() {
  // Test various formatting styles
  let source = r#"
    @pack = (name:"compact",version:"1.0.0",)
"#;

  let config = parse_config(source).expect("Failed to parse compact format");
  assert_eq!(config.name, "compact");

  // Test with extra whitespace
  let source2 = r#"


@pack = (
    name    :    "spaced"    ,
    version :    "1.0.0"     ,


)


"#;

  let config2 = parse_config(source2).expect("Failed to parse spaced format");
  assert_eq!(config2.name, "spaced");
}
