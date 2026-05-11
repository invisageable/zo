//! ```
//! cargo bench -p zo-codegen --bench generate
//! ```
//!
//! Whole-pipeline throughput bench: tokenize → parse →
//! analyze → codegen, on synthetic zo source at multiple
//! sizes. Reports both bytes/s and lines/s so the headline
//! 10M LoC / 5M LoC / 5M LoC targets can be read directly
//! against the AOT phases.

use zo_analyzer::Analyzer;
use zo_codegen::codegen::Codegen;
use zo_codegen_backend::Target;
use zo_interner::Interner;
use zo_parser::Parser;
use zo_tokenizer::Tokenizer;
use zo_ty_checker::TyChecker;

use criterion::{
  BenchmarkId, Criterion, Throughput, criterion_group, criterion_main,
};

use std::hint::black_box;
use std::time::Duration;

/// Generate `n` small functions plus a `main`. Each function
/// is ~10 lines (signature, two locals, an `if`/`else` with
/// returns) — enough shape to exercise binops, branches, and
/// inter-function calls without bloating the analyzer past
/// what one logical line of work normally costs.
///
/// The prefix is `gen_` rather than the obvious `f`: `f32` /
/// `f64` are reserved type keywords, so a `fun f32(...)`
/// would tokenize as a type, not an identifier, and the
/// generated source would fail to parse at i = 32 and i = 64.
fn synth(num_functions: usize) -> String {
  let mut src = String::with_capacity(num_functions * 200);

  src.push_str("fun gen_0(x: int) -> int {\n  return x + 1;\n}\n\n");

  for i in 1..num_functions {
    src.push_str(&format!(
      "fun gen_{i}(x: int) -> int {{\n  imu y: int = gen_{prev}(x);\n  imu z: int = y * 2 - 1;\n  if z > 0 {{\n    return z;\n  }} else {{\n    return -z;\n  }}\n}}\n\n",
      i = i,
      prev = i - 1
    ));
  }

  src.push_str(&format!(
    "fun main() {{\n  imu r: int = gen_{}(1);\n  showln(r);\n}}\n",
    num_functions - 1
  ));

  src
}

/// Run the full AOT pipeline once. Returns nothing — black-
/// boxed inside the bench so LLVM doesn't fold the work away.
fn run_pipeline(source: &str) {
  let mut interner = Interner::new();
  let mut ty_checker = TyChecker::new();

  let tokenization = Tokenizer::new(source, &mut interner).tokenize();
  let parsing = Parser::new(&tokenization, source).parse();

  let analyzer = Analyzer::new(
    &parsing.tree,
    &mut interner,
    &tokenization.literals,
    &mut ty_checker,
  );
  let result = analyzer.analyze();

  let codegen = Codegen::new(Target::Arm64AppleDarwin);
  let type_view = Some((ty_checker.tys(), &ty_checker.ty_table));
  let artifact = codegen.generate_artifact(&interner, &result.sir, type_view);

  black_box(artifact);
}

fn bench_pipeline(c: &mut Criterion) {
  // Sizes chosen to show throughput scaling: 100 fns ≈ 1K
  // lines, 500 fns ≈ 5K lines, 1000 fns ≈ 10K lines.
  // 5000 fns ≈ 50K lines stresses cache locality at scale.
  for num_fns in [100usize, 500, 1000, 5000] {
    let source = synth(num_fns);
    let bytes = source.len() as u64;
    let lines = source.lines().count() as u64;

    {
      let mut group = c.benchmark_group("pipeline_lines");

      group
        .sample_size(10)
        .measurement_time(Duration::from_secs(15))
        .throughput(Throughput::Elements(lines));

      group.bench_with_input(
        BenchmarkId::from_parameter(format!("{num_fns}fns/{lines}lines")),
        &source,
        |b, src| b.iter(|| run_pipeline(black_box(src))),
      );

      group.finish();
    }

    {
      let mut group = c.benchmark_group("pipeline_bytes");

      group
        .sample_size(10)
        .measurement_time(Duration::from_secs(15))
        .throughput(Throughput::Bytes(bytes));

      group.bench_with_input(
        BenchmarkId::from_parameter(format!("{num_fns}fns/{bytes}B")),
        &source,
        |b, src| b.iter(|| run_pipeline(black_box(src))),
      );

      group.finish();
    }
  }
}

criterion_group!(benches, bench_pipeline);
criterion_main!(benches);
