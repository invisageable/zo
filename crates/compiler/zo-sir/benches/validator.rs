//! SIR validator benchmark.
//!
//! ```
//! cargo bench -p zo-sir --bench validator --quiet
//! ```
//!
//! Measures the cost of `validate` on synthetic insn streams
//! of varying sizes so we can decide whether it's cheap
//! enough for release-mode wiring. The streams mimic
//! realistic shapes: a pair of `ConstInt` + `BinOp` per
//! "line" of user code.

use zo_interner::Symbol;
use zo_sir::{BinOp, Insn, validate};
use zo_ty::TyId;
use zo_value::{FunctionKind, Pubness, ValueId};

use criterion::{Criterion, Throughput, criterion_group, criterion_main};

use std::hint::black_box;

/// Builds a synthetic SIR stream with `n` binop "lines":
/// `ConstInt, ConstInt, BinOp` × n, all tagged `s32`. A
/// realistic proxy for a simple function body.
fn make_binop_stream(n: u32) -> Vec<Insn> {
  let mut insns = Vec::with_capacity(3 * n as usize + 2);

  insns.push(Insn::FunDef {
    name: Symbol(1),
    params: Vec::new(),
    return_ty: TyId(1),
    body_start: 1,
    kind: FunctionKind::UserDefined,
    pubness: Pubness::No,
    mut_self: false,
  });

  for i in 0..n {
    let lhs = ValueId(3 * i);
    let rhs = ValueId(3 * i + 1);
    let dst = ValueId(3 * i + 2);

    insns.push(Insn::ConstInt {
      dst: lhs,
      value: 1,
      ty_id: TyId(8),
    });
    insns.push(Insn::ConstInt {
      dst: rhs,
      value: 2,
      ty_id: TyId(8),
    });
    insns.push(Insn::BinOp {
      dst,
      op: BinOp::Add,
      lhs,
      rhs,
      ty_id: TyId(8),
    });
  }

  insns.push(Insn::Return {
    value: None,
    ty_id: TyId(1),
  });

  insns
}

fn bench_validate(c: &mut Criterion) {
  let mut group = c.benchmark_group("validate");

  for size in [64u32, 1024, 10_000] {
    let insns = make_binop_stream(size);

    group.throughput(Throughput::Elements(insns.len() as u64));
    group.bench_function(format!("{size}_binop_lines"), |b| {
      b.iter(|| {
        black_box(validate(black_box(&insns)));
      })
    });
  }

  group.finish();
}

criterion_group!(benches, bench_validate);
criterion_main!(benches);
