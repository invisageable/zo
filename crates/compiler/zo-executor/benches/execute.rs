//!```
//! cargo bench -p zo-executor --bench execute --quiet
//! ```

use zo_executor::Executor;
use zo_interner::Interner;
use zo_parser::Parser;
use zo_tokenizer::Tokenizer;
use zo_ty_checker::TyChecker;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};

use std::hint::black_box;

const HELLO_PROGRAM: &str = r#"
  fun main() {
    showln("hello world!");
  }
"#;

fn bench_executor_body<'a>(
  source: &'a str,
) -> impl FnMut(&mut criterion::Bencher) + 'a {
  move |b: &mut criterion::Bencher| {
    b.iter(|| {
      let mut interner = Interner::new();
      let tokenizer = Tokenizer::new(black_box(source), &mut interner);
      let tokenization = tokenizer.tokenize();

      let parser = Parser::new(&tokenization, source);
      let parsing = parser.parse();

      let mut ty_checker = TyChecker::new();

      let executor = Executor::new(
        &parsing.tree,
        &mut interner,
        &tokenization.literals,
        &mut ty_checker,
      );

      black_box(executor.execute());
    })
  }
}

/// Instruction-dense program: each function emits ~18 SIR
/// instructions (consts, loads, binops, stores, return), so
/// `n` functions drive ~18·n `emit`s — the right workload to
/// measure per-instruction analyze cost (e.g. the span side
/// array) rather than the near-empty `hello` program.
fn dense_program(n: usize) -> String {
  let mut source = String::with_capacity(n * 96);

  for i in 0..n {
    source.push_str(&format!(
      "fun f{i}() -> int {{\n  \
         imu a: int = {i};\n  \
         imu b: int = a + {i};\n  \
         imu c: int = a * b - {i};\n  \
         c + a + b\n}}\n"
    ));
  }

  source.push_str("fun main() {}\n");
  source
}

fn bench_executor_dense(c: &mut Criterion) {
  let source = dense_program(1000);
  let numlines = source.lines().count() as u64;

  let mut group = c.benchmark_group("executor_dense");
  group.throughput(Throughput::Elements(numlines));
  group.bench_function("1000 fns", |b| {
    // Tokenize + parse in the (untimed) setup so the measured
    // routine is execution (analyze) only — that is where the
    // span side array is populated.
    b.iter_batched(
      || {
        let mut interner = Interner::new();
        let tokenization = Tokenizer::new(&source, &mut interner).tokenize();
        let parsing = Parser::new(&tokenization, &source).parse();

        (interner, tokenization, parsing, TyChecker::new())
      },
      |(mut interner, tokenization, parsing, mut ty_checker)| {
        let executor = Executor::new(
          &parsing.tree,
          &mut interner,
          &tokenization.literals,
          &mut ty_checker,
        );

        black_box(executor.execute());
      },
      criterion::BatchSize::SmallInput,
    )
  });
  group.finish();
}

fn bench_executor_hello(c: &mut Criterion) {
  let numbytes = HELLO_PROGRAM.len() as u64;
  let numlines = HELLO_PROGRAM.lines().count() as u64;

  {
    let mut group = c.benchmark_group("executor_bytes");
    group.throughput(Throughput::Bytes(numbytes));
    group.bench_function("`hello` program", bench_executor_body(HELLO_PROGRAM));
    group.finish();
  }
  {
    let mut group = c.benchmark_group("executor_lines");
    group.throughput(Throughput::Elements(numlines));
    group.bench_function("`hello` program", bench_executor_body(HELLO_PROGRAM));
    group.finish();
  }
}

criterion_group!(benches, bench_executor_hello, bench_executor_dense);
criterion_main!(benches);
