//! ```rs
//! cargo bench --package zo-buffer --bench bytes
//! ```

use zo_buffer::Buffer;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};

use std::hint::black_box;
use std::io::{BufWriter, Write};

const ITERATIONS: u32 = 1000;

fn writer_workload(writer: &mut Buffer) {
  for i in 0..ITERATIONS {
    writer.indent();
    writer.char(b'%');
    writer.u32(i);
    writer.str(" = add nsw i32 %");
    writer.u32(i * 2);
    writer.str(", %");
    writer.u32(i * 3);
    writer.newline();
  }
}

fn std_bufwriter_workload(writer: &mut impl Write) {
  for i in 0..ITERATIONS {
    write!(writer, "  %{} = add nsw i32 %{}, %{}\n", i, i * 2, i * 3).unwrap();
  }
}

fn writer_benchmark(c: &mut Criterion) {
  let mut counter_buffer = Vec::new();

  std_bufwriter_workload(&mut counter_buffer);

  let numbytes = counter_buffer.len() as u64;
  let numlines = ITERATIONS as u64;

  {
    let mut group = c.benchmark_group("writer_throughput");
    group.throughput(Throughput::Bytes(numbytes));
    group.bench_function("zo_writer_bytes_per_sec", |b| {
      b.iter(|| {
        let mut buffer = Buffer::new();

        writer_workload(black_box(&mut buffer));
        black_box(buffer.finish());
      })
    });
    group.finish();
  }
  {
    let mut group = c.benchmark_group("writer_throughput");
    group.throughput(Throughput::Elements(numlines));
    group.bench_function("zo_writer_bytes_per_sec", |b| {
      b.iter(|| {
        let mut buffer = Buffer::new();

        writer_workload(black_box(&mut buffer));
        black_box(buffer.finish());
      })
    });
    group.finish();
  }

  {
    let mut group = c.benchmark_group("writer_throughput");
    group.throughput(Throughput::Bytes(numbytes));
    group.bench_function("std_bufwriter_bytes_per_sec", |b| {
      b.iter(|| {
        let mut buffer = Vec::new();

        {
          let mut buf_writer = BufWriter::new(buffer);

          std_bufwriter_workload(black_box(&mut buf_writer));

          buffer = buf_writer.into_inner().unwrap();
        }

        black_box(buffer);
      })
    });

    group.finish();
  }
  {
    let mut group = c.benchmark_group("writer_throughput");
    group.throughput(Throughput::Elements(numlines));
    group.bench_function("std_bufwriter_bytes_per_sec", |b| {
      b.iter(|| {
        let mut buffer = Vec::new();

        {
          let mut buf_writer = BufWriter::new(buffer);

          std_bufwriter_workload(black_box(&mut buf_writer));

          buffer = buf_writer.into_inner().unwrap();
        }

        black_box(buffer);
      })
    });

    group.finish();
  }
}

criterion_group!(benches, writer_benchmark);
criterion_main!(benches);
