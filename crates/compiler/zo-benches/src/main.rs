use clap::Parser;
use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
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
  about = "zo compiler benchmark runner",
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
  /// Also measure runtime of the compiled binary (run it
  /// `runs` times and report the average). Off by default
  /// because some benches (e.g. munchhausen) do hundreds
  /// of millions of iterations and turn a 30s sweep into
  /// several minutes.
  #[arg(long)]
  with_runtime: bool,
  /// Argv passed to the compiled binary at runtime.
  /// Whitespace-split into individual args. Only meaningful
  /// when paired with `--with-runtime` — programs that read
  /// `args()` (e.g. `concurrency_pike_sieve N`) use this to
  /// drive scale sweeps without hardcoding N.
  #[arg(long, default_value = "")]
  argv: String,
  /// Sample peak RSS (KiB) during each runtime invocation
  /// and report it alongside wall time. Uses `ps -o rss=`
  /// polled every 50 ms — cross-platform, no syscall deps.
  /// Honored only when `--with-runtime` is set.
  #[arg(long)]
  with_rss: bool,
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

/// Hot average: mean wall time excluding the cold first run —
/// the warm steady state. Falls back to the sole sample when
/// there is only one run; `None` when no run succeeded.
fn hot_avg(times: &[u64]) -> Option<u64> {
  match times.len() {
    0 => None,
    1 => times.first().copied(),
    n => Some(times[1..].iter().sum::<u64>() / (n - 1) as u64),
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

        if let Some(avg) = run_bench(
          name,
          runs,
          cli.quick,
          cli.with_runtime,
          &cli.argv,
          cli.with_rss,
        ) {
          results.insert(name.clone(), avg);
        }
      }
    }
  } else if let Some(avg) = run_bench(
    &cli.benchmark,
    runs,
    cli.quick,
    cli.with_runtime,
    &cli.argv,
    cli.with_rss,
  ) {
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
          diff_pct = diff_pct * 100.0,
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

  // Always sweep stale dylibs from every `<bench>/bin/`.
  // zo build emits `LC_LOAD_DYLIB @executable_path/lib*.dylib`
  // for the concurrency/UI/provider runtimes; earlier
  // harness iterations dropped copies next to the bin and
  // they accumulated across runs. Clean unconditionally so
  // git status stays empty after a sweep.
  cleanup_dylibs(&bench_dir);

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
fn run_bench(
  name: &str,
  num_runs: usize,
  quick: bool,
  with_runtime: bool,
  argv: &str,
  with_rss: bool,
) -> Option<u64> {
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
  let go_file = bench_dir.join(format!("{name}.go"));
  let rs_file = bench_dir.join(format!("{name}.rs"));
  let odin_file = bench_dir.join(format!("{name}.odin"));
  let gleam_file = bench_dir.join(format!("{name}.gleam"));
  let zo_file = bench_dir.join(format!("{name}.zo"));

  let bin_dir = bench_dir.join("bin");
  let _ = fs::create_dir_all(&bin_dir);

  if !quick {
    if c_file.exists() {
      benchmark_c(
        &c_file,
        &bin_dir.join(format!("{name}_c")),
        num_runs,
        with_runtime,
        argv,
        with_rss,
      );
    }

    if go_file.exists() {
      benchmark_go(
        &go_file,
        &bin_dir.join(format!("{name}_go")),
        num_runs,
        with_runtime,
        argv,
        with_rss,
      );
    }

    if rs_file.exists() {
      benchmark_rust(
        &rs_file,
        &bin_dir.join(format!("{name}_rust")),
        num_runs,
        with_runtime,
        argv,
        with_rss,
      );
    }

    if odin_file.exists() {
      benchmark_odin(
        &odin_file,
        &bin_dir.join(format!("{name}_odin")),
        num_runs,
        with_runtime,
        argv,
        with_rss,
      );
    }

    if gleam_file.exists() {
      benchmark_gleam(
        &gleam_file,
        &bin_dir.join(format!("{name}_gleam")),
        num_runs,
        with_runtime,
      );
    }
  }

  if zo_file.exists() {
    benchmark_zo(
      &zo_file,
      &bin_dir.join(format!("{name}_zo")),
      num_runs,
      with_runtime,
      argv,
      with_rss,
    )
  } else {
    None
  }
}

/// Time `runs` invocations of `binary` and print a
/// per-run + average breakdown. Stdout/stderr of the
/// program are silenced to keep the bench output tidy.
///
/// `argv` is whitespace-split and passed to each
/// invocation — empty string = no args.
///
/// When `with_rss` is set, a background thread polls
/// `ps -o rss= -p <pid>` every 50 ms and records the
/// peak. Reported as `Runtime peak RSS: X MiB`
/// alongside the wall time. Cross-platform via POSIX
/// `ps`; no syscall deps. The 50 ms cadence is a
/// pragmatic tradeoff — fast-finishing benches may
/// undersample, but for the runtime/concurrency
/// targets (which take ≥ 100 ms even at N=10k) it
/// captures the peak with one to two orders of
/// magnitude headroom.
fn time_runtime(binary: &PathBuf, runs: usize, argv: &str, with_rss: bool) {
  let mut times = Vec::new();
  let mut peaks: Vec<u64> = Vec::new();
  let extra_args: Vec<&str> = if argv.is_empty() {
    Vec::new()
  } else {
    argv.split_whitespace().collect()
  };

  for i in 1..=runs {
    let mut cmd = Command::new(binary);

    cmd
      .args(&extra_args)
      .stdout(std::process::Stdio::null())
      .stderr(std::process::Stdio::null());

    let start = Instant::now();

    if !with_rss {
      let result = cmd.output();
      let elapsed = start.elapsed().as_nanos() as u64;

      match result {
        Ok(o) if o.status.success() => {
          times.push(elapsed);
          println!("  Runtime {i}: {}", fmt_dur(elapsed));
        }
        _ => println!("  Runtime {i}: FAILED"),
      }

      continue;
    }

    // RSS-sampling path: spawn so we have a PID to poll.
    let mut child = match cmd.spawn() {
      Ok(c) => c,
      Err(_) => {
        println!("  Runtime {i}: FAILED");
        continue;
      }
    };

    let pid = child.id();
    let peak = sample_peak_rss_kib(pid);

    let status = child.wait();
    let elapsed = start.elapsed().as_nanos() as u64;

    let peak_kib = peak.recv().unwrap_or(0);

    match status {
      Ok(s) if s.success() => {
        times.push(elapsed);
        peaks.push(peak_kib);

        println!(
          "  Runtime {i}: {} (peak RSS: {})",
          fmt_dur(elapsed),
          fmt_kib(peak_kib),
        );
      }
      _ => {
        peaks.push(peak_kib);

        println!("  Runtime {i}: FAILED (peak RSS: {})", fmt_kib(peak_kib),);
      }
    }
  }

  if !times.is_empty() {
    let avg = times.iter().sum::<u64>() / times.len() as u64;
    println!("  Runtime avg: {}", fmt_dur(avg));

    // The cold first run is dyld + page-in warmup, not steady
    // state. Exclude it for the warm number, matching the
    // compile-side `Hot avg` and the README's warm runtime table.
    if let Some(hot) = hot_avg(&times) {
      println!("  Runtime hot avg: {}", fmt_dur(hot));
    }
  }

  if !peaks.is_empty() {
    let avg_peak = peaks.iter().sum::<u64>() / peaks.len() as u64;
    let max_peak = peaks.iter().copied().max().unwrap_or(0);

    println!(
      "  Runtime peak RSS: avg {}, max {}",
      fmt_kib(avg_peak),
      fmt_kib(max_peak),
    );
  }
}

/// Pretty-print a `KiB` count at the largest unit that
/// keeps three significant figures.
fn fmt_kib(kib: u64) -> String {
  if kib < 1_024 {
    format!("{kib} KiB")
  } else if kib < 1_024 * 1_024 {
    format!("{:.2} MiB", kib as f64 / 1_024.0)
  } else {
    format!("{:.2} GiB", kib as f64 / (1_024.0 * 1_024.0))
  }
}

/// Background-poll `ps -o rss= -p <pid>` every 50 ms and
/// send the running peak (in KiB) back when the child
/// exits. Reading once after wait would only see RSS at
/// exit time — most green-task workloads peak mid-run.
///
/// Returns a `mpsc::Receiver` that yields the peak after
/// the spawned thread observes `ps` returning empty (the
/// child has exited and its PID is gone).
fn sample_peak_rss_kib(pid: u32) -> std::sync::mpsc::Receiver<u64> {
  use std::sync::mpsc;
  use std::thread;
  use std::time::Duration;

  let (tx, rx) = mpsc::channel();

  thread::spawn(move || {
    let mut peak: u64 = 0;

    loop {
      let out = Command::new("ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output();

      let rss = match out {
        Ok(o) if o.status.success() => {
          let s = String::from_utf8_lossy(&o.stdout).trim().to_owned();

          // Empty stdout ⇒ PID gone ⇒ child exited.
          if s.is_empty() {
            break;
          }

          s.parse::<u64>().unwrap_or(0)
        }
        _ => break,
      };

      if rss > peak {
        peak = rss;
      }

      thread::sleep(Duration::from_millis(50));
    }

    let _ = tx.send(peak);
  });

  rx
}

fn benchmark_c(
  source: &PathBuf,
  output: &PathBuf,
  runs: usize,
  with_runtime: bool,
  argv: &str,
  with_rss: bool,
) {
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
      Ok(output) if output.status.success() => {
        times.push(elapsed);
        println!("Run {i}: {}", fmt_dur(elapsed));
      }
      _ => println!("Run {i}: FAILED"),
    }
  }

  if !times.is_empty() {
    let avg = (times.iter().sum::<u64>()) / times.len() as u64;

    println!("Average: {}", fmt_dur(avg));
  }

  if let Some(hot) = hot_avg(&times) {
    println!("Hot avg: {}", fmt_dur(hot));
  }

  if with_runtime && output.exists() {
    time_runtime(output, runs, argv, with_rss);
  }

  println!();
}

fn benchmark_go(
  source: &PathBuf,
  output: &PathBuf,
  runs: usize,
  with_runtime: bool,
  argv: &str,
  with_rss: bool,
) {
  println!("go (ARM64):");

  let lines = count_lines(source).unwrap_or(0);
  let filename = source.file_name().unwrap().to_string_lossy();

  println!("Compiling {filename} — {lines} lines.");

  let mut times = Vec::new();

  for i in 1..=runs {
    let _ = fs::remove_file(output);

    let start = Instant::now();
    let result = Command::new("go")
      .arg("build")
      .arg("-o")
      .arg(output)
      .arg(source)
      .output();
    let elapsed = start.elapsed().as_nanos() as u64;

    match result {
      Ok(output) if output.status.success() => {
        times.push(elapsed);
        println!("Run {i}: {}", fmt_dur(elapsed));
      }
      _ => println!("Run {i}: FAILED"),
    }
  }

  if !times.is_empty() {
    let avg = (times.iter().sum::<u64>()) / times.len() as u64;

    println!("Average: {}", fmt_dur(avg));
  }

  if let Some(hot) = hot_avg(&times) {
    println!("Hot avg: {}", fmt_dur(hot));
  }

  if with_runtime && output.exists() {
    time_runtime(output, runs, argv, with_rss);
  }

  println!();
}

fn benchmark_odin(
  source: &PathBuf,
  output: &PathBuf,
  runs: usize,
  with_runtime: bool,
  argv: &str,
  with_rss: bool,
) {
  println!("odin (ARM64):");

  let lines = count_lines(source).unwrap_or(0);
  let filename = source.file_name().unwrap().to_string_lossy();

  println!("Compiling {filename} — {lines} lines.");

  let mut times = Vec::new();

  for i in 1..=runs {
    let _ = fs::remove_file(output);

    let start = Instant::now();
    let result = Command::new("odin")
      .arg("build")
      .arg(source)
      .arg("-file")
      .arg(format!("-out:{}", output.display()))
      .output();
    let elapsed = start.elapsed().as_nanos() as u64;

    match result {
      Ok(output) if output.status.success() => {
        times.push(elapsed);
        println!("Run {i}: {}", fmt_dur(elapsed));
      }
      _ => println!("Run {i}: FAILED"),
    }
  }

  if !times.is_empty() {
    let avg = (times.iter().sum::<u64>()) / times.len() as u64;

    println!("Average: {}", fmt_dur(avg));
  }

  if let Some(hot) = hot_avg(&times) {
    println!("Hot avg: {}", fmt_dur(hot));
  }

  if with_runtime && output.exists() {
    time_runtime(output, runs, argv, with_rss);
  }

  println!();
}

fn benchmark_rust(
  source: &PathBuf,
  output: &PathBuf,
  runs: usize,
  with_runtime: bool,
  argv: &str,
  with_rss: bool,
) {
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
      Ok(output) if output.status.success() => {
        times.push(elapsed);
        println!("Run {i}: {}", fmt_dur(elapsed));
      }
      _ => println!("Run {i}: FAILED"),
    }
  }

  if !times.is_empty() {
    let avg = (times.iter().sum::<u64>()) / times.len() as u64;

    println!("Average: {}", fmt_dur(avg));
  }

  if let Some(hot) = hot_avg(&times) {
    println!("Hot avg: {}", fmt_dur(hot));
  }

  if with_runtime && output.exists() {
    time_runtime(output, runs, argv, with_rss);
  }

  println!();
}

/// Returns the hot average (excluding first run) in ns.
fn benchmark_zo(
  source: &PathBuf,
  output: &PathBuf,
  runs: usize,
  with_runtime: bool,
  argv: &str,
  with_rss: bool,
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
      Ok(output) if output.status.success() => {
        times.push(elapsed);
        println!("Run {i}: {}", fmt_dur(elapsed));
      }
      _ => println!("Run {i}: FAILED"),
    }
  }

  if !times.is_empty() {
    let avg = (times.iter().sum::<u64>()) / times.len() as u64;
    println!("Average: {}", fmt_dur(avg));
  }

  let hot = hot_avg(&times);

  if let Some(h) = hot {
    println!("Hot avg: {}", fmt_dur(h));
  }

  if with_runtime && output.exists() {
    time_runtime(output, runs, argv, with_rss);
  }

  println!();

  hot
}

/// Benchmark a Gleam program on the BEAM target.
///
/// Gleam compiles a *project*, not a bare file, so we scaffold a
/// throwaway project under `bin/<name>_gleam/` and reuse its
/// `build/` (downloaded + compiled stdlib) across runs. Each
/// timed run deletes only our module's compiled output and runs
/// `gleam build` — recompiling our module against the prebuilt
/// stdlib, the analog of clang compiling against a prebuilt libc.
fn benchmark_gleam(
  source: &PathBuf,
  proj_dir: &Path,
  runs: usize,
  with_runtime: bool,
) {
  println!("gleam (BEAM):");

  let lines = count_lines(source).unwrap_or(0);
  let filename = source.file_name().unwrap().to_string_lossy();

  println!("Compiling {filename} — {lines} lines.");

  // Module names forbid hyphens — `n-body` → module `n_body`.
  let module = source
    .file_stem()
    .unwrap()
    .to_string_lossy()
    .replace('-', "_");

  if scaffold_gleam_project(proj_dir, &module, source).is_err() {
    println!("Run 1: FAILED (scaffold)");
    println!();

    return;
  }

  // Warm-up: resolve + download + compile deps and our module
  // once, untimed. The timed loop measures only our module.
  let warm = Command::new("gleam")
    .arg("build")
    .current_dir(proj_dir)
    .output();

  if !matches!(&warm, Ok(o) if o.status.success()) {
    println!("Run 1: FAILED (build)");

    if let Ok(o) = &warm {
      eprint!("{}", String::from_utf8_lossy(&o.stderr));
    }

    println!();

    return;
  }

  let module_build = proj_dir.join("build/dev/erlang").join(&module);

  let mut times = Vec::new();

  for i in 1..=runs {
    let _ = fs::remove_dir_all(&module_build);

    let start = Instant::now();
    let result = Command::new("gleam")
      .arg("build")
      .current_dir(proj_dir)
      .output();
    let elapsed = start.elapsed().as_nanos() as u64;

    match result {
      Ok(o) if o.status.success() => {
        times.push(elapsed);
        println!("Run {i}: {}", fmt_dur(elapsed));
      }
      _ => println!("Run {i}: FAILED"),
    }
  }

  if !times.is_empty() {
    let avg = times.iter().sum::<u64>() / times.len() as u64;

    println!("Average: {}", fmt_dur(avg));
  }

  if let Some(hot) = hot_avg(&times) {
    println!("Hot avg: {}", fmt_dur(hot));
  }

  if with_runtime {
    time_gleam_runtime(proj_dir, &module, runs);
  }

  println!();
}

/// Write a minimal Gleam project (`gleam.toml` + `src/<module>.
/// gleam`) into `proj_dir`. `gleam_erlang` is pulled in for the
/// concurrency ports (`threadring`); numeric ports ignore it.
fn scaffold_gleam_project(
  proj_dir: &Path,
  module: &str,
  source: &PathBuf,
) -> std::io::Result<()> {
  let src_dir = proj_dir.join("src");

  fs::create_dir_all(&src_dir)?;

  let manifest = format!(
    "name = \"{module}\"\n\
     version = \"1.0.0\"\n\
     target = \"erlang\"\n\n\
     [dependencies]\n\
     gleam_stdlib = \">= 0.34.0 and < 2.0.0\"\n\
     gleam_erlang = \">= 0.25.0 and < 2.0.0\"\n"
  );

  fs::write(proj_dir.join("gleam.toml"), manifest)?;
  fs::copy(source, src_dir.join(format!("{module}.gleam")))?;

  Ok(())
}

/// Time `runs` direct BEAM executions of a compiled Gleam
/// module via `erl`, bypassing `gleam run` so its build-freshness
/// check doesn't leak into the runtime number. The wall time is
/// dominated by BEAM VM startup — the honest cost of running any
/// BEAM program, analogous to a dynamic-runtime startup floor.
fn time_gleam_runtime(proj_dir: &Path, module: &str, runs: usize) {
  let erlang_dir = proj_dir.join("build/dev/erlang");
  let mut pa_args: Vec<String> = Vec::new();

  if let Ok(entries) = fs::read_dir(&erlang_dir) {
    for entry in entries.flatten() {
      let ebin = entry.path().join("ebin");

      if ebin.is_dir() {
        pa_args.push("-pa".to_string());
        pa_args.push(ebin.to_string_lossy().into_owned());
      }
    }
  }

  let mut times = Vec::new();

  for i in 1..=runs {
    let start = Instant::now();
    let result = Command::new("erl")
      .args(&pa_args)
      .arg("-noshell")
      .arg("-eval")
      .arg(format!("{module}:main()"))
      .arg("-eval")
      .arg("init:stop()")
      .stdout(std::process::Stdio::null())
      .stderr(std::process::Stdio::null())
      .output();
    let elapsed = start.elapsed().as_nanos() as u64;

    match result {
      Ok(o) if o.status.success() => {
        times.push(elapsed);
        println!("  Runtime {i}: {}", fmt_dur(elapsed));
      }
      _ => println!("  Runtime {i}: FAILED"),
    }
  }

  if !times.is_empty() {
    let avg = times.iter().sum::<u64>() / times.len() as u64;

    println!("  Runtime avg: {}", fmt_dur(avg));

    if let Some(hot) = hot_avg(&times) {
      println!("  Runtime hot avg: {}", fmt_dur(hot));
    }
  }
}

fn count_lines(path: &PathBuf) -> std::io::Result<usize> {
  let content = fs::read_to_string(path)?;
  Ok(content.lines().count())
}

/// Sweep `*.dylib` files out of every `<bench>/bin/`.
/// Compiled zo programs reference runtime dylibs via
/// `@executable_path/lib*.dylib`; when those files end up
/// in `bin/` (older harness behavior / stray copies)
/// they pollute `git status` and snowball across runs.
fn cleanup_dylibs(bench_dir: &PathBuf) {
  let Ok(entries) = fs::read_dir(bench_dir) else {
    return;
  };

  for entry in entries.flatten() {
    let bin = entry.path().join("bin");

    if !bin.is_dir() {
      continue;
    }

    let Ok(files) = fs::read_dir(&bin) else {
      continue;
    };

    for f in files.flatten() {
      let path = f.path();

      if path.extension().is_some_and(|e| e == "dylib") {
        let _ = fs::remove_file(&path);
      }
    }
  }
}

// ================================================================
// Baseline persistence (serde).
// ================================================================

fn load_baseline(path: &PathBuf) -> BTreeMap<String, Baseline> {
  // A missing file is normal (first run) — no baseline yet.
  let Ok(content) = fs::read_to_string(path) else {
    return BTreeMap::new();
  };

  // A file that exists but won't parse is NOT normal: silently returning empty
  // disables every regression check with no trace (how a stale `zo_hot_avg_ms`
  // baseline went unnoticed after the ns migration). Warn loudly and point at
  // the fix.
  match serde_json::from_str(&content) {
    Ok(baselines) => baselines,
    Err(error) => {
      eprintln!(
        "warning: baseline {} failed to parse ({error}); \
         regression checks are off — regenerate with \
         `--update-baseline`",
        path.display(),
      );

      BTreeMap::new()
    }
  }
}

fn save_baseline(path: &PathBuf, results: &BTreeMap<String, u64>) {
  // Merge into the existing baselines: updating one benchmark
  // (`--update-baseline mandelbrot`) must not drop the others.
  let mut baselines = load_baseline(path);

  for (name, ns) in results {
    baselines.insert(name.clone(), Baseline { zo_hot_avg_ns: *ns });
  }

  let json = serde_json::to_string_pretty(&baselines).unwrap();
  let _ = fs::write(path, json + "\n");
}
