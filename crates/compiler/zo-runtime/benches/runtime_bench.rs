//! Runtime-level benchmarks — fan-out throughput,
//! ping-pong latency, producer/consumer throughput.
//!
//! ```sh
//! cargo bench -p zo-runtime --bench runtime_bench
//! ```
//!
//! Each bench asserts *correctness* (counts, sums,
//! echo-order) but never asserts timing — benches
//! measure, they don't fail on variance. `criterion`'s
//! statistical harness (warm-up, sample size, outlier
//! rejection) handles run-to-run noise so regressions
//! surface as mean / p99 shifts, not one-shot spikes.

use zo_runtime::channel::{
  ZoChan, zo_chan_close, zo_chan_free, zo_chan_new, zo_chan_recv, zo_chan_send,
};
use zo_runtime::pool::Pool;
use zo_runtime::scheduler;
use zo_runtime::task::{zo_task_await, zo_task_spawn};

use criterion::{
  BenchmarkId, Criterion, Throughput, criterion_group, criterion_main,
};

use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};

// ===== fan-out: N tasks, M pool workers =====

static FAN_OUT_COUNTER: AtomicU64 = AtomicU64::new(0);

extern "C-unwind" fn fan_out_increment() {
  FAN_OUT_COUNTER.fetch_add(1, Ordering::SeqCst);
}

fn run_fan_out(n_tasks: usize, n_workers: usize) {
  FAN_OUT_COUNTER.store(0, Ordering::SeqCst);

  let pool = Pool::new(n_workers);

  for _ in 0..n_tasks {
    pool.spawn(fan_out_increment);
  }

  pool.wait_idle();

  assert_eq!(FAN_OUT_COUNTER.load(Ordering::SeqCst), n_tasks as u64);

  pool.shutdown();
}

fn bench_fan_out(c: &mut Criterion) {
  let mut group = c.benchmark_group("runtime_fan_out");

  for &(n_tasks, n_workers) in &[(10_000usize, 4usize), (100_000, 4)] {
    group.throughput(Throughput::Elements(n_tasks as u64));
    group.bench_with_input(
      BenchmarkId::new(format!("{n_workers}w"), n_tasks),
      &(n_tasks, n_workers),
      |b, &(n_tasks, n_workers)| {
        b.iter(|| run_fan_out(black_box(n_tasks), black_box(n_workers)));
      },
    );
  }

  group.finish();
}

// ===== ping-pong: two green tasks, N roundtrips =====

static PING_CHAN: AtomicU64 = AtomicU64::new(0);
static PONG_CHAN: AtomicU64 = AtomicU64::new(0);

const PING_PONG_ROUNDS: u64 = 1_000;

extern "C-unwind" fn pinger() {
  let ping = PING_CHAN.load(Ordering::SeqCst) as *mut ZoChan;
  let pong = PONG_CHAN.load(Ordering::SeqCst) as *mut ZoChan;

  unsafe {
    for i in 0..PING_PONG_ROUNDS {
      zo_chan_send(ping, (&raw const i).cast::<u8>());

      let mut echo: u64 = 0;
      zo_chan_recv(pong, (&raw mut echo).cast::<u8>());

      assert_eq!(echo, i, "pinger saw echo mismatch");
    }
  }
}

extern "C-unwind" fn ponger() {
  let ping = PING_CHAN.load(Ordering::SeqCst) as *mut ZoChan;
  let pong = PONG_CHAN.load(Ordering::SeqCst) as *mut ZoChan;

  unsafe {
    for _ in 0..PING_PONG_ROUNDS {
      let mut v: u64 = 0;

      zo_chan_recv(ping, (&raw mut v).cast::<u8>());
      zo_chan_send(pong, (&raw const v).cast::<u8>());
    }
  }
}

fn run_ping_pong() {
  scheduler::reset_for_test();

  unsafe {
    let ping = zo_chan_new(std::mem::size_of::<u64>(), 0);
    let pong = zo_chan_new(std::mem::size_of::<u64>(), 0);

    PING_CHAN.store(ping as u64, Ordering::SeqCst);
    PONG_CHAN.store(pong as u64, Ordering::SeqCst);

    let pinger_h = zo_task_spawn(pinger);
    let ponger_h = zo_task_spawn(ponger);

    zo_task_await(pinger_h);
    zo_task_await(ponger_h);

    zo_chan_free(ping);
    zo_chan_free(pong);
  }
}

fn bench_ping_pong(c: &mut Criterion) {
  let mut group = c.benchmark_group("runtime_ping_pong");

  group.throughput(Throughput::Elements(PING_PONG_ROUNDS));
  group.sample_size(20);
  group.bench_function(BenchmarkId::new("roundtrips", PING_PONG_ROUNDS), |b| {
    b.iter(run_ping_pong)
  });
  group.finish();
}

// ===== producer / consumer with close =====

static PROD_CHAN: AtomicU64 = AtomicU64::new(0);
static PROD_SUM: AtomicU64 = AtomicU64::new(0);

const PROD_N: u64 = 500;

extern "C-unwind" fn producer() {
  let ch = PROD_CHAN.load(Ordering::SeqCst) as *mut ZoChan;

  unsafe {
    for i in 1..=PROD_N {
      zo_chan_send(ch, (&raw const i).cast::<u8>());
    }

    zo_chan_close(ch);
  }
}

extern "C-unwind" fn consumer() {
  let ch = PROD_CHAN.load(Ordering::SeqCst) as *mut ZoChan;

  unsafe {
    loop {
      let mut v: u64 = 0;

      zo_chan_recv(ch, (&raw mut v).cast::<u8>());

      if v == 0 {
        return;
      }

      PROD_SUM.fetch_add(v, Ordering::SeqCst);
    }
  }
}

fn run_producer_consumer_close() {
  scheduler::reset_for_test();

  PROD_SUM.store(0, Ordering::SeqCst);

  unsafe {
    let ch = zo_chan_new(std::mem::size_of::<u64>(), 16);

    PROD_CHAN.store(ch as u64, Ordering::SeqCst);

    let prod_h = zo_task_spawn(producer);
    let cons_h = zo_task_spawn(consumer);

    zo_task_await(prod_h);
    zo_task_await(cons_h);

    let expected_sum: u64 = (1..=PROD_N).sum();

    assert_eq!(PROD_SUM.load(Ordering::SeqCst), expected_sum);

    zo_chan_free(ch);
  }
}

fn bench_producer_consumer_close(c: &mut Criterion) {
  let mut group = c.benchmark_group("runtime_producer_consumer_close");

  group.throughput(Throughput::Elements(PROD_N));
  group.sample_size(30);
  group.bench_function(BenchmarkId::new("values+close", PROD_N), |b| {
    b.iter(run_producer_consumer_close)
  });
  group.finish();
}

criterion_group!(
  benches,
  bench_fan_out,
  bench_ping_pong,
  bench_producer_consumer_close,
);

criterion_main!(benches);
