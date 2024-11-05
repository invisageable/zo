// bench: `cargo bench --package zor-tokenizer`

use zor_session::session::Session;
use zor_token::token::TokenKind;
use zor_tokenizer::tokenizer::Tokenizer;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_tokenize_program_light<'source>(c: &mut Criterion) {
  let source = "imu foo: int := 1 + 2;";
  let mut session = Session::default();
  let mut tokenizer = Tokenizer::new(source, true, &mut session.interner);

  c.bench_function(format!("tokenize::program[light]").as_str(), |b| {
    b.iter(|| {
      while black_box(tokenizer.next().unwrap().kind) != TokenKind::Eof {}
    });
  });
}

criterion_group!(benches, bench_tokenize_program_light);
criterion_main!(benches);
