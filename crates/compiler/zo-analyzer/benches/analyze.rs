// Performance benchmarks for Analyzer using real source code
// Target: >20M LoC/s throughput for the full compilation pipeline

use zo_analyzer::Analyzer;
use zo_parser::Parser;
use zo_tokenizer::Tokenizer;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};

use std::hint::black_box;

const SIMPLE_CODE: &str = r#"
  fun add(x: s32, y: s32) -> s32 {
    return x + y;
  }

  fun main() -> s32 {
    imu x := 10;
    imu y := 20;

    return add(x, y);
  }
"#;

const COMPLEX_CODE: &str = r#"
  fun max(a: s32, b: s32) -> s32 {
    -- if a > b {
      -- return a;
    -- } else {
      return b;
    -- }
  }

  fun fibonacci(n: s32) -> s32 {
    -- if n <= 1 {
      -- return n;
    -- }
    
    -- imu a := fibonacci(n - 1);
    -- imu b := fibonacci(n - 2);
    
    return a + b;
  }

  fun factorial(n: s32) -> s32 {
    mut result := 1;
    mut i := 2;
    
    -- while i <= n {
      -- result = result * i;
      -- i = i + 1;
    -- }
    
    return result;
  }

  fun main() -> s32 {
    imu fib := fibonacci(10);
    imu fact := factorial(5);
    imu maximum := max(fib, fact);

    return maximum;
  }
"#;

fn generate_realistic_code(num_functions: usize) -> String {
  let mut code = String::with_capacity(num_functions * 500);

  code.push_str(
    r#"
    fun abs(x: s32) -> s32 {
      -- if x < 0 {
        return -x;
      -- } else {
      --   return x;
      -- }
    }

    fun min(a: s32, b: s32) -> s32 {
      -- if a < b {
        return a;
      -- } else {
      --   return b;
      -- }
    }

    fun max(a: s32, b: s32) -> s32 {
      -- if a > b {
        return a;
      -- } else {
      --   return b;
      -- }
    }
  "#,
  );

  for i in 0..num_functions {
    code.push_str(&format!(
      r#"
        fun compute_{i}(x: s32, y: s32, z: s32) -> s32 {{
          -- variable declarations with type inference.
          imu sum := x + y + z;
          imu product := x * y * z;
          mut result := 0;
          
          -- Control flow with nested blocks.
          -- if sum > product {{
          --   {{
          --     imu temp := sum - product;
          --     result = abs(temp);
          --   }}
          -- }} else {{
          --   {{
          --     imu diff := product - sum;
          --     imu bounded := min(diff, 1000);
          --     result = max(bounded, 1);
          --   }}
          -- }}
          
          -- more computation.
          mut counter := 0;
          -- while counter < 10 {{
          --   result = result + counter;
          --   counter = counter + 1;
          -- }}
          
          -- function calls.
          imu final_result := min(result, 10000);

          return final_result;
        }}

        fun process_{i}(input: s32) -> s32 {{
          -- type inference challenges.
          imu a := input * 2;
          imu b := a + 10;
          imu c := compute_{i}(a, b, input);
          
          -- nested control flow.
          mut output := 0;

          -- if c > 100 {{
          --   if c > 1000 {{
          --     output = c / 10;
          --   }} else {{
          --     output = c / 2;
          --   }}
          -- }} else {{
          --   output = c * 2;
          -- }}
          
          return output;
        }}
      "#,
      i = i
    ));
  }

  code.push_str(&format!(
    r#"
      fun main() -> s32 {{
        mut total := 0;
        
        -- call some of the generated functions.
        {}
        
        return total;
      }}
    "#,
    (0..num_functions.min(10))
      .map(|i| format!("total = total + process_{}({})", i, i + 1))
      .collect::<Vec<_>>()
      .join(";\n        ")
  ));

  code
}

fn bench_analyzer<'a>(
  source: &'a str,
) -> impl FnMut(&mut criterion::Bencher) + 'a {
  move |b: &mut criterion::Bencher| {
    b.iter(|| {
      let tokenizer = Tokenizer::new(black_box(source));
      let tokenization = tokenizer.tokenize();

      let parser = Parser::new(&tokenization, source);
      let parsing = parser.parse();

      let analyzer = Analyzer::new(
        &parsing.tree,
        &tokenization.interner,
        &tokenization.literals,
      );

      black_box(analyzer.analyze());
    })
  }
}

fn bench_simple(c: &mut Criterion) {
  let numlines = SIMPLE_CODE.lines().count() as u64;
  let numbytes = SIMPLE_CODE.len() as u64;

  {
    let mut group = c.benchmark_group("analyzer_bytes");
    group.throughput(Throughput::Bytes(numbytes));
    group.bench_function("simple (~10 lines)", bench_analyzer(SIMPLE_CODE));
    group.finish();
  }
  {
    let mut group = c.benchmark_group("analyzer_lines");
    group.throughput(Throughput::Elements(numlines));
    group.bench_function("simple (~10 lines)", bench_analyzer(SIMPLE_CODE));
    group.finish();
  }
}

fn bench_complex(c: &mut Criterion) {
  let numlines = COMPLEX_CODE.lines().count() as u64;
  let numbytes = COMPLEX_CODE.len() as u64;

  {
    let mut group = c.benchmark_group("analyzer_bytes");
    group.throughput(Throughput::Bytes(numbytes));
    // group.bench_function("complex (~50 lines)", bench_analyzer(COMPLEX_CODE));
    group.finish();
  }
  {
    let mut group = c.benchmark_group("analyzer_lines");
    group.throughput(Throughput::Elements(numlines));
    // group.bench_function("complex (~50 lines)", bench_analyzer(COMPLEX_CODE));
    group.finish();
  }
}

fn bench_small(c: &mut Criterion) {
  let code = generate_realistic_code(10);
  let numlines = code.lines().count() as u64;
  let numbytes = code.len() as u64;

  {
    let mut group = c.benchmark_group("analyzer_bytes");
    group.throughput(Throughput::Bytes(numbytes));
    group.bench_function("small (~100 lines)", bench_analyzer(&code));
    group.finish();
  }
  {
    let mut group = c.benchmark_group("analyzer_lines");
    group.throughput(Throughput::Elements(numlines));
    group.bench_function("small (~100 lines)", bench_analyzer(&code));
    group.finish();
  }
}

fn bench_medium(c: &mut Criterion) {
  let code = generate_realistic_code(100);
  let numlines = code.lines().count() as u64;
  let numbytes = code.len() as u64;

  {
    let mut group = c.benchmark_group("analyzer_bytes");
    group.throughput(Throughput::Bytes(numbytes));
    group.bench_function("medium (~1,000 lines)", bench_analyzer(&code));
    group.finish();
  }
  {
    let mut group = c.benchmark_group("analyzer_lines");
    group.throughput(Throughput::Elements(numlines));
    group.bench_function("medium (~1,000 lines)", bench_analyzer(&code));
    group.finish();
  }
}

fn bench_large(c: &mut Criterion) {
  let source = generate_realistic_code(1000);
  let numlines = source.lines().count() as u64;
  let numbytes = source.len() as u64;

  {
    let mut group = c.benchmark_group("analyzer_bytes");
    group.throughput(Throughput::Bytes(numbytes));
    group.bench_function("large (~10,000 lines)", bench_analyzer(&source));
    group.finish();
  }
  {
    let mut group = c.benchmark_group("analyzer_lines");
    group.throughput(Throughput::Elements(numlines));
    group.bench_function("large (~10,000 lines)", bench_analyzer(&source));
    group.finish();
  }
}

fn bench_xlarge(c: &mut Criterion) {
  let source = generate_realistic_code(10000);
  let numlines = source.lines().count() as u64;
  let numbytes = source.len() as u64;

  {
    let mut group = c.benchmark_group("analyzer_bytes");
    group.throughput(Throughput::Bytes(numbytes));
    group.bench_function("xlarge (~100,000 lines)", bench_analyzer(&source));
    group.finish();
  }
  {
    let mut group = c.benchmark_group("analyzer_lines");
    group.throughput(Throughput::Elements(numlines));
    group.bench_function("xlarge (~100,000 lines)", bench_analyzer(&source));
    group.finish();
  }
}

fn bench_scaling(c: &mut Criterion) {
  for size in [100, 500, 1000, 5000] {
    let source = generate_realistic_code(size);
    let numlines = source.lines().count() as u64;
    let numbytes = source.len() as u64;

    {
      let mut group = c.benchmark_group("analyzer_bytes");
      group.throughput(Throughput::Bytes(numbytes));
      group.bench_function(
        format!("scaling {} functions (~{} bytes)", size, numbytes),
        bench_analyzer(&source),
      );
      group.finish();
    }
    {
      let mut group = c.benchmark_group("analyzer_lines");
      group.throughput(Throughput::Elements(numlines));
      group.bench_function(
        format!("scaling {} functions (~{} lines)", size, numlines),
        bench_analyzer(&source),
      );
      group.finish();
    }
  }
}

criterion_group!(
  benches,
  bench_simple,
  bench_complex,
  bench_small,
  bench_medium,
  bench_large,
  bench_xlarge,
  bench_scaling
);

criterion_main!(benches);
