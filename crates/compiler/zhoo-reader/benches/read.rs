use zhoo_reader::reader;
use zhoo_session::session::Session;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_read(c: &mut Criterion) {
  let mut session = Session::default();

  session.settings.input = "../zhoo-notes/samples/main.zo".into();

  c.bench_function("read", |b| {
    b.iter(|| match reader::read(black_box(&mut session)) {
      Ok(_) => {}
      Err(error) => panic!("{error}"),
    });
  });
}

criterion_group!(benches, bench_read);
criterion_main!(benches);
