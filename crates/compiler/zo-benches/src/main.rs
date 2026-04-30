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

/// Minimum absolute difference (ns) to trigger regression.
/// 500 µs ignores noise at sub-millisecond timescales without
/// burying real perf wins on the fastest benches.
const REGRESSION_MIN_DIFF_NS: u64 = 500_000;

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

/// Stored baseline entry for a single benchmark. Hot
/// average (first run dropped) in nanoseconds — fine
/// granularity matters because `hello` finishes in
/// hundreds of µs; storing in ms used to round it to 0.
#[derive(Debug, Serialize, Deserialize)]
struct Baseline {
  zo_hot_avg_ns: u64,
}

/// Pretty-print a wall-clock duration in nanoseconds at the
/// largest unit that keeps three significant figures.
///   < 1 µs   → "Xns"
///   < 1 ms   → "X.XXµs"
///   < 1 s    → "X.XXms"
///   ≥ 1 s    → "X.XXs"
fn fmt_dur(ns: u64) -> String {
  if ns < 1_000 {
    format!("{ns}ns")
  } else if ns < 1_000_000 {
    format!("{:.2}µs", ns as f64 / 1_000.0)
  } else if ns < 1_000_000_000 {
    format!("{:.2}ms", ns as f64 / 1_000_000.0)
  } else {
    format!("{:.2}s", ns as f64 / 1_000_000_000.0)
  }
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

    for (name, current_ns) in &results {
      if let Some(baseline) = baselines.get(name.as_str()) {
        let baseline_ns = baseline.zo_hot_avg_ns;
        let diff_pct = if baseline_ns > 0 {
          (*current_ns as f64 - baseline_ns as f64) / baseline_ns as f64
        } else {
          0.0
        };

        let abs_diff = (*current_ns as i64 - baseline_ns as i64).unsigned_abs();

        let status = if diff_pct > REGRESSION_THRESHOLD
          && abs_diff >= REGRESSION_MIN_DIFF_NS
        {
          regressions.push(name.clone());
          "REGRESSION"
        } else if diff_pct < -REGRESSION_THRESHOLD
          && abs_diff >= REGRESSION_MIN_DIFF_NS
        {
          "faster"
        } else {
          "ok"
        };

        println!(
          "  {name}: {current} \
           (baseline: {baseline}, \
           {diff_pct:+.0}%) [{status}]",
          current = fmt_dur(*current_ns),
          baseline = fmt_dur(baseline_ns),
        );
      } else {
        println!("  {name}: {} (no baseline)", fmt_dur(*current_ns));
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

/// Run a benchmark. Returns zo hot average (ns) if available.
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

    let elapsed = start.elapsed().as_nanos() as u64;

    match result {
      Ok(_) => {
        times.push(elapsed);
        println!("Run {i}: {}", fmt_dur(elapsed));
      }
      Err(_) => println!("Run {i}: FAILED"),
    }
  }

  if !times.is_empty() {
    let avg = (times.iter().sum::<u64>()) / times.len() as u64;

    println!("Average: {}", fmt_dur(avg));
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

    let elapsed = start.elapsed().as_nanos() as u64;

    match result {
      Ok(_) => {
        times.push(elapsed);
        println!("Run {i}: {}", fmt_dur(elapsed));
      }
      Err(_) => println!("Run {i}: FAILED"),
    }
  }

  if !times.is_empty() {
    let avg = (times.iter().sum::<u64>()) / times.len() as u64;

    println!("Average: {}", fmt_dur(avg));
  }

  println!();
}

/// Returns the hot average (excluding first run) in ns.
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

    let elapsed = start.elapsed().as_nanos() as u64;

    match result {
      Ok(_) => {
        times.push(elapsed);
        println!("Run {i}: {}", fmt_dur(elapsed));
      }
      Err(_) => println!("Run {i}: FAILED"),
    }
  }

  if !times.is_empty() {
    let avg = (times.iter().sum::<u64>()) / times.len() as u64;

    println!("Average: {}", fmt_dur(avg));
  }

  // Hot average: exclude first run (cold cache).
  let hot_avg = if times.len() > 1 {
    let hot: Vec<_> = times[1..].to_vec();
    let sum: u64 = hot.iter().sum();

    Some(sum / hot.len() as u64)
  } else {
    times.first().copied()
  };

  if let Some(hot) = hot_avg {
    println!("Hot avg: {}", fmt_dur(hot));
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
    .map(|(name, ns)| (name.clone(), Baseline { zo_hot_avg_ns: *ns }))
    .collect();

  let json = serde_json::to_string_pretty(&baselines).unwrap();

  let _ = fs::write(path, json + "\n");
}
