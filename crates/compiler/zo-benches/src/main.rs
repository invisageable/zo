use clap::Parser;
use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

/// Regression threshold: fail if hot average exceeds baseline
/// by this fraction. 10% allows for hardware variance.
const REGRESSION_THRESHOLD: f64 = 0.10;

/// Minimum absolute difference (ms) to trigger regression.
/// Ignores noise at sub-millisecond timescales.
const REGRESSION_MIN_DIFF_MS: u64 = 2;

#[derive(Parser)]
#[command(name = "bench")]
#[command(
  about = "Compiler benchmark runner",
  long_about = None
)]
struct Cli {
  /// The name of the benchmark to run (or "all").
  benchmark: String,
  /// The number of runs per benchmark.
  #[arg(short, long, default_value = "5")]
  runs: usize,
  /// Update the baseline file with current results.
  #[arg(long)]
  update_baseline: bool,
  /// Strict mode: exit with error on regression.
  #[arg(long)]
  strict: bool,
  /// Quick mode: 3 runs, zo only, for pre-commit hook.
  #[arg(long)]
  quick: bool,
}

/// Stored baseline entry for a single benchmark.
#[derive(Debug, Serialize, Deserialize)]
struct Baseline {
  /// Hot average (excluding first run) in ms.
  zo_hot_avg_ms: u64,
}

fn main() {
  let cli = Cli::parse();
  let bench_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("benches");
  let baseline_path =
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("baseline.json");

  let runs = if cli.quick { 3 } else { cli.runs };
  let mut results: BTreeMap<String, u64> = BTreeMap::new();
  let mut regressions = Vec::new();

  let baselines = load_baseline(&baseline_path);

  if cli.benchmark == "all" {
    if let Ok(entries) = fs::read_dir(&bench_dir) {
      let mut names: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_str().map(String::from))
        .collect();

      names.sort();

      for name in &names {
        println!("\nRunning {name} benchmark...\n");

        if let Some(avg) = run_bench(name, runs, cli.quick) {
          results.insert(name.clone(), avg);
        }
      }
    }
  } else if let Some(avg) = run_bench(&cli.benchmark, runs, cli.quick) {
    results.insert(cli.benchmark.clone(), avg);
  }

  // Compare against baseline.
  if !baselines.is_empty() && !cli.update_baseline {
    println!("\n── baseline comparison ──\n");

    for (name, current_ms) in &results {
      if let Some(baseline) = baselines.get(name.as_str()) {
        let baseline_ms = baseline.zo_hot_avg_ms;
        let diff_pct = if baseline_ms > 0 {
          (*current_ms as f64 - baseline_ms as f64) / baseline_ms as f64
        } else {
          0.0
        };

        let abs_diff = (*current_ms as i64 - baseline_ms as i64).unsigned_abs();

        let status = if diff_pct > REGRESSION_THRESHOLD
          && abs_diff >= REGRESSION_MIN_DIFF_MS
        {
          regressions.push(name.clone());
          "REGRESSION"
        } else if diff_pct < -REGRESSION_THRESHOLD
          && abs_diff >= REGRESSION_MIN_DIFF_MS
        {
          "faster"
        } else {
          "ok"
        };

        println!(
          "  {name}: {current_ms}ms \
           (baseline: {baseline_ms}ms, \
           {diff_pct:+.0}%) [{status}]",
        );
      } else {
        println!("  {name}: {current_ms}ms (no baseline)");
      }
    }

    println!();
  }

  // Update baseline if requested.
  if cli.update_baseline {
    save_baseline(&baseline_path, &results);

    println!("baseline updated: {}", baseline_path.display());
  }

  // Strict mode: exit with error on regression.
  if cli.strict && !regressions.is_empty() {
    eprintln!("REGRESSION detected in: {}", regressions.join(", "));
    eprintln!(
      "threshold: {:.0}%. \
       run with --update-baseline to accept.",
      REGRESSION_THRESHOLD * 100.0
    );

    std::process::exit(1);
  }
}

/// Run a benchmark. Returns zo hot average (ms) if available.
fn run_bench(name: &str, num_runs: usize, quick: bool) -> Option<u64> {
  let bench_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("benches")
    .join(name);

  if !bench_dir.exists() {
    eprintln!("Benchmark '{name}' not found in benches/");

    if let Ok(entries) =
      fs::read_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("benches"))
    {
      eprintln!("Available benchmarks:");

      for entry in entries.flatten() {
        if entry.path().is_dir()
          && let Some(name) = entry.file_name().to_str()
        {
          eprintln!("  - {}", name);
        }
      }
    }

    return None;
  }

  let c_file = bench_dir.join(format!("{name}.c"));
  let rs_file = bench_dir.join(format!("{name}.rs"));
  let zo_file = bench_dir.join(format!("{name}.zo"));

  let bin_dir = bench_dir.join("bin");
  let _ = fs::create_dir_all(&bin_dir);

  if !quick {
    if c_file.exists() {
      benchmark_c(&c_file, &bin_dir.join(format!("{name}_c")), num_runs);
    }

    if rs_file.exists() {
      benchmark_rust(&rs_file, &bin_dir.join(format!("{name}_rust")), num_runs);
    }
  }

  if zo_file.exists() {
    benchmark_zo(&zo_file, &bin_dir.join(format!("{name}_zo")), num_runs)
  } else {
    None
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

  println!("Compiling {filename} — {lines} lines.");

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

/// Returns the hot average (excluding first run) in ms.
fn benchmark_zo(
  source: &PathBuf,
  output: &PathBuf,
  runs: usize,
) -> Option<u64> {
  println!("zo (ARM64):");

  let lines = count_lines(source).unwrap_or(0);
  let filename = source.file_name().unwrap().to_string_lossy();

  println!("Compiling {filename} — {lines} lines.");

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

  // Hot average: exclude first run (cold cache).
  let hot_avg = if times.len() > 1 {
    let hot: Vec<_> = times[1..].to_vec();
    let sum: u128 = hot.iter().sum();

    Some((sum / hot.len() as u128) as u64)
  } else if times.len() == 1 {
    Some(times[0] as u64)
  } else {
    None
  };

  if let Some(hot) = hot_avg {
    println!("Hot avg: {hot}ms");
  }

  println!();

  hot_avg
}

fn count_lines(path: &PathBuf) -> std::io::Result<usize> {
  let content = fs::read_to_string(path)?;

  Ok(content.lines().count())
}

// ================================================================
// Baseline persistence (serde).
// ================================================================

fn load_baseline(path: &PathBuf) -> BTreeMap<String, Baseline> {
  let content = match fs::read_to_string(path) {
    Ok(c) => c,
    Err(_) => return BTreeMap::new(),
  };

  serde_json::from_str(&content).unwrap_or_default()
}

fn save_baseline(path: &PathBuf, results: &BTreeMap<String, u64>) {
  let baselines: BTreeMap<String, Baseline> = results
    .iter()
    .map(|(name, ms)| (name.clone(), Baseline { zo_hot_avg_ms: *ms }))
    .collect();

  let json = serde_json::to_string_pretty(&baselines).unwrap();

  let _ = fs::write(path, json + "\n");
}
