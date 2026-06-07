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
//!   cargo run --bin zo-test-runner -- --all
//!
//! `--all` enables the `WindowRun` category — windowed
//! programs (raylib, misato, templating UI) build, spawn,
//! get killed after `WINDOW_KILL_AFTER`, and pass iff
//! they're still alive at kill time. Dev-machine only:
//! requires a display, so CI keeps the default mode.

use swisskit_core::fmt::ansi::strip_ansi;

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use std::env;
use std::fs;
use std::io;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Upper bound on the time a single test program is allowed
/// to run before we kill it and flag it as `runtime timeout`.
/// Set generously enough to cover slow Rosetta-translated
/// x86_64 binaries on arm64 hosts.
const RUN_TIMEOUT: Duration = Duration::from_secs(10);

// Runtime-flavor dylib names. Cargo always names its cdylib
// `libzo_runtime.{dylib,so}` (`RUNTIME_BUILT_DYLIB`); the
// runner copies each flavor's build aside under the lean /
// full names the driver stages from.
#[cfg(target_os = "macos")]
const RUNTIME_BUILT_DYLIB: &str = "libzo_runtime.dylib";
#[cfg(target_os = "macos")]
const RUNTIME_CORE_DYLIB: &str = "libzo_runtime_core.dylib";
#[cfg(target_os = "macos")]
const RUNTIME_UI_DYLIB: &str = "libzo_runtime_ui.dylib";

#[cfg(not(target_os = "macos"))]
const RUNTIME_BUILT_DYLIB: &str = "libzo_runtime.so";
#[cfg(not(target_os = "macos"))]
const RUNTIME_CORE_DYLIB: &str = "libzo_runtime_core.so";
#[cfg(not(target_os = "macos"))]
const RUNTIME_UI_DYLIB: &str = "libzo_runtime_ui.so";

/// How long a windowed program must stay alive before the
/// runner SIGKILLs it. "Still running at this point" is the
/// success signal — every windowed program is an infinite
/// frame loop, so an early exit means it crashed during init
/// (missing dylib, bad symbol, bogus Mach-O header, etc.).
const WINDOW_KILL_AFTER: Duration = Duration::from_secs(2);

/// Test category — determines how to run and what to expect.
#[derive(Clone, Copy)]
enum Category {
  /// Build + run + optional output check.
  Pass,
  /// Must fail to compile. Check error output if present.
  Fail,
  /// Build with `--emit`, verify `-- CHECK:` directives.
  Check,
  /// Build only, pass if no crash (signal death).
  Crash,
  /// Windowed program (raylib / misato / templating UI):
  /// in default mode this is build-only; with `--all` also
  /// spawns, sleeps `WINDOW_KILL_AFTER`, kills, and passes
  /// iff the process was still alive at kill time.
  WindowRun,
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

  // --target <triple> threads into every `zo build` invocation
  // below so the same suite can be measured against the CLIF
  // backend (x86_64-*, arm64-pc-windows-msvc) in addition to
  // the default ARM64 Darwin / Linux fast lane.
  let target = args
    .iter()
    .position(|a| a == "--target" || a == "-t")
    .and_then(|i| args.get(i + 1))
    .map(|s| s.as_str());

  // --all flips `WindowRun` from skip-the-run to actually
  // launch + sleep + kill. Dev-machine only: needs a display
  // (Cocoa / X11 / Wayland), so CI keeps the default mode.
  let run_all = args.iter().any(|a| a == "--all");

  let root = find_workspace_root();
  let zo = find_zo_binary(&root);

  ensure_runtime_flavors(&root, &zo);

  let tests_dir = root.join("crates/compiler/zo-tests");
  let howto_dir = root.join("crates/compiler/zo-how-zo");

  println!("zo: {}", zo.display());

  if let Some(t) = target {
    println!("target: {t}");
  }

  if run_all {
    println!("mode: --all (windowed programs will launch)");
  }

  let tmp =
    env::temp_dir().join(format!("zo-test-runner-{}", std::process::id()));

  fs::create_dir_all(&tmp).expect("failed to create temp dir");

  let start = Instant::now();
  let mut results = Vec::new();

  let ctx = RunnerCtx {
    zo: &zo,
    tmp: &tmp,
    filter,
    target,
    run_all,
  };

  // programming/ — build + run + optional output check.
  run_dir(
    &ctx,
    &tests_dir.join("programming"),
    Category::Pass,
    &mut results,
  );

  // programming/fail/ — must fail to compile.
  run_dir(
    &ctx,
    &tests_dir.join("programming/fail"),
    Category::Fail,
    &mut results,
  );

  // programming/attributes/ — `%%` attribute pipeline
  // (parse + executor buffer + codegen `link_name`
  // dispatch). Build + run + EXPECTED OUTPUT match.
  run_dir(
    &ctx,
    &tests_dir.join("programming/attributes"),
    Category::Pass,
    &mut results,
  );

  // templating/ — ZSX programs. Now windowed: P2 of
  // `PLAN_DOM_CODEGEN_WIRING` wired `#render` codegen to
  // `_zo_run_native` in `libzo_runtime_native.dylib`,
  // which blocks on `eframe::run_native`. Same WindowRun
  // shape as raylib / misato: build always, run only with
  // `--all`.
  run_dir(
    &ctx,
    &tests_dir.join("templating"),
    Category::WindowRun,
    &mut results,
  );

  // provider/raylib/ — raylib demos via the C-library
  // provider shim. Same WindowRun shape as templating:
  // build always, run only with --all.
  run_dir(
    &ctx,
    &tests_dir.join("provider/raylib"),
    Category::WindowRun,
    &mut results,
  );

  // provider/sqlite/ — sqlite scoreboard via provider
  // shim. Stdout-emitting program, no window — Pass.
  run_dir(
    &ctx,
    &tests_dir.join("provider/sqlite"),
    Category::Pass,
    &mut results,
  );

  // provider/json/ — JSON parse/read/write via provider
  // shim wrapping serde_json. Same Pass shape as sqlite.
  run_dir(
    &ctx,
    &tests_dir.join("provider/json"),
    Category::Pass,
    &mut results,
  );

  // programming/misato/ — misato 3D demos. Same WindowRun
  // shape — covers cube_static, three_cubes, grid_1000, …
  run_dir(
    &ctx,
    &tests_dir.join("programming/misato"),
    Category::WindowRun,
    &mut results,
  );

  // templating/fail/ — must fail to compile.
  run_dir(
    &ctx,
    &tests_dir.join("templating/fail"),
    Category::Fail,
    &mut results,
  );

  // programming/sir/ — SIR verification (-- CHECK:).
  run_dir(
    &ctx,
    &tests_dir.join("programming/sir"),
    Category::Check,
    &mut results,
  );

  // programming/codegen/ — ARM64 verification (-- CHECK:).
  run_dir(
    &ctx,
    &tests_dir.join("programming/codegen"),
    Category::Check,
    &mut results,
  );

  // programming/crashes/ — ICE regression tests.
  run_dir(
    &ctx,
    &tests_dir.join("programming/crashes"),
    Category::Crash,
    &mut results,
  );

  // zo-how-to tutorials — build + run + output check.
  if howto_dir.exists() {
    run_dir(&ctx, &howto_dir, Category::Pass, &mut results);
  }

  // zo-usecases — multi-file projects (lib.zo + modules).
  // `pass/` projects must build + run cleanly; `fail/`
  // projects must fail to build (used for compile-time
  // diagnostic regressions that need a multi-file fixture).
  let usecases_dir = root.join("crates/compiler/zo-usecases");

  if usecases_dir.exists() {
    let pass_dir = usecases_dir.join("pass");
    let fail_dir = usecases_dir.join("fail");

    if pass_dir.exists() {
      run_projects(&pass_dir, &zo, &tmp, filter, target, &mut results);
    }

    if fail_dir.exists() {
      run_projects_fail(&fail_dir, &zo, &tmp, filter, target, &mut results);
    }
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
    "  {passed} passed, {failed} failed ({total} total) in {:.2}s",
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

/// Spawns `cmd` with stdout+stderr piped and either returns
/// its `Output` (if it finishes within `timeout`) or kills it
/// and returns `ErrorKind::TimedOut`.
///
/// Polling is `try_wait` + 25 ms sleep. Good enough for
/// test-runner granularity — we just don't want cross-arch
/// hangs (Rosetta-translated programs stuck at 0 CPU) to
/// block the whole rayon pool indefinitely.
///
/// On timeout: `child.kill()` sends SIGKILL and returns
/// immediately — we do NOT call `child.wait()` afterwards
/// because some Rosetta-translated processes ignore SIGKILL
/// and `wait` would then block the rayon worker forever.
/// Zombie leak is acceptable (OS reaps on parent exit);
/// unblocked test-runner progress is not.
fn run_with_timeout(mut cmd: Command, timeout: Duration) -> io::Result<Output> {
  wait_with_timeout(cmd.spawn()?, timeout)
}

/// Spawn the child with `stdin` piped, deliver `stdin_bytes`,
/// then close the pipe (sends EOF to `readln()`/`read()`).
/// A short program that reads less than we sent will close
/// its end first; the resulting broken-pipe write is normal
/// and ignored. Anything else propagates as a test failure.
fn run_with_timeout_stdin(
  mut cmd: Command,
  timeout: Duration,
  stdin_bytes: Vec<u8>,
) -> io::Result<Output> {
  cmd.stdin(Stdio::piped());

  let mut child = cmd.spawn()?;

  if let Some(mut stdin) = child.stdin.take()
    && let Err(e) = stdin.write_all(&stdin_bytes)
    && e.kind() != io::ErrorKind::BrokenPipe
  {
    let _ = child.kill();

    return Err(e);
  }

  wait_with_timeout(child, timeout)
}

/// Poll the spawned `child` until it exits or `timeout`
/// elapses; on timeout, kill the child and surface a
/// `TimedOut` error. Shared post-spawn body for the two
/// `run_with_timeout*` spawners — one source of truth for
/// the wait loop.
fn wait_with_timeout(
  mut child: Child,
  timeout: Duration,
) -> io::Result<Output> {
  let start = Instant::now();

  loop {
    match child.try_wait()? {
      Some(_) => return child.wait_with_output(),
      None => {
        if start.elapsed() > timeout {
          let _ = child.kill();

          return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "runtime timeout",
          ));
        }

        thread::sleep(Duration::from_millis(25));
      }
    }
  }
}

/// Starts a `zo build` invocation with the optional `--target`
/// flag pre-applied, before the test adds `<file> -o <out>`.
/// Threading the flag this way means `run_test` / `run_project`
/// don't each have to reimplement the same conditional arg
/// injection.
fn build_cmd(zo: &Path, target: Option<&str>) -> Command {
  let mut cmd = Command::new(zo);

  cmd.arg("build");

  if let Some(t) = target {
    cmd.args(["-t", t]);
  }

  cmd
}

/// Runner-wide config: same for every `run_dir` call in a
/// single `main()` invocation. Bundles the moving parts that
/// otherwise turn `run_dir` into an 8-arg function — the `zo`
/// binary path, scratch dir, optional filter / target, and
/// the `--all` flag.
struct RunnerCtx<'a> {
  zo: &'a Path,
  tmp: &'a Path,
  filter: Option<&'a str>,
  target: Option<&'a str>,
  run_all: bool,
}

fn run_dir(
  ctx: &RunnerCtx,
  dir: &Path,
  category: Category,
  results: &mut Vec<TestResult>,
) {
  let zo = ctx.zo;
  let tmp = ctx.tmp;
  let filter = ctx.filter;
  let target = ctx.target;
  let run_all = ctx.run_all;
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
  println!("[{dir_name}] {} files", files.len());

  // Parallelise per-file `run_test` calls within this
  // group — each test is a cold child-process spawn
  // (`zo build` + program run) so work scales linearly
  // with CPU count. We stream each PASS/FAIL as it
  // completes (a single `println!` is atomic, so lines
  // don't interleave) instead of buffering + draining —
  // the latter makes the group look "locked" until every
  // parallel worker finishes.
  //
  // WindowRun is the exception: in `--all` mode it
  // launches eframe + winit + wgpu per program. Running
  // 25+ graphics processes simultaneously saturates the
  // system and the OS starts killing unrelated
  // child-processes (Pass tests running in other groups)
  // by signal. Serialise WindowRun to keep that load
  // bounded; the cost is `n × WINDOW_KILL_AFTER`
  // wall-time (~50 s for templating's 25 files) which
  // only `--all` callers ever pay.
  let serialize = matches!(category, Category::WindowRun) && run_all;
  let mk_result = |file: &PathBuf| -> Option<TestResult> {
    let name = file.file_stem().and_then(|s| s.to_str()).unwrap_or("?");

    if let Some(f) = filter
      && !name.contains(f)
    {
      return None;
    }

    let result = run_test(file, name, category, zo, tmp, target, run_all);

    print_result(&result);

    Some(result)
  };
  let group_results = if serialize {
    files.iter().filter_map(mk_result).collect::<Vec<_>>()
  } else {
    files.par_iter().filter_map(mk_result).collect::<Vec<_>>()
  };

  results.extend(group_results);
}

/// Runs multi-file project tests. Each subdirectory with a
/// `src/main.zo` is treated as a project. The compiler auto-
/// discovers `src/lib.zo` for module resolution.
fn run_projects(
  dir: &Path,
  zo: &Path,
  tmp: &Path,
  filter: Option<&str>,
  target: Option<&str>,
  results: &mut Vec<TestResult>,
) {
  if !dir.exists() {
    return;
  }

  let mut projects = fs::read_dir(dir)
    .expect("failed to read dir")
    .filter_map(|e| e.ok())
    .map(|e| e.path())
    .filter(|p| p.is_dir() && p.join("src/main.zo").exists())
    .collect::<Vec<_>>();

  projects.sort();

  if projects.is_empty() {
    return;
  }

  let dir_name = dir
    .strip_prefix(dir.ancestors().nth(3).unwrap_or(dir))
    .unwrap_or(dir)
    .display();

  println!();
  println!("[{dir_name}] {} projects", projects.len());

  // Parallelise per-project build+run and stream PASS /
  // FAIL lines as each project finishes (same rationale
  // as `run_dir`).
  let group_results = projects
    .par_iter()
    .filter_map(|project| {
      let name = project.file_name().and_then(|s| s.to_str()).unwrap_or("?");

      if let Some(f) = filter
        && !name.contains(f)
      {
        return None;
      }

      let result = run_project(project, name, zo, tmp, target);

      print_result(&result);

      Some(result)
    })
    .collect::<Vec<_>>();

  results.extend(group_results);
}

fn print_result(result: &TestResult) {
  let icon = if result.passed {
    "\x1b[32mPASS\x1b[0m"
  } else {
    "\x1b[31mFAIL\x1b[0m"
  };

  if result.passed {
    println!("  {icon} {}", result.name);
  } else {
    println!("  {icon} {} — {}", result.name, result.reason);
  }
}

/// Build + run a single multi-file project rooted at
/// `project`. Extracted out of `run_projects` so the body
/// can run inside rayon's `par_iter` (closures there must
/// be `Send` and the borrow of `tmp`/`zo` as `&Path` is).
fn run_project(
  project: &Path,
  name: &str,
  zo: &Path,
  tmp: &Path,
  target: Option<&str>,
) -> TestResult {
  let main_zo = project.join("src/main.zo");
  let out = tmp.join(name);

  let build = build_cmd(zo, target)
    .arg(&*main_zo.to_string_lossy())
    .arg("-o")
    .arg(&out)
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .status();

  match build {
    Ok(s) if !s.success() => fail(name, "compilation failed"),
    Err(e) => fail(name, &format!("build error: {e}")),
    _ => {
      let mut run_cmd = Command::new(&out);

      run_cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .current_dir(project);

      let run = match extract_stdin(&main_zo) {
        Some(bytes) => {
          run_with_timeout_stdin(run_cmd, RUN_TIMEOUT, bytes.into_bytes())
        }
        None => run_with_timeout(run_cmd, RUN_TIMEOUT),
      };

      match run {
        Err(e) if e.kind() == io::ErrorKind::TimedOut => {
          fail(name, "runtime timeout (10s)")
        }
        Ok(output) if !output.status.success() => fail(
          name,
          &format!(
            "runtime crash (exit {})",
            output.status.code().unwrap_or(-1)
          ),
        ),
        Ok(output) => {
          let expected = extract_expected(&main_zo);

          if expected.is_empty() {
            ok(name)
          } else {
            let actual = String::from_utf8_lossy(&output.stdout);
            let actual_trimmed = actual.trim_end();
            let expected_trimmed = expected.trim_end();

            if actual_trimmed == expected_trimmed {
              ok(name)
            } else {
              fail(
                name,
                &format!(
                  "output mismatch\n  \
                   expected: {expected_trimmed}\n  \
                   actual:   {actual_trimmed}"
                ),
              )
            }
          }
        }
        Err(e) => fail(name, &format!("run error: {e}")),
      }
    }
  }
}

/// PASS-style projects all live under `pass/`; fail-mode
/// projects (compile-time diagnostic regressions that need a
/// multi-file fixture) live under `fail/`. Shape and naming
/// rules are identical — only the success condition flips.
fn run_projects_fail(
  dir: &Path,
  zo: &Path,
  tmp: &Path,
  filter: Option<&str>,
  target: Option<&str>,
  results: &mut Vec<TestResult>,
) {
  if !dir.exists() {
    return;
  }

  let mut projects = fs::read_dir(dir)
    .expect("failed to read dir")
    .filter_map(|e| e.ok())
    .map(|e| e.path())
    .filter(|p| p.is_dir() && p.join("src/main.zo").exists())
    .collect::<Vec<_>>();

  projects.sort();

  if projects.is_empty() {
    return;
  }

  let dir_name = dir
    .strip_prefix(dir.ancestors().nth(3).unwrap_or(dir))
    .unwrap_or(dir)
    .display();

  println!();
  println!("[{dir_name}] {} projects", projects.len());

  let group_results = projects
    .par_iter()
    .filter_map(|project| {
      let name = project.file_name().and_then(|s| s.to_str()).unwrap_or("?");

      if let Some(f) = filter
        && !name.contains(f)
      {
        return None;
      }

      let result = run_project_fail(project, name, zo, tmp, target);

      print_result(&result);

      Some(result)
    })
    .collect::<Vec<_>>();

  results.extend(group_results);
}

/// A fail-mode project passes iff `zo build` exits non-zero.
/// We compile with stderr captured: if the build succeeded
/// the project breaks the contract (it was supposed to be a
/// compile-time regression). If the project's `main.zo`
/// carries an `EXPECTED ERROR:` directive followed by `-- `
/// lines, stderr must contain that substring — same shape
/// `extract_expected` uses for `EXPECTED OUTPUT:`.
fn run_project_fail(
  project: &Path,
  name: &str,
  zo: &Path,
  tmp: &Path,
  target: Option<&str>,
) -> TestResult {
  let main_zo = project.join("src/main.zo");
  let out = tmp.join(name);

  let build = build_cmd(zo, target)
    .arg(&*main_zo.to_string_lossy())
    .arg("-o")
    .arg(&out)
    .stdout(Stdio::null())
    .stderr(Stdio::piped())
    .output();

  match build {
    Err(e) => fail(name, &format!("build error: {e}")),
    Ok(output) if output.status.success() => {
      fail(name, "compilation succeeded but failure was expected")
    }
    Ok(output) => {
      let expected = extract_expected_error(&main_zo);

      if expected.is_empty() {
        ok(name)
      } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains(&expected) {
          ok(name)
        } else {
          fail(
            name,
            &format!(
              "stderr missing expected fragment\n  \
               expected fragment: {expected}\n  \
               actual stderr: {stderr}"
            ),
          )
        }
      }
    }
  }
}

fn run_test(
  file: &Path,
  name: &str,
  category: Category,
  zo: &Path,
  tmp: &Path,
  target: Option<&str>,
  run_all: bool,
) -> TestResult {
  let out = tmp.join(name);

  match category {
    Category::WindowRun => {
      run_window_test(file, name, &out, zo, target, run_all)
    }

    Category::Fail => {
      let output = build_cmd(zo, target)
        .arg(&*file.to_string_lossy())
        .arg("-o")
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
              return fail(name, &format!("missing in error output: '{line}'"));
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
                "expected output covers {expected_errors} error(s) \
                 but compiler produced {actual_errors}",
              ),
            );
          }

          ok(name)
        }
        Err(e) => fail(name, &format!("exec error: {e}")),
      }
    }

    Category::Pass => {
      let build = build_cmd(zo, target)
        .arg(&*file.to_string_lossy())
        .arg("-o")
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

      let mut run_cmd = Command::new(&out);

      run_cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .current_dir(file.parent().unwrap_or(file));

      let run = match extract_stdin(file) {
        Some(bytes) => {
          run_with_timeout_stdin(run_cmd, RUN_TIMEOUT, bytes.into_bytes())
        }
        None => run_with_timeout(run_cmd, RUN_TIMEOUT),
      };

      match run {
        Err(e) if e.kind() == io::ErrorKind::TimedOut => {
          fail(name, "runtime timeout (10s)")
        }
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
            let actual_lines = actual_trimmed.lines().collect::<Vec<_>>();
            let expected_lines = expected_trimmed.lines().collect::<Vec<_>>();

            for (i, (a, e)) in
              actual_lines.iter().zip(expected_lines.iter()).enumerate()
            {
              if a != e {
                return fail(
                  name,
                  &format!("line {}: expected '{}', got '{}'", i + 1, e, a),
                );
              }
            }

            if actual_lines.len() != expected_lines.len() {
              fail(
                name,
                &format!(
                  "expected {} lines, got {}",
                  expected_lines.len(),
                  actual_lines.len()
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

    Category::Check => {
      // Parse -- @emit directive from the file.
      let content = fs::read_to_string(file).unwrap_or_default();

      let emit_flag = content
        .lines()
        .find_map(|l| l.trim().strip_prefix("-- @emit "))
        .unwrap_or("sir")
        .trim();

      // Collect -- CHECK: lines.
      let checks = content
        .lines()
        .filter_map(|l| l.trim().strip_prefix("-- CHECK: ").map(String::from))
        .collect::<Vec<_>>();

      if checks.is_empty() {
        return fail(name, "no -- CHECK: directives found");
      }

      // Build with --emit flag.
      let status = build_cmd(zo, target)
        .arg(&*file.to_string_lossy())
        .args(["--emit", emit_flag, "-o"])
        .arg(&out)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

      match status {
        Ok(s) if !s.success() => {
          return fail(name, "compilation failed");
        }
        Err(e) => {
          return fail(name, &format!("build exec error: {e}"));
        }
        _ => {}
      }

      // Read the emitted file (.sir or .asm).
      let emit_path = file.with_extension(emit_flag);
      let emit_content = fs::read_to_string(&emit_path).unwrap_or_default();

      // Clean up emitted file.
      let _ = fs::remove_file(&emit_path);

      // Verify each CHECK line.
      for check in &checks {
        if !emit_content.contains(check.as_str()) {
          return fail(name, &format!("CHECK failed: '{check}'"));
        }
      }

      ok(name)
    }

    Category::Crash => {
      // Build only — pass if the compiler doesn't crash.
      let output = build_cmd(zo, target)
        .arg(&*file.to_string_lossy())
        .arg("-o")
        .arg(&out)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output();

      match output {
        Ok(_o) => {
          // Check for signal death (crash).
          #[cfg(unix)]
          {
            use std::os::unix::process::ExitStatusExt;

            if let Some(signal) = _o.status.signal() {
              return fail(
                name,
                &format!("compiler crashed (signal {signal})"),
              );
            }
          }

          // Any exit (0 or error code) is fine — just no crash.
          ok(name)
        }
        Err(e) => fail(name, &format!("exec error: {e}")),
      }
    }
  }
}

/// Build the program; with `--all`, also spawn it, sleep
/// `WINDOW_KILL_AFTER`, then SIGKILL it. Pass iff the
/// process was still alive at kill time — windowed programs
/// are infinite frame loops, so an early exit means a
/// crash during `InitWindow` (missing dylib, bad symbol,
/// malformed Mach-O, …). Without `--all` this is build-only,
/// matching the prior `BuildOnly` behavior CI relies on.
fn run_window_test(
  file: &Path,
  name: &str,
  out: &Path,
  zo: &Path,
  target: Option<&str>,
  run_all: bool,
) -> TestResult {
  let build = build_cmd(zo, target)
    .arg(&*file.to_string_lossy())
    .arg("-o")
    .arg(out)
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .status();

  match build {
    Ok(s) if !s.success() => return fail(name, "compilation failed"),
    Err(e) => return fail(name, &format!("exec error: {e}")),
    _ => {}
  }

  if !run_all {
    return ok(name);
  }

  let mut cmd = Command::new(out);

  cmd
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .current_dir(file.parent().unwrap_or(file));

  let mut child = match cmd.spawn() {
    Ok(c) => c,
    Err(e) => return fail(name, &format!("spawn error: {e}")),
  };

  thread::sleep(WINDOW_KILL_AFTER);

  match child.try_wait() {
    Ok(Some(status)) => {
      // Process exited before we could kill it — failure mode
      // we care about (crash during init).
      let _ = child.wait();

      let detail = status.code().map_or_else(
        || "killed by signal".to_string(),
        |c| format!("exit {c}"),
      );

      fail(name, &format!("exited early ({detail})"))
    }
    Ok(None) => {
      // Still running after WINDOW_KILL_AFTER — success.
      // SIGKILL it; we deliberately do NOT `wait()` after,
      // mirroring `wait_with_timeout`'s rationale (some
      // children ignore SIGKILL; OS reaps zombies on parent
      // exit and we'd rather move on than block the worker).
      let _ = child.kill();

      ok(name)
    }
    Err(e) => {
      let _ = child.kill();

      fail(name, &format!("try_wait error: {e}"))
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

/// Reads an `EXPECTED ERROR:` directive from a fail-mode
/// project's `main.zo`. The directive's value is a single
/// substring (e.g. an error code like `E0708`) that must
/// appear somewhere in the compiler's stderr. Multi-line
/// continuations aren't supported — diagnostics format
/// changes frequently and we don't want to over-couple.
fn extract_expected_error(file: &Path) -> String {
  let content = fs::read_to_string(file).unwrap_or_default();

  for line in content.lines() {
    if let Some(rest) = line.split_once("EXPECTED ERROR:") {
      return rest.1.trim().to_string();
    }
  }

  String::new()
}

/// Read the `-- @stdin:` block from a test source. Returns
/// the bytes to feed the child process's stdin (each line
/// terminated by `\n` so `readln()` returns one entry per
/// directive line), or `None` when the directive is absent.
///
/// Shape mirrors `extract_expected`: a header line, then
/// zero or more `-- <line>` continuations, terminated by a
/// blank line, EOF, or any other directive marker.
fn extract_stdin(file: &Path) -> Option<String> {
  let content = fs::read_to_string(file).ok()?;
  let mut lines = Vec::new();
  let mut in_block = false;

  for line in content.lines() {
    if line.trim() == "-- @stdin:" {
      in_block = true;

      continue;
    }

    if !in_block {
      continue;
    }

    // Blank line ends the block — keeps `@stdin:` and a
    // following `EXPECTED OUTPUT:` visually separate.
    if line.trim().is_empty() {
      break;
    }

    // Any non-`-- ` line (or another directive header) ends
    // the block — we never silently swallow source code.
    let Some(rest) = line.strip_prefix("-- ") else {
      break;
    };

    lines.push(rest);
  }

  if !in_block {
    return None;
  }

  let mut joined = lines.join("\n");

  // Trailing newline so the last line is a complete `readln()`
  // entry. Without it the program would block waiting for
  // either more input or EOF on the same syscall.
  joined.push('\n');

  Some(joined)
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
/// Build both runtime flavors next to the chosen `zo`
/// binary's profile when either is missing.
///
/// `cargo build --bin zo` never compiles the `zo-runtime`
/// cdylib (it's not an rlib dep of the bin), so the lean
/// core (`libzo_runtime_core`) and full UI
/// (`libzo_runtime_ui`) dylibs the driver stages must be
/// produced explicitly. The two flavors share cargo's
/// `libzo_runtime.dylib` output name, so each cargo build
/// overwrites it — build, copy aside, repeat.
fn ensure_runtime_flavors(root: &Path, zo: &Path) {
  let profile = zo
    .parent()
    .and_then(|dir| dir.file_name())
    .and_then(|name| name.to_str())
    .unwrap_or("debug")
    .to_owned();

  let profile_dir = root.join("target").join(&profile);
  let core = profile_dir.join(RUNTIME_CORE_DYLIB);
  let ui = profile_dir.join(RUNTIME_UI_DYLIB);

  if core.exists() && ui.exists() {
    return;
  }

  eprintln!("building runtime flavors ({profile})...");

  let release = profile == "release";
  let built = profile_dir.join(RUNTIME_BUILT_DYLIB);

  // Full UI flavor (default features) — the superset dylib.
  if build_runtime_flavor(root, release, false) {
    let _ = fs::copy(&built, &ui);
  }

  // Lean core flavor — no UI / render / web tree.
  if build_runtime_flavor(root, release, true) {
    let _ = fs::copy(&built, &core);
  }
}

/// Run one `cargo build -p zo-runtime` for a single flavor.
/// `lean` toggles `--no-default-features`. Returns whether
/// the build succeeded.
fn build_runtime_flavor(root: &Path, release: bool, lean: bool) -> bool {
  let mut cmd = Command::new("cargo");

  cmd.args(["build", "-p", "zo-runtime"]).current_dir(root);

  if release {
    cmd.arg("--release");
  }

  if lean {
    cmd.arg("--no-default-features");
  }

  cmd.status().map(|status| status.success()).unwrap_or(false)
}

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

#[cfg(test)]
mod tests {
  use super::extract_stdin;

  use std::io::Write as _;
  use std::path::PathBuf;

  fn write_tmp(name: &str, body: &str) -> PathBuf {
    let path = std::env::temp_dir().join(name);
    let mut f = std::fs::File::create(&path).unwrap();

    f.write_all(body.as_bytes()).unwrap();

    path
  }

  #[test]
  fn extract_stdin_returns_none_when_directive_absent() {
    let path = write_tmp(
      "extract_stdin_none.zo",
      "fun main() {}\n\n-- EXPECTED OUTPUT:\n-- \n",
    );

    assert!(extract_stdin(&path).is_none());
  }

  #[test]
  fn extract_stdin_collects_lines_with_trailing_newline() {
    let path = write_tmp(
      "extract_stdin_lines.zo",
      "fun main() {}\n\n-- @stdin:\n-- buy milk\n-- write zo\n",
    );

    let bytes = extract_stdin(&path).expect("directive present");

    // Each line is `\n`-terminated so `readln` returns one
    // entry per directive line.
    assert_eq!(bytes, "buy milk\nwrite zo\n");
  }

  #[test]
  fn extract_stdin_stops_at_blank_line_then_expected_block() {
    let path = write_tmp(
      "extract_stdin_stops.zo",
      "fun main() {}\n\n\
       -- @stdin:\n\
       -- alpha\n\
       -- beta\n\
       \n\
       -- EXPECTED OUTPUT:\n\
       -- > alpha\n\
       -- > beta\n",
    );

    let bytes = extract_stdin(&path).expect("directive present");

    // Should NOT swallow `EXPECTED OUTPUT:` lines into stdin.
    assert_eq!(bytes, "alpha\nbeta\n");
  }

  #[test]
  fn extract_stdin_empty_directive_returns_just_newline() {
    let path = write_tmp(
      "extract_stdin_empty.zo",
      "fun main() {}\n\n-- @stdin:\n\n-- EXPECTED OUTPUT:\n",
    );

    // Empty block still signals "stdin opened then closed",
    // which is distinct from "no directive at all".
    let bytes = extract_stdin(&path).expect("directive present");

    assert_eq!(bytes, "\n");
  }
}
