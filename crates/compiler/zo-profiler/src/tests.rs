use crate::Profiler;

use std::time::Duration;

#[test]
fn fresh_profiler_has_zero_total() {
  let profiler = Profiler::new();

  assert_eq!(profiler.total_time(), Duration::ZERO);
  assert_eq!(profiler.wall_time(), profiler.total_time());
}

#[test]
fn resolver_counts_toward_frontend() {
  let mut profiler = Profiler::new();

  profiler.start_phase("tokenizer");
  profiler.end_phase("tokenizer");
  profiler.start_phase("resolver");
  profiler.end_phase("resolver");

  let tokenizer = profiler.phase_time("tokenizer").unwrap();
  let resolver = profiler.phase_time("resolver").unwrap();

  assert_eq!(profiler.frontend_time(), tokenizer + resolver);
}

#[test]
fn wall_time_covers_every_phase() {
  let mut profiler = Profiler::new();

  profiler.start_phase("tokenizer");
  profiler.end_phase("tokenizer");
  profiler.start_phase("linker");
  profiler.end_phase("linker");

  assert!(profiler.wall_time() >= profiler.total_time());
}
