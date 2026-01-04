//! Example: Building a zo project for multiple native targets
//!
//! This demonstrates how to use fret to compile the same project
//! for different native platforms (x86_64 and aarch64 on Linux, macOS,
//! Windows).

use fret::{Pipeline, Target};

use std::fs;
use std::path::PathBuf;
use std::time::Instant;

fn main() {
  // Get project path from command line or use current directory
  let args: Vec<String> = std::env::args().collect();
  let project_path = if args.len() > 1 {
    PathBuf::from(&args[1])
  } else {
    std::env::current_dir().expect("Failed to get current directory")
  };

  println!("Building project: {}", project_path.display());
  println!("{}", "=".repeat(60));

  // Define all native targets we want to build for
  let targets = vec![
    ("x86_64 Linux", Target::X86_64Linux),
    ("x86_64 macOS", Target::X86_64MacOS),
    ("x86_64 Windows", Target::X86_64Windows),
    ("aarch64 Linux", Target::Aarch64Linux),
    ("aarch64 macOS", Target::Aarch64MacOS),
  ];

  // Create pipeline once and reuse it
  let pipeline = Pipeline::simple_mode();

  // Build for each target
  for (name, target) in targets {
    println!("\nBuilding for {}...", name);
    let start = Instant::now();

    match pipeline.execute_with_target(project_path.clone(), Some(target)) {
      Ok(output_path) => {
        let elapsed = start.elapsed();
        println!("✓ Success: {}", output_path.display());
        println!("  Time: {:.2}s", elapsed.as_secs_f64());

        // Show file size
        if let Ok(metadata) = fs::metadata(&output_path) {
          let size = metadata.len();
          println!("  Size: {} bytes", size);
        }
      }
      Err(e) => {
        println!("✗ Failed: {}", e);
      }
    }
  }

  println!("\n{}", "=".repeat(60));
  println!("Build complete!");
}
