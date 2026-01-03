//! fret - The blazing fast package manager for zo
//!
//! Usage: fret build [OPTIONS] [PATH]

use fret::pipeline::Pipeline;
use fret::types::Target;
use std::env;
use std::path::PathBuf;
use std::time::Instant;

fn main() {
  let args = env::args().collect::<Vec<_>>();

  if args.len() < 2 {
    print_usage(&args[0]);
    std::process::exit(1);
  }

  match args[1].as_str() {
    "build" => {
      let mut project_path =
        env::current_dir().expect("Failed to get current directory");
      let mut target = None;

      // Parse command line arguments
      let mut i = 2;
      while i < args.len() {
        match args[i].as_str() {
          "--target" | "-t" => {
            if i + 1 < args.len() {
              target = Some(parse_target(&args[i + 1]));
              i += 2;
            } else {
              eprintln!("Error: --target requires a value");
              std::process::exit(1);
            }
          }
          path => {
            project_path = PathBuf::from(path);
            i += 1;
          }
        }
      }

      build_project(project_path, target);
    }
    "version" | "--version" | "-v" => {
      println!("fret {}", env!("CARGO_PKG_VERSION"));
    }
    "help" | "--help" | "-h" => {
      print_usage(&args[0]);
    }
    _ => {
      eprintln!("Unknown command: {}", args[1]);
      print_usage(&args[0]);
      std::process::exit(1);
    }
  }
}

fn build_project(project_path: PathBuf, target: Option<Target>) {
  println!("Building project at: {}", project_path.display());
  if let Some(t) = target {
    println!("Target: {:?}", t);
  }

  let start_time = Instant::now();
  let pipeline = Pipeline::simple_mode();

  match pipeline.execute_with_target(project_path, target) {
    Ok(binary_path) => {
      let total_time = start_time.elapsed();
      println!("\nBuild successful!");
      println!("Output: {}", binary_path.display());
      println!("Total time: {:.2}s", total_time.as_secs_f64());
    }
    Err(e) => {
      eprintln!("\nBuild failed: {}", e);
      std::process::exit(1);
    }
  }
}

fn parse_target(target_str: &str) -> Target {
  match target_str.to_lowercase().as_str() {
    "x86_64-linux" => Target::X86_64Linux,
    "x86_64-macos" => Target::X86_64MacOS,
    "x86_64-windows" => Target::X86_64Windows,
    "aarch64-linux" => Target::Aarch64Linux,
    "aarch64-macos" => Target::Aarch64MacOS,
    "native" => Target::current(),
    _ => {
      eprintln!("Unknown target: {}", target_str);
      eprintln!("Available targets:");
      eprintln!("  native         - Current platform");
      eprintln!("  x86_64-linux   - Linux x86_64");
      eprintln!("  x86_64-macos   - macOS x86_64");
      eprintln!("  x86_64-windows - Windows x86_64");
      eprintln!("  aarch64-linux  - Linux ARM64");
      eprintln!("  aarch64-macos  - macOS ARM64 (Apple Silicon)");
      eprintln!("  python         - Python source code");
      eprintln!("  wasm           - WebAssembly");
      eprintln!("  llvm           - LLVM IR");
      std::process::exit(1);
    }
  }
}

fn print_usage(program: &str) {
  println!("fret - The blazing fast package manager for zo");
  println!();
  println!("USAGE:");
  println!("    {} <COMMAND> [OPTIONS]", program);
  println!();
  println!("COMMANDS:");
  println!("    build [OPTIONS] [PATH]    Build a zo project");
  println!("    version                   Show version information");
  println!("    help                      Show this help message");
  println!();
  println!("OPTIONS:");
  println!(
    "    -t, --target <TARGET>     Compilation target (default: native)"
  );
  println!();
  println!("TARGETS:");
  println!("    native         Current platform");
  println!("    x86_64-linux   Linux x86_64");
  println!("    x86_64-macos   macOS x86_64");
  println!("    x86_64-windows Windows x86_64");
  println!("    aarch64-linux  Linux ARM64");
  println!("    aarch64-macos  macOS ARM64 (Apple Silicon)");
  println!("    python         Python source code");
  println!("    wasm           WebAssembly");
  println!("    llvm           LLVM IR");
  println!();
  println!("EXAMPLES:");
  println!("    {} build", program);
  println!("    {} build --target wasm", program);
  println!("    {} build ./my-project --target python", program);
}
