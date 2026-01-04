//! Benchmark demonstrating fret's compilation speed.
//!
//! This creates a large project and measures compilation throughput.

use fret::Pipeline;

use rayon::prelude::*;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

const NUM_FILES: usize = 100;
const FUNCTIONS_PER_FILE: usize = 50;

fn main() -> Result<(), Box<dyn std::error::Error>> {
  println!("fret Pipeline Benchmark");
  println!("======================");
  println!("Files: {}", NUM_FILES);
  println!("Functions per file: {}", FUNCTIONS_PER_FILE);

  // Create benchmark project
  let project_dir = PathBuf::from("benchmark_project");
  let setup_start = Instant::now();
  setup_benchmark_project(&project_dir)?;
  let setup_time = setup_start.elapsed();

  // Count total lines
  let total_lines = count_lines(&project_dir.join("src"))?;
  println!("Total lines: {}", total_lines);
  println!("Setup time: {:.3}s", setup_time.as_secs_f64());
  println!();

  // Run the build
  println!("Starting build...");
  let build_start = Instant::now();

  let pipeline = Pipeline::simple_mode();
  match pipeline.execute(project_dir.clone()) {
    Ok(binary_path) => {
      let build_time = build_start.elapsed();

      println!("\nBuild Results:");
      println!("--------------");
      println!("Binary: {}", binary_path.display());
      println!("Build time: {:.3}s", build_time.as_secs_f64());

      // Calculate throughput
      let loc_per_sec = total_lines as f64 / build_time.as_secs_f64();
      println!("Throughput: {:.0} lines/second", loc_per_sec);

      // Performance breakdown (if we exceed 1M LoC/s)
      if loc_per_sec >= 1_000_000.0 {
        println!("\n🚀 ACHIEVED 1M+ LoC/s!");
      }
    }
    Err(e) => {
      eprintln!("Build failed: {}", e);
      return Err(e.into());
    }
  }

  // Cleanup
  fs::remove_dir_all(&project_dir)?;

  Ok(())
}

/// Create a large benchmark project
fn setup_benchmark_project(dir: &Path) -> std::io::Result<()> {
  fs::create_dir_all(dir.join("src"))?;

  // Write fret.oz
  let config = r#"
package benchmark {
  name = "benchmark"
  version = "1.0.0"
  entry = "src/main.zo"
  opt_level = 3
}
"#;
  fs::write(dir.join("fret.oz"), config)?;

  // Generate source files in parallel
  let src_dir = dir.join("src");
  (0..NUM_FILES).into_par_iter().for_each(|i| {
    let content = generate_source_file(i);
    let path = src_dir.join(format!("module_{}.zo", i));
    fs::write(path, content).expect("Failed to write file");
  });

  // Write main.zo
  let mut main_content = String::from("fn main() {\n");
  for i in 0..10 {
    main_content.push_str(&format!("  module_{}_func_0()\n", i));
  }
  main_content.push_str("}\n");
  fs::write(dir.join("src/main.zo"), main_content)?;

  Ok(())
}

/// Generate a source file with many functions
fn generate_source_file(module_id: usize) -> String {
  let mut content = String::with_capacity(FUNCTIONS_PER_FILE * 200);

  for i in 0..FUNCTIONS_PER_FILE {
    // Generate a function with some complexity
    content.push_str(&format!(
      r#"
fun module_{}_func_{}(): int {{
  mut sum := 0;
  for i := 0..100 {{
    sum = sum + i;
    if i % 2 == 0 {{
      sum = sum * 2;
    }} else {{
      sum = sum + 1;
    }}
  }}
  return sum;
}}
"#,
      module_id, i
    ));
  }

  content
}

/// Count total lines in source directory
fn count_lines(dir: &PathBuf) -> std::io::Result<usize> {
  let mut total = 0;
  for entry in fs::read_dir(dir)? {
    let entry = entry?;
    let path = entry.path();
    if path.extension().and_then(|s| s.to_str()) == Some("zo") {
      let content = fs::read_to_string(&path)?;
      total += content.lines().count();
    }
  }
  Ok(total)
}
