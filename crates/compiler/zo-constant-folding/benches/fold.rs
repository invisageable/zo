//! ```
//! cargo bench -p zo-constant-folding --bench fold --quiet
//! ```

use zo_constant_folding::ConstFold;
use zo_interner::Interner;
use zo_sir::BinOp;
use zo_span::Span;
use zo_ty::{IntWidth, Ty};
use zo_value::{ValueId, ValueStorage};

use criterion::{Criterion, Throughput, criterion_group, criterion_main};

use std::hint::black_box;

const U64: Ty = Ty::Int {
  signed: false,
  width: IntWidth::U64,
};
const S32: Ty = Ty::Int {
  signed: true,
  width: IntWidth::S32,
};
const F64: Ty = Ty::Float(zo_ty::FloatWidth::F64);
const BOOL: Ty = Ty::Bool;

/// Pre-populate a ValueStorage with N int pairs and return
/// the ids for benchmarking.
fn make_int_pairs(n: usize) -> (ValueStorage, Vec<(ValueId, ValueId)>) {
  let mut values = ValueStorage::new(n * 2);
  let mut pairs = Vec::with_capacity(n);

  for i in 0..n {
    let a = values.store_int(i as u64 + 1);
    let b = values.store_int(i as u64 + 2);

    pairs.push((a, b));
  }

  (values, pairs)
}

// === BINOP FOLDING ===

fn bench_binop(c: &mut Criterion) {
  let mut group = c.benchmark_group("fold_binop");

  group.bench_function("int add (1k)", |b| {
    let (values, pairs) = make_int_pairs(1000);
    let mut interner = Interner::new();
    let mut fold = ConstFold::new(&values, &mut interner);

    b.iter(|| {
      for &(a, b_id) in &pairs {
        black_box(fold.fold_binop(BinOp::Add, a, b_id, Span::ZERO, U64));
      }
    })
  });

  group.bench_function("int div (1k)", |b| {
    let (values, pairs) = make_int_pairs(1000);
    let mut interner = Interner::new();
    let mut fold = ConstFold::new(&values, &mut interner);

    b.iter(|| {
      for &(a, b_id) in &pairs {
        black_box(fold.fold_binop(BinOp::Div, a, b_id, Span::ZERO, U64));
      }
    })
  });

  group.bench_function("int comparison (1k)", |b| {
    let (values, pairs) = make_int_pairs(1000);
    let mut interner = Interner::new();
    let mut fold = ConstFold::new(&values, &mut interner);

    b.iter(|| {
      for &(a, b_id) in &pairs {
        black_box(fold.fold_binop(BinOp::Lt, a, b_id, Span::ZERO, U64));
      }
    })
  });

  group.bench_function("int bitwise (1k)", |b| {
    let (values, pairs) = make_int_pairs(1000);
    let mut interner = Interner::new();
    let mut fold = ConstFold::new(&values, &mut interner);

    b.iter(|| {
      for &(a, b_id) in &pairs {
        black_box(fold.fold_binop(BinOp::BitAnd, a, b_id, Span::ZERO, U64));
      }
    })
  });

  group.bench_function("float add (1k)", |b| {
    let mut values = ValueStorage::new(2000);
    let mut pairs = Vec::with_capacity(1000);

    for i in 0..1000 {
      let a = values.store_float(i as f64 + 0.5);
      let b_id = values.store_float(i as f64 + 1.5);

      pairs.push((a, b_id));
    }

    let mut interner = Interner::new();
    let mut fold = ConstFold::new(&values, &mut interner);

    b.iter(|| {
      for &(a, b_id) in &pairs {
        black_box(fold.fold_binop(BinOp::Add, a, b_id, Span::ZERO, F64));
      }
    })
  });

  group.bench_function("bool logic (1k)", |b| {
    let mut values = ValueStorage::new(2000);
    let mut pairs = Vec::with_capacity(1000);

    for i in 0..1000 {
      let a = values.store_bool(i % 2 == 0);
      let b_id = values.store_bool(i % 3 == 0);

      pairs.push((a, b_id));
    }

    let mut interner = Interner::new();
    let mut fold = ConstFold::new(&values, &mut interner);

    b.iter(|| {
      for &(a, b_id) in &pairs {
        black_box(fold.fold_binop(BinOp::And, a, b_id, Span::ZERO, BOOL));
      }
    })
  });

  group.finish();
}

// === ALGEBRAIC SIMPLIFICATION ===

fn bench_simplify(c: &mut Criterion) {
  let mut group = c.benchmark_group("simplify");

  group.bench_function("identity x+0 (1k)", |b| {
    let mut values = ValueStorage::new(2000);
    let mut pairs = Vec::with_capacity(1000);

    for _ in 0..1000 {
      let x = values.store_runtime(0);
      let zero = values.store_int(0);

      pairs.push((x, zero));
    }

    let mut interner = Interner::new();
    let mut fold = ConstFold::new(&values, &mut interner);

    b.iter(|| {
      for &(x, zero) in &pairs {
        black_box(fold.fold_binop(BinOp::Add, x, zero, Span::ZERO, U64));
      }
    })
  });

  group.bench_function("strength x*8→shl (1k)", |b| {
    let mut values = ValueStorage::new(2000);
    let mut pairs = Vec::with_capacity(1000);

    for _ in 0..1000 {
      let x = values.store_runtime(0);
      let eight = values.store_int(8);

      pairs.push((x, eight));
    }

    let mut interner = Interner::new();
    let mut fold = ConstFold::new(&values, &mut interner);

    b.iter(|| {
      for &(x, eight) in &pairs {
        black_box(fold.fold_binop(BinOp::Mul, x, eight, Span::ZERO, U64));
      }
    })
  });

  group.finish();
}

// === WIDTH-AWARE OVERFLOW ===

fn bench_width(c: &mut Criterion) {
  let mut group = c.benchmark_group("width");

  group.bench_function("s32 add with validation (1k)", |b| {
    let (values, pairs) = make_int_pairs(1000);
    let mut interner = Interner::new();
    let mut fold = ConstFold::new(&values, &mut interner);

    b.iter(|| {
      for &(a, b_id) in &pairs {
        black_box(fold.fold_binop(BinOp::Add, a, b_id, Span::ZERO, S32));
      }
    })
  });

  group.finish();
}

// === SCALING ===

fn bench_scaling(c: &mut Criterion) {
  for n in [100, 1000, 10_000] {
    let label = format!("{n} folds");
    let mut group = c.benchmark_group("scaling");

    group.throughput(Throughput::Elements(n as u64));

    group.bench_function(&label, |b| {
      let (values, pairs) = make_int_pairs(n);
      let mut interner = Interner::new();
      let mut fold = ConstFold::new(&values, &mut interner);

      b.iter(|| {
        for &(a, b_id) in &pairs {
          black_box(fold.fold_binop(BinOp::Add, a, b_id, Span::ZERO, U64));
        }
      })
    });

    group.finish();
  }
}

criterion_group!(
  benches,
  bench_binop,
  bench_simplify,
  bench_width,
  bench_scaling,
);

criterion_main!(benches);
