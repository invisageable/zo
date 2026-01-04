//!```
//! cargo bench -p zo-parser --bench parse
//! ```

use zo_parser::Parser;
use zo_tokenizer::Tokenizer;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};

use std::hint::black_box;
use std::time::Duration;

const SIMPLE_PROGRAM: &str = r#"
  fun main() -> int {
    imu x := 10;
    imu y := 20;

    return x + y;
  }
"#;

const COMPLEX_PROGRAM: &str = r#"
  fun fibonacci(n: int) -> int {
    if n <= 1 {
      return n;
    }

    return fibonacci(n - 1) + fibonacci(n - 2);
  }

  fun factorial(n: u32) -> u32 {
    mut result: int = 1;

    for i := 2..n+1 {
      result *= i;
    }

    return result;
  }

  fun main() -> int {
    imu fib: int = fibonacci(10);
    imu fact: int = factorial(5);

    return fib + fact;
  }
"#;

fn generate_code(num_functions: usize) -> String {
  let mut program = String::with_capacity(num_functions * 200);

  for i in 0..num_functions {
    program.push_str(&format!(
      r#"
        fun function_{i}(x: s32, y: s32) -> s32 {{
          imu a := x + y;
          imu b := x * y;
          imu c := a - b;
          
          if c > 0 {{
            return c;
          }} else {{
            return -c;
          }}
        }}
      "#,
      i = i
    ));
  }

  program.push_str(
    r#"
    fun main() -> int {
      imu result: int = 0;
      mut result: int = 0;

      for i := 0..100 {
        result = result + function_0(i, i + 1);
      }

      return result;
    }
  "#,
  );

  program
}

fn bench_parser_body<'a>(
  source: &'a str,
) -> impl FnMut(&mut criterion::Bencher) + 'a {
  move |b: &mut criterion::Bencher| {
    b.iter(|| {
      let tokenizer = Tokenizer::new(black_box(source));
      let tokenization = tokenizer.tokenize();
      let parser = Parser::new(&tokenization, source);

      black_box(parser.parse());
    })
  }
}

fn bench_parser_simple(c: &mut Criterion) {
  let numbytes = SIMPLE_PROGRAM.len() as u64;
  let numlines = SIMPLE_PROGRAM.lines().count() as u64;

  {
    let mut group = c.benchmark_group("parser_bytes");
    group.throughput(Throughput::Bytes(numbytes));
    group.bench_function(
      "simple program (15 nodes)",
      bench_parser_body(SIMPLE_PROGRAM),
    );
    group.finish();
  }
  {
    let mut group = c.benchmark_group("parser_lines");
    group.throughput(Throughput::Elements(numlines));
    group.bench_function(
      "simple program (15 nodes)",
      bench_parser_body(SIMPLE_PROGRAM),
    );
    group.finish();
  }
}

fn bench_parser_complex(c: &mut Criterion) {
  let numbytes = COMPLEX_PROGRAM.len() as u64;
  let numlines = COMPLEX_PROGRAM.lines().count() as u64;

  {
    let mut group = c.benchmark_group("parser_bytes");
    group.throughput(Throughput::Bytes(numbytes));
    group.bench_function(
      "complex program (_ nodes)",
      bench_parser_body(COMPLEX_PROGRAM),
    );
    group.finish();
  }
  {
    let mut group = c.benchmark_group("parser_lines");
    group.throughput(Throughput::Elements(numlines));
    group.bench_function(
      "complex program (_ nodes)",
      bench_parser_body(COMPLEX_PROGRAM),
    );
    group.finish();
  }
}

fn bench_medium(c: &mut Criterion) {
  let medium_code = generate_code(100);
  let numbytes = medium_code.len() as u64;
  let numlines = medium_code.lines().count() as u64;

  {
    let mut group = c.benchmark_group("parser_bytes");
    group.throughput(Throughput::Bytes(numbytes));
    group.bench_function(
      "medium program (100 functions)",
      bench_parser_body(&medium_code),
    );
    group.finish();
  }
  {
    let mut group = c.benchmark_group("parser_lines");
    group.throughput(Throughput::Elements(numlines));
    group.bench_function(
      "medium program (100 functions)",
      bench_parser_body(&medium_code),
    );
    group.finish();
  }
}

fn bench_parser_throughput(c: &mut Criterion) {
  let large_code = generate_code(1000);
  let numbytes = large_code.len() as u64;
  let numlines = large_code.lines().count() as u64;

  {
    let mut group = c.benchmark_group("parser_bytes");
    group.throughput(Throughput::Bytes(numbytes));
    group.bench_function(
      "large program (1000 functions)",
      bench_parser_body(&large_code),
    );
    group.finish();
  }
  {
    let mut group = c.benchmark_group("parser_lines");
    group.throughput(Throughput::Elements(numlines));
    group.bench_function(
      "large program (1000 functions)",
      bench_parser_body(&large_code),
    );
    group.finish();
  }
}

fn bench_mixed_multiple_sizes(c: &mut Criterion) {
  for size in [100, 500, 1000, 5000] {
    let code = generate_code(size);
    let numbytes = code.len() as u64;
    let numlines = code.lines().count() as u64;

    {
      let mut group = c.benchmark_group("parser_bytes");

      group
        .sample_size(20)
        .measurement_time(Duration::from_secs(10));

      group.throughput(Throughput::Bytes(numbytes));
      group.bench_function(
        format!("mixed code ({size} functions)"),
        bench_parser_body(&code),
      );
      group.finish();
    }
    {
      let mut group = c.benchmark_group("parser_lines");

      group
        .sample_size(20)
        .measurement_time(Duration::from_secs(10));

      group.throughput(Throughput::Elements(numlines));
      group.bench_function(
        format!("mixed code ({size} functions)"),
        bench_parser_body(&code),
      );
      group.finish();
    }
  }
}

criterion_group!(
  benches,
  bench_parser_simple,
  bench_parser_complex,
  bench_parser_throughput,
  bench_medium,
  bench_mixed_multiple_sizes,
);

criterion_main!(benches);
