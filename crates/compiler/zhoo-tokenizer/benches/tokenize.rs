use zhoo_session::session::Session;
use zhoo_tokenizer::tokenizer;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_tokenize(c: &mut Criterion) {
  let mut session = Session::default();
  let source = "fun main() { imu x := 4 + 8 * 268 / 2; }".as_bytes();

  c.bench_function(format!("tokenize").as_str(), |b| {
    b.iter(|| {
      tokenizer::tokenize(
        black_box(&mut session),
        black_box(&source),
      )
      .unwrap();
    });
  });
}

criterion_group!(benches, bench_tokenize);
criterion_main!(benches);
