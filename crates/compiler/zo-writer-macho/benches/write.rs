use zo_writer_macho::{ARM64RelocationType, MachO, SymbolVisibility};

use criterion::{Criterion, criterion_group, criterion_main};

use std::hint::black_box;

fn bench_small_binary(c: &mut Criterion) {
  c.bench_function("small_binary_generation", |b| {
    b.iter(|| {
      let mut macho = MachO::new();

      macho.add_code(vec![0xd6, 0x5f, 0x03, 0xc0]); // ret
      black_box(macho.finish());
    });
  });
}

fn bench_medium_binary_with_symbols(c: &mut Criterion) {
  c.bench_function("medium_binary_with_symbols", |b| {
    b.iter(|| {
      let mut macho = MachO::new();

      // Add 1KB of code
      let mut code = Vec::new();

      for _ in 0..256 {
        code.extend_from_slice(&[0xd6, 0x5f, 0x03, 0xc0]); // ret
      }

      macho.add_code(code);

      // Add 100 symbols
      for i in 0..100 {
        macho.add_local_symbol(&format!("_sym_{i}"), 1, 0x400 + i * 4);
      }

      black_box(macho.finish());
    });
  });
}

fn bench_large_code_section(c: &mut Criterion) {
  c.bench_function("large_code_section_1mb", |b| {
    b.iter(|| {
      let mut macho = MachO::new();

      // Build 1MB of code
      let mut code = Vec::with_capacity(1024 * 1024);
      let ret = [0xd6, 0x5f, 0x03, 0xc0];

      for _ in 0..(1024 * 1024 / 4) {
        code.extend_from_slice(&ret);
      }

      macho.add_code(code);
      black_box(macho.finish());
    });
  });
}

fn bench_many_relocations(c: &mut Criterion) {
  c.bench_function("relocations_1000", |b| {
    b.iter(|| {
      let mut macho = MachO::new();

      // Add enough code for relocations
      let mut code = Vec::new();

      for _ in 0..1000 {
        code.extend_from_slice(&[0x00, 0x00, 0x00, 0x94]); // bl
      }

      macho.add_code(code);

      // Add 1000 relocations
      for i in 0..1000 {
        macho.add_text_relocation(
          (i * 4) as u32,
          &format!("_func_{i}"),
          ARM64RelocationType::Branch26,
          true,
        );
      }

      black_box(macho.finish());
    });
  });
}

fn bench_complex_binary(c: &mut Criterion) {
  c.bench_function("complex_binary_full_featured", |b| {
    b.iter(|| {
      let mut macho = MachO::new();

      // Code section
      let mut code = Vec::new();

      for _ in 0..256 {
        code.extend_from_slice(&[0x20, 0x00, 0x02, 0x8b]); // add x0, x1, x2
        code.extend_from_slice(&[0x83, 0x00, 0x05, 0xcb]); // sub x3, x4, x5
        code.extend_from_slice(&[0xd6, 0x5f, 0x03, 0xc0]); // ret
      }

      macho.add_code(code);

      // Data section
      let mut data = Vec::new();

      for i in 0..256 {
        data.extend_from_slice(&(i as u64).to_le_bytes());
      }

      macho.add_data(data);

      // Symbols
      for i in 0..50 {
        macho.add_function_symbol(
          &format!("_func_{i}"),
          1,
          0x400 + i * 12,
          true,
        );
        macho.add_data_symbol(
          &format!("_data_{i}"),
          2,
          0x8000 + i * 8,
          8,
          SymbolVisibility::Default,
        );
      }

      // Relocations
      for i in 0..50 {
        macho.add_text_relocation(
          (i * 12) as u32,
          &format!("_func_{i}"),
          ARM64RelocationType::Branch26,
          true,
        );
      }

      black_box(macho.finish());
    });
  });
}

fn bench_signature_generation(c: &mut Criterion) {
  c.bench_function("code_signature_generation", |b| {
    b.iter(|| {
      let mut macho = MachO::new();

      // Add some code
      let mut code = Vec::new();

      for _ in 0..1024 {
        code.extend_from_slice(&[0xd6, 0x5f, 0x03, 0xc0]); // ret
      }

      macho.add_code(code);
      black_box(macho.finish_with_signature());
    });
  });
}

fn bench_realistic_compiler_output(c: &mut Criterion) {
  c.bench_function("realistic_compiler_output", |b| {
    b.iter(|| {
      let mut macho = MachO::new();

      // Simulate a real program with ~10KB of varied code
      let mut code = Vec::new();

      // Function prologue (main)
      code.extend_from_slice(&[0xff, 0x83, 0x00, 0xd1]); // sub sp, sp, #32
      code.extend_from_slice(&[0xfd, 0x7b, 0x01, 0xa9]); // stp x29, x30, [sp, #16]
      code.extend_from_slice(&[0xfd, 0x43, 0x00, 0x91]); // add x29, sp, #16

      // Simulate 100 functions with calls between them
      for _ in 0..100 {
        // Function entry
        code.extend_from_slice(&[0xff, 0x83, 0x00, 0xd1]); // sub sp, sp, #32
        code.extend_from_slice(&[0xfd, 0x7b, 0x01, 0xa9]); // stp x29, x30, [sp, #16]

        // Some arithmetic operations
        for _ in 0..5 {
          code.extend_from_slice(&[0x20, 0x00, 0x02, 0x8b]); // add x0, x1, x2
          code.extend_from_slice(&[0x83, 0x00, 0x05, 0xcb]); // sub x3, x4, x5
          code.extend_from_slice(&[0xe6, 0x7c, 0x08, 0x9b]); // mul x6, x7, x8
        }

        // Load/store operations
        code.extend_from_slice(&[0x00, 0x00, 0x40, 0xf9]); // ldr x0, [x0]
        code.extend_from_slice(&[0x20, 0x00, 0x00, 0xf9]); // str x0, [x1]

        // Conditional branch
        code.extend_from_slice(&[0x1f, 0x00, 0x01, 0xeb]); // cmp x0, x1
        code.extend_from_slice(&[0x00, 0x00, 0x00, 0x54]); // b.eq #0

        // Function call
        code.extend_from_slice(&[0x00, 0x00, 0x00, 0x94]); // bl _func

        // Function epilogue
        code.extend_from_slice(&[0xfd, 0x7b, 0x41, 0xa9]); // ldp x29, x30, [sp, #16]
        code.extend_from_slice(&[0xff, 0x83, 0x00, 0x91]); // add sp, sp, #32
        code.extend_from_slice(&[0xc0, 0x03, 0x5f, 0xd6]); // ret
      }

      macho.add_code(code);

      // Add realistic data section with strings and constants
      let mut data = Vec::new();

      // String literals
      for i in 0..50 {
        let string = format!("String literal {i}\0");

        data.extend_from_slice(string.as_bytes());
      }

      // Numeric constants
      for i in 0..100 {
        data.extend_from_slice(&(i as u64).to_le_bytes());
      }

      // Float constants
      for i in 0..50 {
        let float_val = (i as f64) * 3.14159;

        data.extend_from_slice(&float_val.to_le_bytes());
      }

      macho.add_data(data);

      // Add realistic symbols
      // Main function
      macho.add_function_symbol("_main", 1, 0x400, true);

      // Library functions
      macho.add_undefined_symbol("_printf", 1);
      macho.add_undefined_symbol("_malloc", 1);
      macho.add_undefined_symbol("_free", 1);
      macho.add_undefined_symbol("_exit", 1);

      // User functions
      for i in 0..100 {
        let offset = 0x400 + (i * 60) as u64; // ~60 bytes per function

        macho.add_function_symbol(&format!("_func_{i}"), 1, offset, true);
      }

      // Global variables
      for i in 0..50 {
        macho.add_data_symbol(
          &format!("_global_{i}"),
          2,
          0x8000 + (i * 8) as u64,
          8,
          SymbolVisibility::Default,
        );
      }

      // String symbols
      for i in 0..50 {
        macho.add_data_symbol(
          &format!("_str_{i}"),
          2,
          0x9000 + (i * 32) as u64,
          32,
          SymbolVisibility::Default,
        );
      }

      // Add realistic relocations
      // Function calls
      for i in 0..100 {
        let call_offset = 0x400 + (i * 60 + 40) as u32; // Call near end of each function
        let target_func = if i < 99 { i + 1 } else { 0 }; // Call next function

        macho.add_text_relocation(
          call_offset,
          &format!("_func_{target_func}"),
          ARM64RelocationType::Branch26,
          true,
        );
      }

      // External function calls
      macho.add_text_relocation(
        0x500,
        "_printf",
        ARM64RelocationType::Branch26,
        true,
      );
      macho.add_text_relocation(
        0x520,
        "_malloc",
        ARM64RelocationType::Branch26,
        true,
      );

      // Data references (ADRP + ADD pairs)
      for i in 0..20 {
        let adrp_offset = 0x600 + (i * 8) as u32;
        macho.add_text_relocation(
          adrp_offset,
          &format!("_global_{i}"),
          ARM64RelocationType::Page21,
          true,
        );
        macho.add_text_relocation(
          adrp_offset + 4,
          &format!("_global_{i}"),
          ARM64RelocationType::Pageoff12,
          false,
        );
      }

      black_box(macho.finish_with_signature());
    });
  });
}

criterion_group!(
  benches,
  bench_small_binary,
  bench_medium_binary_with_symbols,
  bench_large_code_section,
  bench_many_relocations,
  bench_complex_binary,
  bench_signature_generation,
  bench_realistic_compiler_output
);

criterion_main!(benches);
