use zhoo_parser::parser;
use zhoo_reader::reader;
use zhoo_session::session::Session;
use zhoo_tokenizer::tokenizer;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_parse(c: &mut Criterion) {
  let mut session = Session::default();

  session.settings.input = "../zhoo-notes/samples/bench/ast.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, black_box(&source)).unwrap();

  c.bench_function("parse", |b| {
    b.iter(|| {
      parser::parse(&mut session, black_box(&tokens)).unwrap();
    });
  });
}

criterion_group!(benches, bench_parse);
criterion_main!(benches);
