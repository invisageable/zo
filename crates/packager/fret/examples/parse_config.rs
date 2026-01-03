//! Example demonstrating how to parse a fret.oz configuration file.

use fret::parse_config;
use std::fs;

fn main() {
  // Example fret.oz content
  let config_content = r#"
-- Project configuration for my-project
@pack = (
  name: "my-project",
  version: "0.1.0",
  authors: ["invisageable <you@example.com>"],
  license: "MIT OR Apache-2.0",
  entry_point: "src/main.zo",
  optimization_level: 2,
  debug_symbols: true,
)
"#;

  // Parse the configuration
  match parse_config(config_content) {
    Ok(config) => {
      println!("Successfully parsed configuration:");
      println!("  Name: {}", config.name);
      println!("  Version: {}", config.version);
      println!("  Entry point: {}", config.entry_point.display());
      println!("  Source dir: {}", config.source_dir.display());
      println!("  Binary name: {}", config.binary_name);
      println!("  Optimization level: {}", config.optimization_level);
      println!("  Debug symbols: {}", config.debug_symbols);
    }
    Err(e) => {
      eprintln!("Failed to parse configuration: {}", e);
    }
  }

  // Example of parsing from a file
  if let Ok(content) = fs::read_to_string("fret.oz") {
    match parse_config(&content) {
      Ok(config) => {
        println!("\nParsed fret.oz from disk:");
        println!("  Project: {} v{}", config.name, config.version);
      }
      Err(e) => {
        eprintln!("Failed to parse fret.oz: {}", e);
      }
    }
  }
}
