//! Example demonstrating the fret build pipeline.
//!
//! This shows how to use the fret library to build a zo project
//! programmatically.

use fret::Pipeline;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
  // Create a test project structure
  let project_dir = PathBuf::from("test_project");
  setup_test_project(&project_dir)?;

  println!("Building test project...");
  let start = Instant::now();

  // Create and execute the pipeline
  let pipeline = Pipeline::simple_mode();
  match pipeline.execute(project_dir.clone()) {
    Ok(binary_path) => {
      let elapsed = start.elapsed();
      println!("Build successful!");
      println!("Binary: {}", binary_path.display());
      println!("Time: {:.3}s", elapsed.as_secs_f64());

      // Show compilation speed
      let loc = count_lines(&project_dir.join("src"))?;
      let loc_per_sec = loc as f64 / elapsed.as_secs_f64();
      println!("Speed: {:.0} lines/second", loc_per_sec);
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

/// Create a minimal test project
fn setup_test_project(dir: &Path) -> std::io::Result<()> {
  fs::create_dir_all(dir.join("src"))?;

  // Write fret.oz configuration
  let config = r#"
package hello {
  name = "hello"
  version = "0.1.0"
  entry = "src/main.zo"
}
"#;
  fs::write(dir.join("fret.oz"), config)?;

  // Write main.zo
  let main_code = r#"
fun main() {
  showln("Hello from fret!");
}
"#;
  fs::write(dir.join("src/main.zo"), main_code)?;

  // Write a library file
  let lib_code = r#"
fun greet(name: str): str {
  return "Hello, " + name + "!";
}

fun add(a: int, b: int): int {
  return a + b;
}
"#;
  fs::write(dir.join("src/lib.zo"), lib_code)?;

  Ok(())
}

/// Count lines of code in a directory
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
