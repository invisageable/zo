//! Decomposes per-spawn cost into isolated components
//! so optimization work is driven by measurement, not
//! guesses.
//!
//! Four microbenchmarks:
//!
//! 1. `task_stack_reserve_commit` — one full
//!    `TaskStack::reserve()` pair (pop from pool or
//!    `mmap` + `mprotect` on miss) plus the matching
//!    recycle on drop. Every spawn pays this
//!    unconditionally.
//! 2. `zotask_construct_drop` — construct via
//!    `ZoTask::new_green_standalone` then drop, no
//!    execution. Isolates stack-alloc cost from the
//!    surrounding `Box<ZoTask>`, `Arc<AtomicBool>`, and
//!    `Context::bootstrap` work.
//! 3. `ctxsw_roundtrip_scheduler` — one task that
//!    yields N times. 2 context-switches per yield; no
//!    channels, no queue contention.
//! 4. `spawn_to_entry_latency` — wall-clock from the
//!    `zo_task_spawn` call site to the first user
//!    instruction inside the task.
//!
//! ```sh
//! cargo bench -p zo-runtime --bench cost_decomposition
//! ```

use zo_runtime::scheduler;
use zo_runtime::stack::TaskStack;
use zo_runtime::task::{ZoTask, zo_task_await, zo_task_spawn};

use criterion::{Criterion, criterion_group, criterion_main};

use std::hint::black_box;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

// ===== 1. task stack reserve + recycle =====

fn bench_task_stack_reserve_commit(c: &mut Criterion) {
  c.bench_function("task_stack_reserve_commit", |b| {
    b.iter(|| {
      let stack = TaskStack::reserve();

      // `recycle` consumes the Box and hands it back
      // to the pool — matches the real spawn / drop
      // cycle this microbench is trying to isolate.
      TaskStack::recycle(black_box(stack));
    });
  });
}

// ===== 2. ZoTask construct + drop =====

extern "C-unwind" fn noop_entry() {}

fn bench_zotask_construct_drop(c: &mut Criterion) {
  c.bench_function("zotask_construct_drop", |b| {
    b.iter(|| {
      let task = ZoTask::new_green_standalone(noop_entry);

      black_box(task);
    });
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

fn bench_ctxsw_roundtrip(c: &mut Criterion) {
  const N_YIELDS: u64 = 10_000;

  let mut group = c.benchmark_group("ctxsw_roundtrip_scheduler");
  // Each iter does `2 * N_YIELDS` context switches
  // (task → scheduler + scheduler → task per yield).
  // Reporting throughput in switches lets criterion
  // print a per-switch number directly.
  group.throughput(criterion::Throughput::Elements(2 * N_YIELDS));
  group.sample_size(20);

  group.bench_function("10k_yields", |b| {
    b.iter_custom(|iters| {
      scheduler::reset_for_test();
      YIELD_COUNT.store(N_YIELDS, Ordering::SeqCst);

      let mut total = Duration::ZERO;

      for _ in 0..iters {
        let start = Instant::now();

        unsafe {
          let task = zo_task_spawn(yielder);

          zo_task_await(task);
        }

        total += start.elapsed();
      }

      total
    });
  });

  group.finish();
}

// ===== 4. spawn → entry latency =====

static EPOCH: OnceLock<Instant> = OnceLock::new();
static ENTRY_NS: AtomicU64 = AtomicU64::new(0);

extern "C-unwind" fn record_entry_timestamp() {
  let epoch = EPOCH.get().expect("epoch not initialized");
  let ns = Instant::now().duration_since(*epoch).as_nanos() as u64;

  ENTRY_NS.store(ns, Ordering::SeqCst);
}

fn bench_spawn_to_entry_latency(c: &mut Criterion) {
  EPOCH.get_or_init(Instant::now);

  let mut group = c.benchmark_group("spawn_to_entry_latency");
  group.sample_size(100);

  group.bench_function("single_spawn", |b| {
    b.iter_custom(|iters| {
      scheduler::reset_for_test();

      let epoch = EPOCH.get().unwrap();
      let mut total = Duration::ZERO;

      for _ in 0..iters {
        ENTRY_NS.store(0, Ordering::SeqCst);

        let spawn_ns = Instant::now().duration_since(*epoch).as_nanos() as u64;

        unsafe {
          let task = zo_task_spawn(record_entry_timestamp);

          zo_task_await(task);
        }

        let entry_ns = ENTRY_NS.load(Ordering::SeqCst);

        total += Duration::from_nanos(entry_ns.saturating_sub(spawn_ns));
      }

      total
    });
  });

  group.finish();
}

criterion_group!(
  benches,
  bench_task_stack_reserve_commit,
  bench_zotask_construct_drop,
  bench_ctxsw_roundtrip,
  bench_spawn_to_entry_latency,
);

criterion_main!(benches);
