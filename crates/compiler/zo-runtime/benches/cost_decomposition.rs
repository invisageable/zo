//! Decomposes per-spawn cost into isolated components
//! so optimization work is driven by measurement, not
//! guesses.
//!
//! Four microbenchmarks, all run serially in one test
//! to avoid parallel-test contention skewing timing:
//!
//! 1. `stack_alloc_256kb_vec0` — one `vec![0u8; 256 * 1024]`.
//!    Every spawn pays this unconditionally through the
//!    task stack allocation.
//! 2. `zotask_construct_drop` — construct via
//!    `ZoTask::new_green_standalone` then drop, no
//!    execution. Isolates stack-alloc cost from the
//!    surrounding `Box<ZoTask>`, `Arc<AtomicBool>`, and
//!    `Context::bootstrap` work.
//! 3. `ctxsw_roundtrip_scheduler` — single task that
//!    yields N times. 2 context-switches per yield; no
//!    channels, no queue contention.
//! 4. `spawn_to_entry_latency` — wall-clock from the
//!    `_zo_task_spawn` call site to the first user
//!    instruction inside the task. Measured as
//!    median / p99 / min / max across 200 iters.
//!
//! Results are written to
//! `/tmp/zo-cost-decomposition.txt` (truncated each
//! run) and also emitted on stderr via `eprintln!` so
//! running with `--success-output immediate` shows
//! them live.

use zo_runtime::scheduler;
use zo_runtime::task::{_zo_task_await, _zo_task_spawn, ZoTask};

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

// ===== result emission =====

static RESULTS_PATH: OnceLock<PathBuf> = OnceLock::new();

fn init_results_path() -> PathBuf {
  let p = PathBuf::from("/tmp/zo-cost-decomposition.txt");
  // Truncate on first access per run so successive
  // invocations don't pile up residue.
  let _ = std::fs::write(&p, "# zo cost decomposition.\n");
  p
}

fn emit(label: &str, result: &str) {
  let path = RESULTS_PATH.get_or_init(init_results_path);

  let mut f = OpenOptions::new()
    .create(true)
    .append(true)
    .open(path)
    .expect("cannot open cost-decomposition file");

  writeln!(f, "{label}: {result}").expect("cannot write measurements");

  eprintln!("[bench] {label}: {result}");
}

// ===== batched timing helper =====

/// Run `f` `ops_per_batch` times, measure wall-time per
/// batch, compute per-op ns, repeat across `batches` to
/// get median / min / max. Warms the instruction +
/// data caches with one untimed batch first so the
/// timed runs don't pay first-touch costs.
fn run_batched<F: FnMut()>(
  label: &str,
  ops_per_batch: u64,
  batches: usize,
  mut f: F,
) {
  // Warmup batch (not timed).
  for _ in 0..ops_per_batch {
    f();
  }

  let mut per_op_ns: Vec<u64> = Vec::with_capacity(batches);

  for _ in 0..batches {
    let start = Instant::now();

    for _ in 0..ops_per_batch {
      f();
    }

    let elapsed_ns = start.elapsed().as_nanos() as u64;

    per_op_ns.push(elapsed_ns / ops_per_batch);
  }

  per_op_ns.sort();

  let median = per_op_ns[per_op_ns.len() / 2];
  let min = per_op_ns[0];
  let max = per_op_ns[per_op_ns.len() - 1];

  emit(
    label,
    &format!(
      "median {median} ns/op, range [{min}, {max}] ({batches} × {ops_per_batch} ops)",
    ),
  );
}

// ===== 1. stack alloc isolated =====

fn bench_stack_alloc() {
  run_batched("stack_alloc_256kb_vec0", 100, 20, || {
    let stack = vec![0u8; 256 * 1024];

    std::hint::black_box(&stack);
  });
}

// ===== 2. ZoTask construct + drop =====

extern "C-unwind" fn noop_entry() {}

fn bench_zotask_construct_drop() {
  run_batched("zotask_construct_drop", 100, 20, || {
    let task = ZoTask::new_green_standalone(noop_entry);

    std::hint::black_box(&task);

    drop(task);
  });
}

// ===== 3. scheduler ctxsw round-trip =====

static YIELD_COUNT: AtomicU64 = AtomicU64::new(0);

extern "C-unwind" fn yielder() {
  let n = YIELD_COUNT.load(Ordering::SeqCst);

  for _ in 0..n {
    // SAFETY: called from inside a green task — the
    // scheduler's `current` slot is Some.
    unsafe { scheduler::yield_now() };
  }
}

fn bench_ctxsw_roundtrip() {
  scheduler::reset_for_test();

  const N_YIELDS: u64 = 10_000;

  // Warmup — spawn a short yielder first so any lazy
  // allocator / page-fault work is paid before timing.
  YIELD_COUNT.store(10, Ordering::SeqCst);
  unsafe {
    let warm = _zo_task_spawn(yielder);
    _zo_task_await(warm);
  }
  YIELD_COUNT.store(N_YIELDS, Ordering::SeqCst);

  let start = Instant::now();

  unsafe {
    let task = _zo_task_spawn(yielder);

    _zo_task_await(task);
  }

  let elapsed_ns = start.elapsed().as_nanos() as u64;

  // Every `yield_now` is 2 ctxsw: task→scheduler,
  // scheduler→task (on the next pop_ready).
  let total_switches = 2 * N_YIELDS;
  let per_switch = elapsed_ns / total_switches;

  emit(
    "ctxsw_roundtrip_scheduler",
    &format!(
      "{per_switch} ns/switch ({total_switches} switches over {elapsed_ns} ns)",
    ),
  );
}

// ===== 4. spawn → entry latency =====

static EPOCH: OnceLock<Instant> = OnceLock::new();
static ENTRY_NS: AtomicU64 = AtomicU64::new(0);

extern "C-unwind" fn record_entry_timestamp() {
  let epoch = EPOCH.get().expect("epoch not initialized");
  let ns = Instant::now().duration_since(*epoch).as_nanos() as u64;

  ENTRY_NS.store(ns, Ordering::SeqCst);
}

fn bench_spawn_to_entry_latency() {
  scheduler::reset_for_test();

  EPOCH.get_or_init(Instant::now);

  let epoch = EPOCH.get().unwrap();

  const ITERS: usize = 200;

  // Warmup — pay first-allocation costs before timing.
  for _ in 0..20 {
    ENTRY_NS.store(0, Ordering::SeqCst);

    unsafe {
      let task = _zo_task_spawn(record_entry_timestamp);
      _zo_task_await(task);
    }
  }

  let mut latencies_ns: Vec<u64> = Vec::with_capacity(ITERS);

  for _ in 0..ITERS {
    ENTRY_NS.store(0, Ordering::SeqCst);

    let spawn_ns = Instant::now().duration_since(*epoch).as_nanos() as u64;

    unsafe {
      let task = _zo_task_spawn(record_entry_timestamp);
      _zo_task_await(task);
    }

    let entry_ns = ENTRY_NS.load(Ordering::SeqCst);

    latencies_ns.push(entry_ns.saturating_sub(spawn_ns));
  }

  latencies_ns.sort();

  let median = latencies_ns[ITERS / 2];
  let p99 = latencies_ns[(ITERS * 99) / 100];
  let min = latencies_ns[0];
  let max = latencies_ns[ITERS - 1];

  emit(
    "spawn_to_entry_latency",
    &format!(
      "median {median} ns, p99 {p99} ns, min {min} ns, max {max} ns ({ITERS} iters)",
    ),
  );
}

// ===== harness =====

fn main() {
  // Truncate the results file up front.
  RESULTS_PATH.get_or_init(init_results_path);

  println!("=== zo-runtime spawn cost breakdown ===");

  bench_stack_alloc();
  bench_zotask_construct_drop();
  bench_ctxsw_roundtrip();
  bench_spawn_to_entry_latency();

  let path = RESULTS_PATH.get().unwrap();

  eprintln!("[bench] all measurements written to {}", path.display());
}
