use clap::Parser;

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

#[derive(Parser)]
#[command(name = "bench")]
#[command(about = "Compiler benchmark runner", long_about = None)]
struct Cli {
  /// The name of the benchmark to run (or "all" to run all benchmarks).
  benchmark: String,
  /// The number of runs per benchmark.
  #[arg(short, long, default_value = "5")]
  runs: usize,
}

fn main() {
  let cli = Cli::parse();
  let bench_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("benches");

  if cli.benchmark == "all"
    && let Ok(entries) = fs::read_dir(&bench_dir)
  {
    for entry in entries.flatten() {
      if entry.path().is_dir()
        && let Some(name) = entry.file_name().to_str()
      {
        println!("\nRunning {name} benchmark...\n");
        run_bench(name, cli.runs);
      }
    }
  } else {
    run_bench(&cli.benchmark, cli.runs);
  }
}

fn run_bench(name: &str, num_runs: usize) {
  let bench_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("benches")
    .join(name);

  if !bench_dir.exists() {
    eprintln!("Benchmark '{name}' not found in benches/");
    eprintln!("Available benchmarks:");
    if let Ok(entries) =
      fs::read_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("benches"))
    {
      for entry in entries.flatten() {
        if entry.path().is_dir()
          && let Some(name) = entry.file_name().to_str()
        {
          eprintln!("  - {}", name);
        }
      }
    }
    return;
  }

  let c_file = bench_dir.join(format!("{name}.c"));
  let rs_file = bench_dir.join(format!("{name}.rs"));
  let zo_file = bench_dir.join(format!("{name}.zo"));

  let bin_dir = bench_dir.join("bin");
  let _ = fs::create_dir_all(&bin_dir);

  if c_file.exists() {
    benchmark_c(&c_file, &bin_dir.join(format!("{name}_c")), num_runs);
  }

  if rs_file.exists() {
    benchmark_rust(&rs_file, &bin_dir.join(format!("{name}_rust")), num_runs);
  }

  if zo_file.exists() {
    benchmark_zo(&zo_file, &bin_dir.join(format!("{name}_zo")), num_runs);
  }
}

fn benchmark_c(source: &PathBuf, output: &PathBuf, runs: usize) {
  println!("c (ARM64):");
  let lines = count_lines(source).unwrap_or(0);
  let filename = source.file_name().unwrap().to_string_lossy();
  println!("Compiling {filename} — {lines} lines.");

  let mut times = Vec::new();
  for i in 1..=runs {
    let _ = fs::remove_file(output);

    let start = Instant::now();
    let result = Command::new("clang")
      .arg("--target=arm64-apple-darwin")
      .arg(source)
      .arg("-o")
      .arg(output)
      .output();
    let elapsed = start.elapsed().as_millis();

    match result {
      Ok(_) => {
        times.push(elapsed);
        println!("Run {i}: {elapsed}ms");
      }
      Err(_) => println!("Run {i}: FAILED"),
    }
  }

  if !times.is_empty() {
    let avg = times.iter().sum::<u128>() / times.len() as u128;
    println!("Average: {avg}ms");
  }

  println!();
}

fn benchmark_rust(source: &PathBuf, output: &PathBuf, runs: usize) {
  println!("rustc (ARM64):");
  let lines = count_lines(source).unwrap_or(0);
  let filename = source.file_name().unwrap().to_string_lossy();
  println!("Compiling {filename} — {lines} lines.",);

  let mut times = Vec::new();
  for i in 1..=runs {
    let _ = fs::remove_file(output);

    let start = Instant::now();
    let result = Command::new("rustc")
      .arg("--target=aarch64-apple-darwin")
      .arg(source)
      .arg("-o")
      .arg(output)
      .output();
    let elapsed = start.elapsed().as_millis();

    match result {
      Ok(_) => {
        times.push(elapsed);
        println!("Run {i}: {elapsed}ms");
      }
      Err(_) => println!("Run {i}: FAILED"),
    }
  }

  if !times.is_empty() {
    let avg = times.iter().sum::<u128>() / times.len() as u128;
    println!("Average: {avg}ms");
  }
  println!();
}

fn benchmark_zo(source: &PathBuf, output: &PathBuf, runs: usize) {
  println!("zo (ARM64):");
  let lines = count_lines(source).unwrap_or(0);
  let filename = source.file_name().unwrap().to_string_lossy();
  println!("Compiling {filename} — {lines} lines.",);

  let zo_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .unwrap()
    .parent()
    .unwrap()
    .parent()
    .unwrap()
    .join("target/release/zo");

  let mut times = Vec::new();
  for i in 1..=runs {
    let _ = fs::remove_file(output);

    let start = Instant::now();
    let result = Command::new(&zo_path)
      .arg("build")
      .arg(source)
      .arg("-o")
      .arg(output)
      .output();
    let elapsed = start.elapsed().as_millis();

    match result {
      Ok(_) => {
        times.push(elapsed);
        println!("Run {i}: {elapsed}ms");
      }
      Err(_) => println!("Run {i}: FAILED"),
    }
  }

  if !times.is_empty() {
    let avg = times.iter().sum::<u128>() / times.len() as u128;
    println!("Average: {avg}ms");
  }
  println!();
}

fn count_lines(path: &PathBuf) -> std::io::Result<usize> {
  let content = fs::read_to_string(path)?;
  Ok(content.lines().count())
}
