//! Reactivity micro-benchmarks — the fine-grained headline.
//!
//! - `refresh/one_slot/N` vs `refresh/all_slots/N`: writing one
//!   of N bound labels is flat (the dirty refresh walks only the
//!   written slot's commands), while the old walk-all grows with
//!   N. The O(1)-vs-O(N) split made visible.
//! - `reconcile/mutate_one/N`: the keyed diff for a single-item
//!   change — a cheap linear scan producing a constant (2-edit)
//!   script regardless of N.

use criterion::{
  BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main,
};

use zo_runtime_render::reactive::{
  BindingGraph, BindingRef, DirtyCommands, reconcile_list, refresh_dirty,
};
use zo_ui_protocol::UiCommand;

use std::hint::black_box;

/// N text commands, each bound 1:1 to its own state slot.
fn labels(n: usize) -> (BindingGraph, Vec<UiCommand>) {
  let edges: Vec<(u32, BindingRef)> = (0..n as u32)
    .map(|i| (i, BindingRef::Text { cmd_idx: i }))
    .collect();
  let cmds = (0..n).map(|i| UiCommand::Text(i.to_string())).collect();

  (BindingGraph::from_edges(n, &edges), cmds)
}

fn bench_refresh(c: &mut Criterion) {
  let mut group = c.benchmark_group("refresh");

  for n in [10usize, 100, 1000] {
    let (graph, base) = labels(n);
    let one = vec![(n / 2) as u32];
    let all: Vec<u32> = (0..n as u32).collect();

    // One slot written — flat as N grows.
    group.bench_with_input(BenchmarkId::new("one_slot", n), &n, |b, _| {
      b.iter_batched(
        || base.clone(),
        |mut cmds| {
          let mut out = DirtyCommands::with_capacity(n);

          refresh_dirty(&graph, &one, &[], &mut cmds, &mut out, |slot| {
            Some(format!("v{slot}"))
          });

          black_box(&out);
        },
        BatchSize::SmallInput,
      );
    });

    // Every slot written — the walk-all baseline, grows with N.
    group.bench_with_input(BenchmarkId::new("all_slots", n), &n, |b, _| {
      b.iter_batched(
        || base.clone(),
        |mut cmds| {
          let mut out = DirtyCommands::with_capacity(n);

          refresh_dirty(&graph, &all, &[], &mut cmds, &mut out, |slot| {
            Some(format!("v{slot}"))
          });

          black_box(&out);
        },
        BatchSize::SmallInput,
      );
    });
  }

  group.finish();
}

fn bench_reconcile(c: &mut Criterion) {
  let mut group = c.benchmark_group("reconcile");

  for n in [10usize, 100, 1000] {
    let old: Vec<u32> = (0..n as u32).collect();
    let mut new = old.clone();

    new[n / 2] = u32::MAX;

    group.bench_with_input(BenchmarkId::new("mutate_one", n), &n, |b, _| {
      b.iter(|| black_box(reconcile_list(black_box(&old), black_box(&new))));
    });
  }

  group.finish();
}

criterion_group!(benches, bench_refresh, bench_reconcile);
criterion_main!(benches);
