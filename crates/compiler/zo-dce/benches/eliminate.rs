//! ```
//! cargo bench -p zo-dce --bench eliminate --quiet
//! ```

use zo_dce::Dce;
use zo_interner::Interner;
use zo_sir::{Insn, Sir};
use zo_ty::TyId;
use zo_value::{FunctionKind, Pubness, ValueId};

use criterion::{Criterion, Throughput, criterion_group, criterion_main};

use std::hint::black_box;

fn make_fun(
  name: zo_interner::Symbol,
  calls: Vec<zo_interner::Symbol>,
) -> Vec<Insn> {
  let mut insns = vec![Insn::FunDef {
    name,
    params: vec![],
    return_ty: TyId(1),
    body_start: 0,
    kind: FunctionKind::UserDefined,
    pubness: Pubness::No,
  }];

  for callee in calls {
    insns.push(Insn::Call {
      dst: ValueId(0),
      name: callee,
      args: vec![],
      ty_id: TyId(1),
    });
  }

  insns.push(Insn::Return {
    value: None,
    ty_id: TyId(1),
  });

  insns
}

fn bench_eliminate(c: &mut Criterion) {
  let mut group = c.benchmark_group("eliminate");

  // Small: 10 functions, 5 reachable.
  group.bench_function("10 funs, 5 dead", |b| {
    let mut interner = Interner::new();
    let names: Vec<_> =
      (0..10).map(|i| interner.intern(&format!("f{i}"))).collect();
    let main = interner.intern("main");

    b.iter(|| {
      let mut insns = Vec::new();

      for &name in &names {
        insns.extend(make_fun(name, vec![]));
      }

      // main calls f5..f9.
      let calls: Vec<_> = names[5..].to_vec();

      insns.extend(make_fun(main, calls));

      let mut sir = Sir {
        instructions: insns,
        next_value_id: 100,
        next_label_id: 0,
      };

      Dce::new(&mut sir, main, &interner).eliminate();
      black_box(&sir);
    })
  });

  // Medium: 100 functions, chain of 50 reachable.
  group.bench_function("100 funs, 50 dead", |b| {
    let mut interner = Interner::new();
    let names: Vec<_> = (0..100)
      .map(|i| interner.intern(&format!("f{i}")))
      .collect();
    let main = interner.intern("main");

    b.iter(|| {
      let mut insns = Vec::new();

      // Dead functions (f0..f49).
      for &name in &names[..50] {
        insns.extend(make_fun(name, vec![]));
      }

      // Reachable chain: f50 → f51 → ... → f99.
      for i in 50..100 {
        let calls = if i < 99 { vec![names[i + 1]] } else { vec![] };

        insns.extend(make_fun(names[i], calls));
      }

      insns.extend(make_fun(main, vec![names[50]]));

      let mut sir = Sir {
        instructions: insns,
        next_value_id: 200,
        next_label_id: 0,
      };

      Dce::new(&mut sir, main, &interner).eliminate();
      black_box(&sir);
    })
  });

  // Large: 1000 functions, 500 dead.
  group.bench_function("1000 funs, 500 dead", |b| {
    let mut interner = Interner::new();
    let names: Vec<_> = (0..1000)
      .map(|i| interner.intern(&format!("f{i}")))
      .collect();
    let main = interner.intern("main");

    b.iter(|| {
      let mut insns = Vec::new();

      for &name in &names[..500] {
        insns.extend(make_fun(name, vec![]));
      }

      for i in 500..1000 {
        let calls = if i < 999 { vec![names[i + 1]] } else { vec![] };

        insns.extend(make_fun(names[i], calls));
      }

      insns.extend(make_fun(main, vec![names[500]]));

      let mut sir = Sir {
        instructions: insns,
        next_value_id: 2000,
        next_label_id: 0,
      };

      Dce::new(&mut sir, main, &interner).eliminate();
      black_box(&sir);
    })
  });

  group.finish();
}

fn bench_scaling(c: &mut Criterion) {
  for n in [100, 500, 1000] {
    let label = format!("{n} functions");
    let mut group = c.benchmark_group("scaling");

    group.throughput(Throughput::Elements(n as u64));

    group.bench_function(&label, |b| {
      let mut interner = Interner::new();
      let names: Vec<_> =
        (0..n).map(|i| interner.intern(&format!("f{i}"))).collect();
      let main = interner.intern("main");

      b.iter(|| {
        let mut insns = Vec::new();
        let half = n / 2;

        for &name in &names[..half] {
          insns.extend(make_fun(name, vec![]));
        }

        for i in half..n {
          let calls = if i < n - 1 {
            vec![names[i + 1]]
          } else {
            vec![]
          };

          insns.extend(make_fun(names[i], calls));
        }

        insns.extend(make_fun(main, vec![names[half]]));

        let mut sir = Sir {
          instructions: insns,
          next_value_id: (n * 2) as u32,
          next_label_id: 0,
        };

        Dce::new(&mut sir, main, &interner).eliminate();
        black_box(&sir);
      })
    });

    group.finish();
  }
}

criterion_group!(benches, bench_eliminate, bench_scaling);

criterion_main!(benches);
