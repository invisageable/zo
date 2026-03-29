//! zo-test-runner — integration test runner for `.zo` programs.
//!
//! Compiles and runs every `.zo` test program, verifies exit
//! codes and expected output. Catches codegen regressions
//! that unit tests miss.
//!
//! Usage:
//!   cargo run --bin zo-test-runner
//!   cargo run --bin zo-test-runner -- --quick
//!   cargo run --bin zo-test-runner -- --filter arrays

use swisskit_core::fmt::ansi::strip_ansi;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

/// Test category — determines how to run and what to expect.
#[derive(Clone, Copy)]
enum Category {
  /// Build + run + optional output check.
  Pass,
  /// Must fail to compile. Check error output if present.
  Fail,
  /// Build only (no run). For ZSX/UI programs.
  BuildOnly,
}

struct TestResult {
  name: String,
  passed: bool,
  reason: String,
}

fn main() {
  let args = env::args().collect::<Vec<_>>();

  let filter = args
    .iter()
    .position(|a| a == "--filter")
    .and_then(|i| args.get(i + 1))
    .map(|s| s.as_str());

  let root = find_workspace_root();
  let zo = find_zo_binary(&root);
  let tests_dir = root.join("crates/compiler/zo-tests");
  let howto_dir = root.join("crates/compiler/zo-how-zo");

  println!("zo: {}", zo.display());
  let tmp =
    env::temp_dir().join(format!("zo-test-runner-{}", std::process::id()));

  fs::create_dir_all(&tmp).expect("failed to create temp dir");

  let start = Instant::now();
  let mut results: Vec<TestResult> = Vec::new();

  // programming/ — build + run + optional output check.
  run_dir(
    &tests_dir.join("programming"),
    Category::Pass,
    &zo,
    &tmp,
    filter,
    &mut results,
  );

  // programming/fail/ — must fail to compile.
  run_dir(
    &tests_dir.join("programming/fail"),
    Category::Fail,
    &zo,
    &tmp,
    filter,
    &mut results,
  );

  // templating/ — build only (ZSX renders to UI, no stdout).
  run_dir(
    &tests_dir.join("templating"),
    Category::BuildOnly,
    &zo,
    &tmp,
    filter,
    &mut results,
  );

  // templating/fail/ — must fail to compile.
  run_dir(
    &tests_dir.join("templating/fail"),
    Category::Fail,
    &zo,
    &tmp,
    filter,
    &mut results,
  );

  // zo-how-to tutorials — build + run + output check.
  if howto_dir.exists() {
    run_dir(&howto_dir, Category::Pass, &zo, &tmp, filter, &mut results);
  }

  // Cleanup.
  let _ = fs::remove_dir_all(&tmp);

  // Summary.
  let elapsed = start.elapsed();
  let passed = results.iter().filter(|r| r.passed).count();
  let failed = results.iter().filter(|r| !r.passed).count();
  let total = results.len();

  println!();
  println!("────────────────────────────────────");
  println!(
    "  {} passed, {} failed ({} total) in {:.2}s",
    passed,
    failed,
    total,
    elapsed.as_secs_f64()
  );

  if failed > 0 {
    println!();
    println!("failures:");

    for r in &results {
      if !r.passed {
        println!("  {} — {}", r.name, r.reason);
      }
    }

    println!();
    std::process::exit(1);
  }

  println!();
  println!("all zo program tests passed.");
}

fn run_dir(
  dir: &Path,
  category: Category,
  zo: &Path,
  tmp: &Path,
  filter: Option<&str>,
  results: &mut Vec<TestResult>,
) {
  if !dir.exists() {
    return;
  }

  let mut files = fs::read_dir(dir)
    .expect("failed to read dir")
    .filter_map(|e| e.ok())
    .map(|e| e.path())
    .filter(|p| p.extension().is_some_and(|e| e == "zo"))
    .collect::<Vec<_>>();

  files.sort();

  if files.is_empty() {
    return;
  }

  // For how-to, only pick numbered files (0*.zo).
  let is_howto = dir.to_string_lossy().contains("zo-how-zo");

  let files = if is_howto {
    files
      .into_iter()
      .filter(|p| {
        p.file_name()
          .and_then(|n| n.to_str())
          .is_some_and(|n| n.starts_with('0'))
      })
      .collect::<Vec<_>>()
  } else {
    files
  };

  if files.is_empty() {
    return;
  }

  let dir_name = dir
    .strip_prefix(dir.ancestors().nth(3).unwrap_or(dir))
    .unwrap_or(dir)
    .display();

  println!();
  println!("[{}] {} files", dir_name, files.len());

  for file in &files {
    let name = file.file_stem().and_then(|s| s.to_str()).unwrap_or("?");

    if let Some(f) = filter
      && !name.contains(f)
    {
      continue;
    }

    let result = run_test(file, name, category, zo, tmp);

    let icon = if result.passed {
      "\x1b[32mPASS\x1b[0m"
    } else {
      "\x1b[31mFAIL\x1b[0m"
    };

    if result.passed {
      println!("  {} {}", icon, name);
    } else {
      println!("  {} {} — {}", icon, name, result.reason);
    }

    results.push(result);
  }
}

fn run_test(
  file: &Path,
  name: &str,
  category: Category,
  zo: &Path,
  tmp: &Path,
) -> TestResult {
  let out = tmp.join(name);

  match category {
    Category::BuildOnly => {
      let status = Command::new(zo)
        .args(["build", &file.to_string_lossy(), "-o"])
        .arg(&out)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

      match status {
        Ok(s) if s.success() => ok(name),
        Ok(_) => fail(name, "compilation failed"),
        Err(e) => fail(name, &format!("exec error: {e}")),
      }
    }

    Category::Fail => {
      let output = Command::new(zo)
        .args(["build", &file.to_string_lossy(), "-o"])
        .arg(&out)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output();

      match output {
        Ok(o) if o.status.success() => {
          fail(name, "compilation succeeded (expected failure)")
        }
        Ok(o) => {
          let expected = extract_expected(file);

          if expected.is_empty() {
            return ok(name);
          }

          let stderr = strip_ansi(&String::from_utf8_lossy(&o.stderr));

          for line in expected.lines() {
            if !line.is_empty() && !stderr.contains(line) {
              return fail(
                name,
                &format!("missing in error output: '{}'", line),
              );
            }
          }

          // Strict: verify all errors are covered.
          let actual_errors =
            stderr.lines().filter(|l| l.contains("] Error:")).count();

          let expected_errors =
            expected.lines().filter(|l| l.contains("] Error:")).count();

          if actual_errors > expected_errors {
            return fail(
              name,
              &format!(
                "expected output covers {} error(s) \
                 but compiler produced {}",
                expected_errors, actual_errors
              ),
            );
          }

          ok(name)
        }
        Err(e) => fail(name, &format!("exec error: {e}")),
      }
    }

    Category::Pass => {
      let build = Command::new(zo)
        .args(["build", &file.to_string_lossy(), "-o"])
        .arg(&out)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

      match build {
        Ok(s) if !s.success() => {
          return fail(name, "compilation failed");
        }
        Err(e) => {
          return fail(name, &format!("build exec error: {e}"));
        }
        _ => {}
      }

      let run = Command::new(&out)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();

      match run {
        Ok(output) if !output.status.success() => fail(
          name,
          &format!(
            "runtime crash (exit {})",
            output.status.code().unwrap_or(-1)
          ),
        ),
        Ok(output) => {
          let actual = String::from_utf8_lossy(&output.stdout);
          let expected = extract_expected(file);

          if expected.is_empty() {
            return ok(name);
          }

          let actual_trimmed = actual.trim_end();
          let expected_trimmed = expected.trim_end();

          if actual_trimmed == expected_trimmed {
            ok(name)
          } else {
            let a_lines = actual_trimmed.lines().collect::<Vec<_>>();
            let e_lines = expected_trimmed.lines().collect::<Vec<_>>();

            for (i, (a, e)) in a_lines.iter().zip(e_lines.iter()).enumerate() {
              if a != e {
                return fail(
                  name,
                  &format!("line {}: expected '{}', got '{}'", i + 1, e, a),
                );
              }
            }

            if a_lines.len() != e_lines.len() {
              fail(
                name,
                &format!(
                  "expected {} lines, got {}",
                  e_lines.len(),
                  a_lines.len()
                ),
              )
            } else {
              fail(name, "output mismatch")
            }
          }
        }
        Err(e) => fail(name, &format!("run exec error: {e}")),
      }
    }
  }
}

/// Strip ANSI escape codes from a string.
/// Extract expected output from `-- EXPECTED OUTPUT:` marker.
fn extract_expected(file: &Path) -> String {
  let content = fs::read_to_string(file).unwrap_or_default();
  let mut lines = Vec::new();
  let mut in_expected = false;

  for line in content.lines() {
    if line.contains("EXPECTED OUTPUT:") {
      in_expected = true;
      continue;
    }

    if in_expected && let Some(rest) = line.strip_prefix("-- ") {
      // Strip trailing parenthetical comments and
      // any trailing whitespace.
      let trimmed = if let Some(pos) = rest.find("  (") {
        rest[..pos].trim_end()
      } else {
        rest.trim_end()
      };
      lines.push(trimmed.to_string());
    }
  }

  lines.join("\n")
}

fn ok(name: &str) -> TestResult {
  TestResult {
    name: name.to_string(),
    passed: true,
    reason: String::new(),
  }
}

fn fail(name: &str, reason: &str) -> TestResult {
  TestResult {
    name: name.to_string(),
    passed: false,
    reason: reason.to_string(),
  }
}

/// Find workspace root by walking up from the binary's
/// directory until we find Cargo.toml with [workspace].
fn find_workspace_root() -> PathBuf {
  let cwd = env::current_dir().expect("no cwd");
  let mut dir = cwd.as_path();

  loop {
    let cargo = dir.join("Cargo.toml");

    if cargo.exists()
      && let Ok(content) = fs::read_to_string(&cargo)
      && content.contains("[workspace]")
    {
      return dir.to_path_buf();
    }

    match dir.parent() {
      Some(parent) => dir = parent,
      None => return cwd,
    }
  }
}

/// Find the zo binary — prefer debug (freshest), fall back
/// to release.
fn find_zo_binary(root: &Path) -> PathBuf {
  let debug = root.join("target/debug/zo");

  if debug.exists() {
    return debug;
  }

  let release = root.join("target/release/zo");

  if release.exists() {
    return release;
  }

  // Try building.
  eprintln!("building zo...");

  let status = Command::new("cargo")
    .args(["build", "--bin", "zo"])
    .current_dir(root)
    .status()
    .expect("failed to run cargo build");

  if !status.success() {
    eprintln!("FATAL: failed to build zo binary");
    std::process::exit(1);
  }

  let debug = root.join("target/debug/zo");

  if debug.exists() {
    return debug;
  }

  eprintln!("FATAL: zo binary not found");
  std::process::exit(1);
}
