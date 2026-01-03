//!```
//! cargo bench -p zo-executor --bench execute --quiet
//! ```

use zo_executor::Executor;
use zo_parser::Parser;
use zo_tokenizer::Tokenizer;

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
      let tokenizer = Tokenizer::new(black_box(source));
      let tokenization = tokenizer.tokenize();

      let parser = Parser::new(&tokenization, source);
      let parsing = parser.parse();

      let executor = Executor::new(
        &parsing.tree,
        &tokenization.interner,
        &tokenization.literals,
      );

      black_box(executor.execute());
    })
  }
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

criterion_group!(benches, bench_executor_hello,);

criterion_main!(benches);
