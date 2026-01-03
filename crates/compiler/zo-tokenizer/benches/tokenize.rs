//! ```sh
//! cargo bench --package zo-tokenizer --bench tokenize
//! ```

use zo_tokenizer::Tokenizer;

use criterion::{
  BenchmarkId, Criterion, Throughput, criterion_group, criterion_main,
};

use std::hint::black_box;

fn generate_complex_template(components: usize) -> String {
  let mut code = String::new();

  for i in 0..components {
    code.push_str(&format!(
      r#"
        fun component_{i}(props: Props) {{
          imu view ::= <div class="container-{i}">
            <header>
              <h1>{{props.title}}</h1>
              <nav>
                <ul>
                  {{props.items.map(|item| {{
                    <li key={{item.id}}>
                      <a href={{item.url}}>{{item.text}}</a>
                    </li>
                  }})}}
                </ul>
              </nav>
            </header>
            <main>
            {{if props.show_content {{
              <article>
                <h2>{{props.content_title}}</h2>
                <p>{{props.content_text}}</p>
                {{for i in 0..props.count {{
                  <span>Item {{i}}</span>
                }}}}
              </article>
            }} else {{
              <div class="empty">No content</div>
            }}}}
            </main>
            <footer>
              <p>© 2024 Component {i}</p>
            </footer>
          </div>;
        }}
      "#
    ));
  }

  code
}

fn generate_mixed_code(size: usize) -> String {
  let mut code = String::new();

  for i in 0..size {
    if i % 3 == 0 {
      code.push_str(&format!(
        r#"
          fun ui_component_{i}() {{
            ::= <div>
              <h1>Component {i}</h1>
              {{if condition {{
                <p>True branch</p>
              }} else {{
                <p>False branch</p>
              }}}}
            </div>;
          }}
        "#
      ));
    } else {
      code.push_str(&format!(
        r#"
          fun calculate_{i}(x: i32, y: i32): i32 {{
            imu result := x + y * {i};

            if result > 100 {{
              return result - 50;
            }} else {{
              return result + 25;
            }}
          }}
        "#
      ));
    }
  }

  code
}

fn bench_body_tokenizer() -> impl FnMut(&mut criterion::Bencher, &String) {
  move |b: &mut criterion::Bencher, source: &String| {
    b.iter(|| {
      let tokenizer = Tokenizer::new(black_box(source));
      let tokenization = tokenizer.tokenize();

      black_box(tokenization.tokens.len());
    })
  }
}

fn benchmark_templates(c: &mut Criterion) {
  for size in [10, 50, 100] {
    let code = generate_complex_template(size);
    let bytes = code.len() as u64;
    let elements = code.lines().count() as u64;

    {
      let mut group = c.benchmark_group("template_heavy_bytes");
      group.throughput(Throughput::Bytes(bytes));
      group.bench_with_input(
        BenchmarkId::new("original", size),
        &code,
        bench_body_tokenizer(),
      );
      group.finish();
    }
    {
      let mut group = c.benchmark_group("template_heavy_lines");
      group.throughput(Throughput::Elements(elements));
      group.bench_with_input(
        BenchmarkId::new("original", size),
        &code,
        bench_body_tokenizer(),
      );
      group.finish();
    }
  }
}

fn benchmark_mixed_code(c: &mut Criterion) {
  for size in [20, 100, 200] {
    let code = generate_mixed_code(size);
    let bytes = code.len() as u64;
    let elements = code.lines().count() as u64;

    {
      let mut group = c.benchmark_group("mixed_code_bytes");
      group.throughput(Throughput::Bytes(bytes));
      group.bench_with_input(
        BenchmarkId::new("original", size),
        &code,
        bench_body_tokenizer(),
      );
      group.finish();
    }
    {
      let mut group = c.benchmark_group("mixed_code_lines");
      group.throughput(Throughput::Elements(elements));
      group.bench_with_input(
        BenchmarkId::new("original", size),
        &code,
        bench_body_tokenizer(),
      );
      group.finish();
    }
  }
}

fn benchmark_mode_transitions(c: &mut Criterion) {
  let size = 20;

  let code = r#"
    fun render(): </> {
      imu div: </> ::= <div>Text</div>;
      imu x := 42;
      imu p ::= <p>{x}</p>;

      for i in 0..10 {
        imu span ::= <span>{i}</span>;
      }

      imu footer ::= <footer>Done</footer>;
    }
  "#
  .repeat(size);

  let bytes = code.len() as u64;
  let elements = code.lines().count() as u64;

  {
    let mut group = c.benchmark_group("mode_transitions_bytes");
    group.throughput(Throughput::Bytes(bytes));
    group.bench_with_input(
      BenchmarkId::new("original", size),
      &code,
      bench_body_tokenizer(),
    );
    group.finish();
  }
  {
    let mut group = c.benchmark_group("mode_transitions_lines");
    group.throughput(Throughput::Elements(elements));
    group.bench_with_input(
      BenchmarkId::new("original", size),
      &code,
      bench_body_tokenizer(),
    );
    group.finish();
  }
}

criterion_group!(
  benches,
  benchmark_templates,
  benchmark_mixed_code,
  benchmark_mode_transitions
);

criterion_main!(benches);
