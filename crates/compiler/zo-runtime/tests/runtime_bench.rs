//! Runtime-level benchmark integration tests —
//! fan-out throughput, ping-pong latency, and mixed
//! producer/consumer throughput.
//!
//! Assertions use generous upper bounds so CI variance
//! doesn't produce flaky failures; the real value is
//! the wall-time print-out developers can compare
//! across runs.
//!
//! Run with:
//!
//! ```sh
//! just test_crate zo-runtime
//! ```

use zo_runtime::channel::{
  _zo_chan_close, _zo_chan_free, _zo_chan_new, _zo_chan_recv, _zo_chan_send,
  ZoChan,
};
use zo_runtime::pool::Pool;
use zo_runtime::scheduler;
use zo_runtime::task::{_zo_task_await, _zo_task_spawn};

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

// Serializes benches that share static state (fan-out
// counter, static channel pointers). Nextest runs
// integration-test functions in parallel by default.
static FANOUT_LOCK: Mutex<()> = Mutex::new(());
static CHAN_LOCK: Mutex<()> = Mutex::new(());

// ===== fan-out: N workers, K tasks =====

static FAN_OUT_COUNTER: AtomicU64 = AtomicU64::new(0);

extern "C-unwind" fn fan_out_increment() {
  FAN_OUT_COUNTER.fetch_add(1, Ordering::SeqCst);
}

#[test]
fn bench_fan_out_10k() {
  // 10K tasks across 4 workers. Each increments an
  // atomic. Exercises multi-scheduler dispatch +
  // work-stealing under moderate load.
  let _lock = FANOUT_LOCK.lock().unwrap();

  const N_WORKERS: usize = 4;
  const N_TASKS: usize = 10_000;

  FAN_OUT_COUNTER.store(0, Ordering::SeqCst);

  let pool = Pool::new(N_WORKERS);
  let start = Instant::now();

  for _ in 0..N_TASKS {
    pool.spawn(fan_out_increment);
  }

  pool.wait_idle();

  let elapsed = start.elapsed();

  assert_eq!(FAN_OUT_COUNTER.load(Ordering::SeqCst), N_TASKS as u64);
  assert!(
    elapsed.as_secs() < 2,
    "fan-out regression: {N_TASKS} tasks in {elapsed:?} (target < 2s)",
  );

  println!(
    "[bench] fan_out {N_TASKS} tasks / {N_WORKERS} workers — {elapsed:?}"
  );

  pool.shutdown();
}

#[test]
fn bench_fan_out_100k() {
  // 100K green tasks on one pool, wall time ≤ 2 s on
  // M1 — the high-fan-out contract for the scheduler.
  let _lock = FANOUT_LOCK.lock().unwrap();

  const N_WORKERS: usize = 4;
  const N_TASKS: usize = 100_000;

  FAN_OUT_COUNTER.store(0, Ordering::SeqCst);

  let pool = Pool::new(N_WORKERS);
  let start = Instant::now();

  for _ in 0..N_TASKS {
    pool.spawn(fan_out_increment);
  }

  pool.wait_idle();

  let elapsed = start.elapsed();

  assert_eq!(FAN_OUT_COUNTER.load(Ordering::SeqCst), N_TASKS as u64);
  assert!(
    elapsed.as_secs() < 2,
    "fan-out 100K regression: {N_TASKS} tasks in {elapsed:?} (target ≤ 2 s)",
  );

  println!(
    "[bench] fan_out {N_TASKS} tasks / {N_WORKERS} workers — {elapsed:?}"
  );

  pool.shutdown();
}

// ===== ping-pong: two green tasks, many roundtrips =====

static PING_CHAN: AtomicU64 = AtomicU64::new(0);
static PONG_CHAN: AtomicU64 = AtomicU64::new(0);

const PING_PONG_ROUNDS: u64 = 1_000;

extern "C-unwind" fn pinger() {
  let ping = PING_CHAN.load(Ordering::SeqCst) as *mut ZoChan;
  let pong = PONG_CHAN.load(Ordering::SeqCst) as *mut ZoChan;

  unsafe {
    for i in 0..PING_PONG_ROUNDS {
      _zo_chan_send(ping, (&raw const i).cast::<u8>());

      let mut echo: u64 = 0;
      _zo_chan_recv(pong, (&raw mut echo).cast::<u8>());

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

      _zo_chan_recv(ping, (&raw mut v).cast::<u8>());
      _zo_chan_send(pong, (&raw const v).cast::<u8>());
    }
  }
}

#[test]
fn bench_ping_pong() {
  // Two green tasks bouncing N integers through a pair
  // of rendezvous channels. Exercises green-task park
  // + wake on every hop — the slowest path in the
  // channel layer.
  let _lock = CHAN_LOCK.lock().unwrap();

  scheduler::reset_for_test();

  unsafe {
    let ping = _zo_chan_new(std::mem::size_of::<u64>(), 0);
    let pong = _zo_chan_new(std::mem::size_of::<u64>(), 0);

    PING_CHAN.store(ping as u64, Ordering::SeqCst);
    PONG_CHAN.store(pong as u64, Ordering::SeqCst);

    let start = Instant::now();
    let pinger_h = _zo_task_spawn(pinger);
    let ponger_h = _zo_task_spawn(ponger);

    _zo_task_await(pinger_h);
    _zo_task_await(ponger_h);

    let elapsed = start.elapsed();

    println!("[bench] ping_pong {PING_PONG_ROUNDS} rounds — {elapsed:?}");

    assert!(
      elapsed.as_secs() < 5,
      "ping-pong regression: {PING_PONG_ROUNDS} rounds in {elapsed:?}",
    );

    _zo_chan_free(ping);
    _zo_chan_free(pong);
  }
}

// ===== producer / consumer with close =====

static PROD_CHAN: AtomicU64 = AtomicU64::new(0);
static PROD_SUM: AtomicU64 = AtomicU64::new(0);

const PROD_N: u64 = 500;

extern "C-unwind" fn producer() {
  let ch = PROD_CHAN.load(Ordering::SeqCst) as *mut ZoChan;

  unsafe {
    for i in 1..=PROD_N {
      _zo_chan_send(ch, (&raw const i).cast::<u8>());
    }

    _zo_chan_close(ch);
  }
}

extern "C-unwind" fn consumer() {
  let ch = PROD_CHAN.load(Ordering::SeqCst) as *mut ZoChan;

  unsafe {
    loop {
      let mut v: u64 = 0;

      _zo_chan_recv(ch, (&raw mut v).cast::<u8>());

      if v == 0 {
        // Closed empty — zero-fill signals end.
        return;
      }

      PROD_SUM.fetch_add(v, Ordering::SeqCst);
    }
  }
}

#[test]
fn bench_producer_consumer_close() {
  // One producer sends N values then closes; one
  // consumer drains the channel until the closed
  // zero-fill sentinel. Validates the close() wake-all
  // semantics end-to-end, including the case where the
  // consumer is parked on an empty channel at the
  // moment of close.
  let _lock = CHAN_LOCK.lock().unwrap();

  scheduler::reset_for_test();

  PROD_SUM.store(0, Ordering::SeqCst);

  unsafe {
    let ch = _zo_chan_new(std::mem::size_of::<u64>(), 16);

    PROD_CHAN.store(ch as u64, Ordering::SeqCst);

    let start = Instant::now();
    let prod_h = _zo_task_spawn(producer);
    let cons_h = _zo_task_spawn(consumer);

    _zo_task_await(prod_h);
    _zo_task_await(cons_h);

    let elapsed = start.elapsed();
    let expected_sum: u64 = (1..=PROD_N).sum();

    assert_eq!(PROD_SUM.load(Ordering::SeqCst), expected_sum);

    println!("[bench] producer_consumer {PROD_N} values — {elapsed:?}");

    _zo_chan_free(ch);
  }
}
