//! ```
//! cargo bench -p zo-ty-checker --bench tycheck --quiet
//! ```

use zo_interner::Interner;
use zo_span::Span;
use zo_ty::{Mutability, Ty};
use zo_ty_checker::TyChecker;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};

use std::hint::black_box;

// === UNIFICATION ===

fn bench_unify_concrete(c: &mut Criterion) {
  let mut group = c.benchmark_group("unify");

  group.bench_function("concrete same type", |b| {
    b.iter(|| {
      let mut checker = TyChecker::new();
      let int_ty = checker.s32_type();

      for _ in 0..1000 {
        black_box(checker.unify(int_ty, int_ty, Span::ZERO));
      }
    })
  });

  group.bench_function("infer var → concrete", |b| {
    b.iter(|| {
      let mut checker = TyChecker::new();
      let int_ty = checker.s32_type();

      for _ in 0..1000 {
        let var = checker.fresh_var();

        black_box(checker.unify(var, int_ty, Span::ZERO));
      }
    })
  });

  group.bench_function("function types (3 params)", |b| {
    b.iter(|| {
      let mut checker = TyChecker::new();

      for _ in 0..1000 {
        let a = checker.fresh_var();
        let b = checker.fresh_var();
        let cv = checker.fresh_var();
        let ret = checker.fresh_var();
        let int_ty = checker.s32_type();

        let f1 = checker.ty_table.intern_fun(vec![a, b, cv], ret);
        let t1 = checker.intern_ty(Ty::Fun(f1));

        let f2 = checker
          .ty_table
          .intern_fun(vec![int_ty, int_ty, int_ty], int_ty);
        let t2 = checker.intern_ty(Ty::Fun(f2));

        black_box(checker.unify(t1, t2, Span::ZERO));
      }
    })
  });

  group.bench_function("substitution chain (depth 20)", |b| {
    b.iter(|| {
      let mut checker = TyChecker::new();
      let mut vars = Vec::with_capacity(20);

      for _ in 0..20 {
        vars.push(checker.fresh_var());
      }

      for i in 0..vars.len() - 1 {
        checker.unify(vars[i], vars[i + 1], Span::ZERO);
      }

      let int_ty = checker.s32_type();

      checker.unify(vars[19], int_ty, Span::ZERO);

      // Resolve from head — exercises path compression.
      black_box(checker.resolve_id(vars[0]));
    })
  });

  group.finish();
}

// === INTERNING ===

fn bench_intern(c: &mut Criterion) {
  let mut group = c.benchmark_group("intern");

  group.bench_function("primitive lookup (cached)", |b| {
    b.iter(|| {
      let mut checker = TyChecker::new();

      for _ in 0..10_000 {
        black_box(checker.s32_type());
        black_box(checker.bool_type());
        black_box(checker.str_type());
      }
    })
  });

  group.bench_function("fresh_var (10k)", |b| {
    b.iter(|| {
      let mut checker = TyChecker::new();

      for _ in 0..10_000 {
        black_box(checker.fresh_var());
      }
    })
  });

  group.bench_function("compound types (fun+array+ref)", |b| {
    b.iter(|| {
      let mut checker = TyChecker::new();
      let int_ty = checker.s32_type();

      for _ in 0..1000 {
        let fid = checker.ty_table.intern_fun(vec![int_ty], int_ty);

        black_box(checker.intern_ty(Ty::Fun(fid)));

        let aid = checker.ty_table.intern_array(int_ty, None);

        black_box(checker.intern_ty(Ty::Array(aid)));

        let rid = checker.ty_table.intern_ref(Mutability::No, int_ty);

        black_box(checker.intern_ty(Ty::Ref(rid)));
      }
    })
  });

  group.finish();
}

// === SCOPE MANAGEMENT ===

fn bench_scope(c: &mut Criterion) {
  let mut group = c.benchmark_group("scope");

  group.bench_function("push/pop empty (1k depth)", |b| {
    b.iter(|| {
      let mut checker = TyChecker::new();

      for _ in 0..1000 {
        checker.push_scope();
      }

      for _ in 0..1000 {
        checker.pop_scope();
      }

      black_box(&checker);
    })
  });

  group.bench_function("push/bind/pop (100 depth, 10 bindings)", |b| {
    let mut interner = Interner::new();

    let names: Vec<_> = (0..10)
      .map(|i| interner.intern(&format!("var_{i}")))
      .collect();

    b.iter(|| {
      let mut checker = TyChecker::new();
      let int_ty = checker.s32_type();

      for _ in 0..100 {
        checker.push_scope();

        for &name in &names {
          checker.bind_var(name, int_ty);
        }
      }

      for _ in 0..100 {
        checker.pop_scope();
      }

      black_box(&checker);
    })
  });

  group.bench_function("shadow + restore (deep)", |b| {
    let mut interner = Interner::new();
    let x = interner.intern("x");

    b.iter(|| {
      let mut checker = TyChecker::new();
      let int_ty = checker.s32_type();
      let bool_ty = checker.bool_type();

      checker.bind_var(x, int_ty);

      for _ in 0..100 {
        checker.push_scope();
        checker.bind_var(x, bool_ty);
      }

      for _ in 0..100 {
        checker.pop_scope();
      }

      // Must be restored to int.
      black_box(checker.lookup_var(x));
    })
  });

  group.finish();
}

// === LET-POLYMORPHISM ===

fn bench_polymorphism(c: &mut Criterion) {
  let mut group = c.benchmark_group("polymorphism");

  group.bench_function("generalize + instantiate (1k)", |b| {
    b.iter(|| {
      let mut checker = TyChecker::new();

      checker.push_scope();

      let alpha = checker.fresh_var();
      let fid = checker.ty_table.intern_fun(vec![alpha], alpha);
      let fun_ty = checker.intern_ty(Ty::Fun(fid));

      checker.pop_scope();

      let scheme = checker.generalize(fun_ty);

      for _ in 0..1000 {
        black_box(checker.instantiate(&scheme));
      }
    })
  });

  group.bench_function("generalize 5-param function", |b| {
    b.iter(|| {
      let mut checker = TyChecker::new();

      checker.push_scope();

      let vars: Vec<_> = (0..5).map(|_| checker.fresh_var()).collect();

      let ret = checker.fresh_var();
      let fid = checker.ty_table.intern_fun(vars, ret);
      let fun_ty = checker.intern_ty(Ty::Fun(fid));

      checker.pop_scope();

      black_box(checker.generalize(fun_ty));
    })
  });

  group.finish();
}

// === SCALING: TYPE INFERENCE WORKLOAD ===

fn bench_scaling(c: &mut Criterion) {
  for n in [100, 1000, 10_000] {
    let label = format!("{n} unifications");

    let mut group = c.benchmark_group("scaling");

    group.throughput(Throughput::Elements(n as u64));

    group.bench_function(&label, |b| {
      b.iter(|| {
        let mut checker = TyChecker::new();
        let int_ty = checker.s32_type();

        for _ in 0..n {
          let var = checker.fresh_var();

          checker.unify(var, int_ty, Span::ZERO);
        }

        black_box(&checker);
      })
    });

    group.finish();
  }
}

criterion_group!(
  benches,
  bench_unify_concrete,
  bench_intern,
  bench_scope,
  bench_polymorphism,
  bench_scaling,
);

criterion_main!(benches);
