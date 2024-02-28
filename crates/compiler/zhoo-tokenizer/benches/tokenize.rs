use zhoo_reader::reader;
use zhoo_session::session::Session;
use zhoo_tokenizer::tokenizer;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_tokenize(c: &mut Criterion) {
  let mut session = Session::default();

  session.settings.input = "../zhoo-notes/samples/main.zo".into();

  let source = reader::read_file(&mut session).unwrap();

  c.bench_function("tokenize", |b| {
    b.iter(|| {
      tokenizer::tokenize(black_box(&mut session), black_box(&source)).unwrap();
    });
  });
}

criterion_group!(benches, bench_tokenize);
criterion_main!(benches);
